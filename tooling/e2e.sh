#!/usr/bin/env bash
#
# End-to-end test: builds the platform, deploys the example agents to a real
# Golem server, invokes them, and checks what comes back.
#
# Unlike `tooling/abi_harness`, nothing here is stubbed: the components run
# inside Golem, HTTP calls go to a real (local) web server, and agent-to-agent
# calls go through Golem's own RPC.
#
# Usage: ./tooling/e2e.sh [--cli <golem-binary>] [--port <router-port>]
#
# Environment:
#   GOLEM_CLI        golem CLI to use (default: `golem` on PATH). The local
#                    server must come from the same binary; this project is
#                    verified against golem 1.5.9 (see README).
#   E2E_WORK_DIR     scratch directory (default: build/e2e)
#   E2E_SKIP_BUILD   set to 1 to reuse the existing build/ artifacts
#
# A server already listening on the port is reused, and agents are created with
# a run-specific id so their state starts fresh.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLI="${GOLEM_CLI:-golem}"
PORT="${GOLEM_PORT:-9881}"
WORK_DIR="${E2E_WORK_DIR:-$ROOT_DIR/build/e2e}"
HTTP_PORT="${E2E_HTTP_PORT:-18099}"
# The HTTP API listener. `golem.yaml` pins the same port in
# `localServer.customRequestPort`, because that is what the CLI bakes into the
# deployment domain (`roc-golem.localhost:<port>`); a server bound to a
# different port is not reachable under that domain.
API_PORT="${E2E_API_PORT:-9882}"
API_DOMAIN="roc-golem.localhost:$API_PORT"
RUN_ID="e2e-$$"
MANIFEST="$ROOT_DIR/golem.yaml"

# Extra CLI flags, word-split on purpose (for example
# `GOLEM_CLI_EXTRA="--config-dir /tmp/cli-state"` to isolate CLI state).
CLI_EXTRA="${GOLEM_CLI_EXTRA:-}"

# Golem 1.6's local flow resolves the built-in "local" profile, so point it at
# this run's router port. Older CLIs ignore the variable.
export GOLEM_BUILTIN_LOCAL_URL="${GOLEM_BUILTIN_LOCAL_URL:-http://127.0.0.1:$PORT}"

failures=0
server_pid=""
http_pid=""

say() { printf '%s\n' "$*"; }

check() { # check <label> <expected substring> <actual>
    local label="$1" expected="$2" actual="$3"
    if [[ "$actual" == *"$expected"* ]]; then
        say "PASS  $label"
    else
        say "FAIL  $label"
        say "      expected to contain: $expected"
        say "      actual: ${actual:-<empty>}"
        failures=$((failures + 1))
    fi
}

cleanup() {
    [[ -n "$http_pid" ]] && kill "$http_pid" 2>/dev/null || true
    if [[ -n "$server_pid" ]]; then
        # `setsid` detaches the server, so match it by its unique data directory
        # rather than relying on the recorded pid.
        pkill -f "data-dir $WORK_DIR/data" 2>/dev/null || true
    fi
}
trap cleanup EXIT

mkdir -p "$WORK_DIR"

say "==> Building platform and example components"
if [[ "${E2E_SKIP_BUILD:-0}" != "1" ]]; then
    "$ROOT_DIR/tooling/build.sh" platform >"$WORK_DIR/build.log" 2>&1
    for app in counter ai_tool streaming_agent effects site static_site; do
        "$ROOT_DIR/tooling/build.sh" "$app" >>"$WORK_DIR/build.log" 2>&1
    done
fi
say "    components: $(ls "$ROOT_DIR/build"/*_agent.wasm | wc -l)"

# Any HTTP answer means the router is up; its root path is a 404.
server_up() { [[ "$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$PORT/")" != "000" ]]; }

say "==> Ensuring a Golem server is listening on :$PORT"
if ! server_up; then
    mkdir -p "$WORK_DIR/data" "$WORK_DIR/tmp"
    # The auxiliary ports and TMPDIR are pinned so this run cannot collide with
    # another local server (or lose its scratch space when a shell's TMPDIR
    # vanishes).
    TMPDIR="$WORK_DIR/tmp" setsid "$CLI" server run \
        --data-dir "$WORK_DIR/data" \
        --ports-file "$WORK_DIR/ports.json" \
        --router-port "$PORT" \
        --custom-request-port "$API_PORT" \
        --mcp-port "$((PORT + 2))" >"$WORK_DIR/server.log" 2>&1 < /dev/null &
    server_pid=$!
    for _ in $(seq 1 60); do
        server_up && break
        sleep 2
    done
    if ! server_up; then
        say "FAIL  could not start the Golem server (see $WORK_DIR/server.log)"
        exit 1
    fi
    say "    started $CLI (pid $server_pid), data in $WORK_DIR/data"
else
    say "    reusing the server already running"
fi

say "==> Starting a local web server for the effects agent"
mkdir -p "$WORK_DIR/www"
printf 'e2e payload' >"$WORK_DIR/www/index.txt"
(cd "$WORK_DIR/www" && exec python3 -m http.server "$HTTP_PORT" --bind 127.0.0.1) \
    >"$WORK_DIR/http.log" 2>&1 &
http_pid=$!
sleep 1

say "==> Deploying"
deploy_output="$("$CLI" $CLI_EXTRA -L -A "$MANIFEST" deploy --yes 2>&1 || true)"
if [[ "$deploy_output" == *"Finished deploying"* || "$deploy_output" == *"no changes required"* ]]; then
    say "PASS  deploy"
else
    say "FAIL  deploy"
    say "      ${deploy_output: -400}"
    failures=$((failures + 1))
fi

invoke() { # invoke <agent-id> <method> <argument> -> result line
    "$CLI" $CLI_EXTRA -L -A "$MANIFEST" agent invoke "$1" "$2" "$3" 2>&1 \
        | tail -n 1 \
        | sed -e 's/^"//' -e 's/"$//' -e 's/\\"/"/g'
}

# Requests to a mounted agent are routed by the `Host` header, which must be the
# domain the deployment registered.
fetch() { # fetch <path> -> body
    curl -s -H "Host: $API_DOMAIN" "http://127.0.0.1:$API_PORT$1"
}

fetch_status() { # fetch_status <path> -> HTTP status code
    curl -s -o /dev/null -w '%{http_code}' -H "Host: $API_DOMAIN" \
        "http://127.0.0.1:$API_PORT$1"
}

fetch_headers() { # fetch_headers <path> -> response head
    curl -s -D - -o /dev/null -H "Host: $API_DOMAIN" \
        "http://127.0.0.1:$API_PORT$1"
}

check_true() { # check_true <label> <actual boolean-ish>
    if [[ "$2" == "true" ]]; then
        say "PASS  $1"
    else
        say "FAIL  $1"
        failures=$((failures + 1))
    fi
}

say "==> Invoking agents (agent ids carry the run id: $RUN_ID)"
counter="counter-agent(\"$RUN_ID\")"

check "counter: first increment" "Counter incremented to 1" \
    "$(invoke "$counter" handle-message '"increment"')"
check "counter: state persisted" "Counter incremented to 2" \
    "$(invoke "$counter" handle-message '"increment"')"
check "counter: tool call" '{"count": 2}' \
    "$(invoke "$counter" get_count '"{}"')"
check "ai_tool: greeting" "I am an AI assistant agent" \
    "$(invoke "ai-tool-agent(\"$RUN_ID\")" handle-message '"hello"')"
check "streaming: status" "WebSocket is disconnected" \
    "$(invoke "streaming-agent(\"$RUN_ID\")" handle-message '"status"')"

check "effects: real HTTP call" "HTTP 200" \
    "$(invoke "effects-agent(\"http://127.0.0.1:$HTTP_PORT/\")" handle-message '"fetch"')"
check "effects: agent-to-agent RPC" "RPC reply:" \
    "$(invoke "effects-agent(\"$RUN_ID\")" handle-message '"rpc"')"

say "==> HTTP mount (file-site agent serving its own filesystem on $API_DOMAIN)"
site_agent="file-site-agent(\"$RUN_ID\")"
check "site: agent answers on the CLI surface" "is serving its mount" \
    "$(invoke "$site_agent" handle-message '"ping"')"
check "site: subtree binding serves a filesystem file" "e2e payload" \
    "$(fetch "/site/$RUN_ID/assets/hello.txt")"
check "site: exact binding serves a filesystem file" "Served from the agent's filesystem" \
    "$(fetch "/site/$RUN_ID/index.html")"
check "site: mapped file answers 200" "200" \
    "$(fetch_status "/site/$RUN_ID/assets/hello.txt")"
check "site: unmapped path answers 404" "404" \
    "$(fetch_status "/site/$RUN_ID/assets/missing.txt")"
check_true "site: configured file response header" \
    "$(fetch_headers "/site/$RUN_ID/assets/hello.txt" | grep -qi 'referrer-policy: no-referrer' && echo true || echo false)"
# The subtree binding exposes `/assets/*` only: the other initial file lives at
# `/site/index.html` and is reachable through the exact binding, not by walking
# the public path.
check "site: subtree binding does not expose other files" "404" \
    "$(fetch_status "/site/$RUN_ID/site/index.html")"
# Reading a mounted path materializes the instance's filesystem (provisioning
# the initial files) without invoking any method, so an id that was never
# called still serves its provisioned files.
check "site: first read serves provisioned files" "200" \
    "$(fetch_status "/site/absent-$RUN_ID/assets/hello.txt")"

say "==> HTTP router (static-site agent, served by Golem from uploaded files)"
# A router serves files that `golem deploy` uploaded for its agent type: no
# method runs, and nothing here creates an agent instance.
check "router: subtree binding serves an uploaded file" "uploaded at deploy time" \
    "$(fetch "/static/assets/hello.txt")"
check "router: exact binding serves an uploaded file" "Served by the router" \
    "$(fetch "/static/index.html")"
check_true "router: uploaded files carry an immutable ETag" \
    "$(fetch_headers "/static/assets/hello.txt" | grep -qi '^etag: "blake3-' && echo true || echo false)"
check "router: unmapped path answers 404" "404" \
    "$(fetch_status "/static/assets/missing.txt")"
# The router's OpenAPI provider method supplies the document for the mount, and
# Golem rebases its paths under the mount prefix.
check "router: openapi provider reaches the deployment spec" "The uploaded index page" \
    "$(fetch "/openapi.yaml")"

say ""
if [[ "$failures" -eq 0 ]]; then
    say "All end-to-end checks passed."
else
    say "$failures end-to-end check(s) failed."
    exit 1
fi
