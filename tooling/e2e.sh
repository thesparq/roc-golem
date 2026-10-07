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
RUN_ID="e2e-$$"
MANIFEST="$ROOT_DIR/golem.yaml"

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
    for app in counter ai_tool streaming_agent effects; do
        "$ROOT_DIR/tooling/build.sh" "$app" >>"$WORK_DIR/build.log" 2>&1
    done
fi
say "    components: $(ls "$ROOT_DIR/build"/*_agent.wasm | wc -l)"

# Any HTTP answer means the router is up; its root path is a 404.
server_up() { [[ "$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$PORT/")" != "000" ]]; }

say "==> Ensuring a Golem server is listening on :$PORT"
if ! server_up; then
    mkdir -p "$WORK_DIR/data"
    setsid "$CLI" server run \
        --data-dir "$WORK_DIR/data" \
        --ports-file "$WORK_DIR/ports.json" \
        --router-port "$PORT" >"$WORK_DIR/server.log" 2>&1 < /dev/null &
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
deploy_output="$("$CLI" -L -A "$MANIFEST" deploy --yes 2>&1 || true)"
if [[ "$deploy_output" == *"Finished deploying"* || "$deploy_output" == *"no changes required"* ]]; then
    say "PASS  deploy"
else
    say "FAIL  deploy"
    say "      ${deploy_output: -400}"
    failures=$((failures + 1))
fi

invoke() { # invoke <agent-id> <method> <argument> -> result line
    "$CLI" -L -A "$MANIFEST" agent invoke "$1" "$2" "$3" 2>&1 \
        | tail -n 1 \
        | sed -e 's/^"//' -e 's/"$//' -e 's/\\"/"/g'
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

say ""
if [[ "$failures" -eq 0 ]]; then
    say "All end-to-end checks passed."
else
    say "$failures end-to-end check(s) failed."
    exit 1
fi
