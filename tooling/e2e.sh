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
skipped=0
server_pid=""
http_pid=""
ds_pid=""

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
    # The durable-streams server detaches into its own session, so match it by
    # the scratch copy of the script rather than relying on the recorded pid.
    [[ -n "$ds_pid" ]] && kill "$ds_pid" 2>/dev/null || true
    pkill -f "$WORK_DIR/durable-streams/server.mjs" 2>/dev/null || true
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

# A result wider than the terminal comes back wrapped onto continuation lines
# indented by five spaces, so the wrap is undone before the checks see the text
# (the CLI only wraps when it can see a terminal width); the CLI's own quoting of
# the result is stripped as well.
unwrap_cli() {
    sed -e ':slurp' -e '$!{N;bslurp}' -e 's/\n     / /g' \
        | tail -n 1 \
        | sed -e 's/^"//' -e 's/"$//' -e 's/\\"/"/g'
}

invoke() { # invoke <agent-id> <method> <argument> -> result line
    "$CLI" $CLI_EXTRA -L -A "$MANIFEST" agent invoke "$1" "$2" "$3" 2>&1 | unwrap_cli
}

invoke_with_key() { # invoke_with_key <idempotency-key> <agent-id> <method> <argument>
    # The same, with an explicit idempotency key: a second call with the same key
    # is answered from the first invocation's recorded result instead of running
    # the agent again.
    "$CLI" $CLI_EXTRA -L -A "$MANIFEST" agent invoke -i "$1" "$2" "$3" "$4" 2>&1 | unwrap_cli
}

invoke_within() { # invoke_within <seconds> <agent-id> <method> <argument>
    # The same, but bounded: `awaitWebhook!` blocks until the callback arrives,
    # so a callback that never came has to fail its check, not hang the run.
    { timeout "$1" "$CLI" $CLI_EXTRA -L -A "$MANIFEST" agent invoke "$2" "$3" "$4" 2>&1 || true; } | unwrap_cli
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

check_nospace() { # check_nospace <label> <expected substring> <actual>
    # The same, for replies the CLI wraps: a wrap only ever adds whitespace (see
    # the retry section), so both sides are compared with their whitespace
    # removed. Use it wherever the expected text has spaces of its own — a
    # message-style reply rather than JSON.
    check "$1" "${2// /}" "${3// /}"
}

say "==> Invoking agents (agent ids carry the run id: $RUN_ID)"
counter="counter-agent(\"$RUN_ID\")"

check "counter: first increment" "Counter incremented to 1" \
    "$(invoke "$counter" handle-message '"increment"')"
check "counter: state persisted" "Counter incremented to 2" \
    "$(invoke "$counter" handle-message '"increment"')"
check "counter: method named like a tool" '{"count": 2}' \
    "$(invoke "$counter" get_count '"{}"')"
check "ai_tool: greeting" "I am an AI assistant agent" \
    "$(invoke "ai-tool-agent(\"$RUN_ID\")" handle-message '"hello"')"

say "==> Multimodal values (a method whose parts travel as a multimodal list)"
# `handle-multimodal` declares a multimodal input *and* result — a list of
# `{tag, value}` parts — which is what lets one call carry text and binary
# content. The caller writes the parts in the CLI's literal syntax (the
# component's source language is `roc`, which the CLI parses with its TypeScript
# dialect), Golem builds the value tree from the declared schema, the host hands
# the app the JSON envelope of those parts, and the envelope the app answers with
# becomes the parts the caller sees. The counter's hook reports what it read out
# of the envelope and hands the caller's own parts back, so one call shows both
# directions.
mm_agent="counter-agent(\"$RUN_ID-mm\")"
mm_parts='[{tag: "text", value: Text("hi from e2e")}, {tag: "binary", value: Binary("data:image/png;base64,AQID")}]'
# The increment is what makes the state check below meaningful: the hook reports
# the count its state carried.
invoke "$mm_agent" handle-message '"increment"' >/dev/null
# The reply is a long rendered value the CLI wraps, and a wrap only ever adds
# whitespace (see the retry section), so these compare space-free copies.
mm_reply="$(invoke "$mm_agent" handle-multimodal "$mm_parts")"
check_nospace "multimodal: the hook ran against the agent's state" 'count=1;' "$mm_reply"
check_nospace "multimodal: the app read the text part" 'text=hi from e2e' "$mm_reply"
check_nospace "multimodal: the app read the binary part's bytes and MIME type" \
    'bytes=[1,2,3];mime=image/png' "$mm_reply"
check_nospace "multimodal: the caller's text part came back" 'Text("hi from e2e")' \
    "$mm_reply"
check_nospace "multimodal: the caller's binary part came back" \
    'Binary("data:image/png;base64,AQID")' "$mm_reply"

say '==> Tool surface (golem:tool/guest, what `golem tool invoke` and MCP use)'
# Tools run on the agent's instance but without its state, so a stateless tool
# answers and a stateful one reports the missing state instead of inventing one.
tool_output() { # tool_output <agent-id> <tool>
    # A failing tool is a non-zero exit, which is exactly what one of the checks
    # below expects, so keep the output either way.
    "$CLI" $CLI_EXTRA -L -A "$MANIFEST" tool invoke --agent "$1" "$2" 2>&1 || true
}
calculator_output="$(tool_output "ai-tool-agent(\"$RUN_ID\")" calculator)"
check_true "ai_tool: stateless tool answers through the tool surface" \
    "$(grep -q 'Success' <<<"$calculator_output" && grep -q '42' <<<"$calculator_output" && echo true || echo false)"
get_count_output="$(tool_output "$counter" get-count)"
check_true "counter: stateful tool reports the missing state" \
    "$(grep -q 'Missing count field' <<<"$get_count_output" && echo true || echo false)"

# Tool streams: the ai tool's `echo` is the stateless tool that uses them — the
# bytes piped into `--stdin` come back out on the tool's stdout, and its stderr
# says how many arrived. Nothing here reads agent state, which is what the fresh
# instance a tool runs in requires.
tool_stdout_file="$WORK_DIR/tool-stdout.txt"
tool_stderr_file="$WORK_DIR/tool-stderr.txt"
printf 'piped stdin bytes' | "$CLI" $CLI_EXTRA -L -A "$MANIFEST" tool invoke \
    --agent "ai-tool-agent(\"$RUN_ID\")" echo --stdin - \
    --stdout --output "$tool_stdout_file" \
    --stderr --stderr-output "$tool_stderr_file" >/dev/null 2>&1 || true
check "tool: stdin comes back on the tool's stdout" "piped stdin bytes" \
    "$(cat "$tool_stdout_file" 2>/dev/null || true)"
check "tool: stderr reports what arrived" "read 17 bytes of stdin" \
    "$(cat "$tool_stderr_file" 2>/dev/null || true)"

check "streaming: status" "WebSocket is disconnected" \
    "$(invoke "streaming-agent(\"$RUN_ID\")" handle-message '"status"')"

check "effects: real HTTP call" "HTTP 200" \
    "$(invoke "effects-agent(\"http://127.0.0.1:$HTTP_PORT/\")" handle-message '"fetch"')"
check "effects: agent-to-agent RPC" "RPC reply:" \
    "$(invoke "effects-agent(\"$RUN_ID\")" handle-message '"rpc"')"

say "==> Agent-to-agent calls beyond invoke-and-await"
# Fire-and-forget, scheduled and cancelable calls. All of them target a counter
# in another component, so these checks also cover the cross-component
# resolution the platform's `parse-agent-id` does on the way to `wasm-rpc`.
calls_counter="counter-agent(\"$RUN_ID-calls\")"
effects_caller="effects-agent(\"$RUN_ID\")"

# `handle-message` takes one JSON string argument, so a message carrying an
# agent id (which has quotes in it) has to be escaped for that.
message_arg() { # message_arg <message> -> JSON argument
    printf '"%s"' "${1//\"/\\\"}"
}

# The fire-and-forget and scheduled calls are asynchronous, so the target's
# state is polled for the effect rather than read once; giving up fails the
# check instead of hanging the run.
poll_reply() { # poll_reply <agent-id> <message> <expected substring>
    local agent="$1" message="$2" expected="$3" actual=""
    for _ in $(seq 1 20); do
        actual="$(invoke "$agent" handle-message "$message")"
        [[ "$actual" == *"$expected"* ]] && break
        sleep 1
    done
    printf '%s' "$actual"
}

check "effects: fire-and-forget returns an invocation identity" "idempotencyKey" \
    "$(invoke "$effects_caller" handle-message \
        "$(message_arg "fire-and-forget $calls_counter handle-message increment")")"
check "effects: fire-and-forget reaches the counter" "Current count is 1" \
    "$(poll_reply "$calls_counter" '"get"' "Current count is 1")"

check "effects: scheduled invocation returns an invocation identity" "idempotencyKey" \
    "$(invoke "$effects_caller" handle-message \
        "$(message_arg "schedule $calls_counter handle-message increment 1500")")"
check "effects: scheduled invocation reaches the counter" "Current count is 2" \
    "$(poll_reply "$calls_counter" '"get"' "Current count is 2")"

# A cancelable schedule that is left alone still runs, which is what makes the
# canceled one below meaningful.
check "effects: cancelable schedule returns a handle" "Cancelable scheduled in" \
    "$(invoke "$effects_caller" handle-message \
        "$(message_arg "schedule-cancelable $calls_counter handle-message increment 1500")")"
check "effects: an uncanceled schedule reaches the counter" "Current count is 3" \
    "$(poll_reply "$calls_counter" '"get"' "Current count is 3")"

# The schedule's own reply is not what this pair checks, so it is dropped. The
# delay leaves room for the cancel below to arrive well before it.
invoke "$effects_caller" handle-message \
    "$(message_arg "schedule-cancelable $calls_counter handle-message increment 8000")" >/dev/null
check "effects: canceling a scheduled invocation" "Canceled scheduled invocation" \
    "$(invoke "$effects_caller" handle-message "$(message_arg "cancel")")"
# Past the canceled invocation's scheduled time: the counter must not have
# moved.
sleep 8
check "effects: the canceled invocation never reached the counter" "Current count is 3" \
    "$(invoke "$calls_counter" handle-message '"get"')"

check "effects: make-agent-id builds an id" "counter-agent(\"$RUN_ID-calls\")" \
    "$(invoke "$effects_caller" handle-message \
        "$(message_arg "make-agent-id counter-agent $RUN_ID-calls")")"
# The reply is one long line of JSON, and the CLI wraps such a line when it can
# see a terminal width (a wrap only adds whitespace, see the retry section), so
# this check compares a space-free copy against a space-free substring.
parsed_id="$(invoke "$effects_caller" handle-message \
    "$(message_arg "parse-agent-id $calls_counter")")"
check "effects: parse-agent-id reads an id back" "\"constructor\":\"$RUN_ID-calls\"" \
    "${parsed_id// /}"

say "==> Quotas (golem:quota/types against the manifest's resourceDefaults)"
# The counter's `quota` command reserves and commits from `example-requests`,
# which the manifest declares with a capacity of ten and
# `enforcementAction: reject`. Capacity is consumed as it is committed, and it
# is durable and shared by the whole environment (not per agent), so a fresh data
# directory is the only place the first reservation fits — a rerun against the
# same one finds it spent and says so instead of failing.
quota_agent="counter-agent(\"$RUN_ID-quota\")"
quota_first="$(invoke "$quota_agent" handle-message '"quota 6"')"
if [[ "$quota_first" == *"Committed 6 units"* ]]; then
    say "PASS  quota: a reservation that fits is committed"
elif [[ "$quota_first" == *"quota reservation failed"* ]]; then
    say "SKIP  quota: a reservation that fits is committed (the environment's capacity is already spent)"
    skipped=$((skipped + 1))
else
    say "FAIL  quota: a reservation that fits is committed"
    say "      actual: ${quota_first:-<empty>}"
    failures=$((failures + 1))
fi
check "quota: the next reservation exceeds the capacity" "quota reservation failed" \
    "$(invoke "$quota_agent" handle-message '"quota 6"')"

say "==> Retry policies (golem:api/retry against the manifest's retryPolicyDefaults)"
# The counter's `retry` command reports the policies the environment declares,
# resolves the rule that matches a `send` operation, and walks the resolved
# policy's delay schedule twice. `send-retries` is the manifest's rule: two
# retries with a fixed 100ms delay, and a priority that beats the environment's
# own `predTrue` default (which every other verb resolves to).
retry_reply="$(invoke "$quota_agent" handle-message '"retry"')"
# The reply is one long line of JSON that the CLI wraps, and a wrap only ever
# adds whitespace, so the checks compare a space-free copy against space-free
# substrings.
retry_reply="${retry_reply// /}"
check "retry: the declared policy is visible" '"name":"send-retries"' "$retry_reply"
check "retry: a matching operation resolves the policy" \
    'match={"nodes":[{"countBox":{"maxRetries":2,"inner":1}}' "$retry_reply"
check "retry: the first walk step returns the policy's delay" \
    'first={"delayNanos":100000000' "$retry_reply"
check "retry: the second walk step returns the policy's delay" \
    'second={"delayNanos":100000000' "$retry_reply"

say "==> Key-value and blobstore (wasi:keyvalue / wasi:blobstore)"
# Both stores belong to the environment rather than to one agent, and both are
# reached through synchronous host calls. The counter's round-trip commands
# report every step's outcome in one line: the value goes in, comes back out,
# the key or object is deleted and read again — and a missing key reads as the
# JSON literal `null`, which is how the platform tells it from an empty value.
stores_agent="counter-agent(\"$RUN_ID-stores\")"

kv_reply="$(invoke "$stores_agent" handle-message \
    "$(message_arg "kv e2e-bucket-$RUN_ID e2e-key-$RUN_ID hello kv")")"
# A wrapped reply only ever gains whitespace (see the retry section), so the
# checks compare a space-free copy against space-free substrings.
kv_reply="${kv_reply// /}"
check "kv: set, get and exists round trip" "kvset=okget=hellokvexists=true" "$kv_reply"
check "kv: the deleted key is gone and reads as null" \
    "delete=okexists-after=falsemissing=null" "$kv_reply"

blob_reply="$(invoke "$stores_agent" handle-message \
    "$(message_arg "blob e2e-container-$RUN_ID e2e-object-$RUN_ID.txt hello blob")")"
blob_reply="${blob_reply// /}"
check "blob: create, put, get and has round trip" \
    "blobcreate=okput=okget=helloblobhas=true" "$blob_reply"
check "blob: list names the object" \
    "list=[\"e2e-object-$RUN_ID.txt\"]" "$blob_reply"
check "blob: the deleted object is gone" "delete=okhas-after=false" "$blob_reply"

say "==> The agent's own filesystem (wasi:filesystem@0.2.6)"
# One filesystem per agent, provisioned from the manifest's per-agent `files:`
# list and durable with the agent: the runtime records every write in the
# invocation's oplog and replays it, so a file one invocation wrote is there for
# a later one. Paths resolve against the agent's root — a leading `/` names it,
# the spelling the manifest's targetPath uses — and a path that would step above
# the root is refused before the host is asked.
files_agent="counter-agent(\"$RUN_ID-files\")"
seed_path="/seed/hello.txt"
notes_path="/seed/notes-$RUN_ID.txt"
seed_size="$(wc -c <"$ROOT_DIR/examples/counter/seed/hello.txt" | tr -d '[:space:]')"

check "fs: reading the manifest-provisioned file" "provisioned by golem.yaml" \
    "$(invoke "$files_agent" handle-message "$(message_arg "fs read $seed_path")")"
check "fs: the provisioned file's size" "fs size: $seed_size" \
    "$(invoke "$files_agent" handle-message "$(message_arg "fs size $seed_path")")"
check "fs: the provisioned file exists" "fs exists: true" \
    "$(invoke "$files_agent" handle-message "$(message_arg "fs exists $seed_path")")"
check "fs: a path that is not there does not exist" "fs exists: false" \
    "$(invoke "$files_agent" handle-message "$(message_arg "fs exists /seed/missing-$RUN_ID.txt")")"

# The write and the read are separate invocations: reading what the earlier one
# wrote is what shows the filesystem is durable rather than per-invocation.
check "fs: writing a file" "fs wrote $notes_path" \
    "$(invoke "$files_agent" handle-message "$(message_arg "fs write $notes_path hello durability")")"
check "fs: a later invocation reads it back" "fs read: hello durability" \
    "$(invoke "$files_agent" handle-message "$(message_arg "fs read $notes_path")")"
check "fs: appending in a later invocation" "fs appended $notes_path" \
    "$(invoke "$files_agent" handle-message "$(message_arg "fs append $notes_path , appended later")")"
# `appendFile!` writes at the end with no separator of its own, so the appended
# text is checked with the comma it carried.
check "fs: the appended text is at the end of the file" "hello durability, appended later" \
    "$(invoke "$files_agent" handle-message "$(message_arg "fs read $notes_path")")"

# The listing carries the provisioned file and the written one together, sorted
# by name, one `{name, type}` per entry. A wrapped reply only ever gains
# whitespace (see the retry section), so the checks compare a space-free copy
# against space-free substrings.
seed_listing="$(invoke "$files_agent" handle-message "$(message_arg "fs list /seed")")"
seed_listing="${seed_listing// /}"
check "fs: the listing shows the provisioned file" \
    '{"name":"hello.txt","type":"file"}' "$seed_listing"
check "fs: the listing shows the written file" \
    "{\"name\":\"notes-$RUN_ID.txt\",\"type\":\"file\"}" "$seed_listing"

check "fs: making a directory" "fs made dir /made-$RUN_ID" \
    "$(invoke "$files_agent" handle-message "$(message_arg "fs mkdir /made-$RUN_ID")")"
root_listing="$(invoke "$files_agent" handle-message "$(message_arg "fs list /")")"
root_listing="${root_listing// /}"
check "fs: the made directory is a directory entry" \
    "{\"name\":\"made-$RUN_ID\",\"type\":\"dir\"}" "$root_listing"

# The path rule, both ways: a path above the root is refused with the app's own
# path in the message, and `..` that stays inside normalizes back.
check "fs: a path above the root is refused" "escapes the agent's filesystem root" \
    "$(invoke "$files_agent" handle-message "$(message_arg "fs read ../golem.yaml")")"
check "fs: a path that normalizes inside the root works" "provisioned by golem.yaml" \
    "$(invoke "$files_agent" handle-message "$(message_arg "fs read /seed/../seed/hello.txt")")"

check "fs: deleting a file" "fs deleted $notes_path" \
    "$(invoke "$files_agent" handle-message "$(message_arg "fs delete $notes_path")")"
check "fs: the deleted file is gone" "fs exists: false" \
    "$(invoke "$files_agent" handle-message "$(message_arg "fs exists $notes_path")")"

say "==> Databases (golem:rdbms)"
# The counter's `db` commands. Golem parses a connection address when `open`
# is called and creates the pool on the first statement, so a well-formed
# address opens without a database running — and an address that does not parse
# comes back as the effect's `Err` (the interface's `connection-failure`
# variant), not as a trap. Those two are the honest checks on a machine with no
# database; the full round trip against a real one is opt-in further down.
db_agent="counter-agent(\"$RUN_ID-db\")"

check "db: a well-formed address opens a connection" "db opened handle " \
    "$(invoke "$db_agent" handle-message "$(message_arg "db open postgresql://roc@127.0.0.1:5432/roc")")"
check "db: an address Golem cannot parse is a connection failure" \
    "db open failed: connection-failure" \
    "$(invoke "$db_agent" handle-message "$(message_arg "db open not-a-database-url")")"
# MySQL and Ignite are separate interfaces with their own `open`s; their
# addresses are parsed the same way, so neither needs a server here either.
check "db: the MySQL interface opens" "db mysql opened handle " \
    "$(invoke "$db_agent" handle-message "$(message_arg "db open-mysql mysql://root@127.0.0.1:3306/roc")")"
check "db: the Ignite interface opens" "db ignite opened handle " \
    "$(invoke "$db_agent" handle-message "$(message_arg "db open-ignite ignite://127.0.0.1:10800")")"
# The first statement is where the pool is created, so a round trip against an
# address with no server behind it reports the host's failure in the effect's
# `Err` channel on every step — the invocation itself survives.
db_no_server="$(invoke "$db_agent" handle-message \
    "$(message_arg "db roundtrip roc_e2e_${RUN_ID//-/_} postgresql://roc@127.0.0.1:5432/roc")")"
check "db: a statement without a reachable server fails as an Err" \
    "db create=error(connection-failure" "$db_no_server"

say "==> Databases: a real round trip (E2E_DATABASE_URL)"
# The data plane against a real postgres: a create, an insert, a select and a
# count with the values travelling as the `db-value` JSON; then a rolled-back
# and a committed transaction, which is what makes the count checks meaningful.
# The table is named after the run and dropped at the end of the round trip.
db_table="roc_e2e_${RUN_ID//-/_}"
if [[ -z "${E2E_DATABASE_URL:-}" ]]; then
    say "SKIP  db: a real round trip (set E2E_DATABASE_URL to a postgresql:// address)"
    skipped=$((skipped + 1))
elif [[ "${E2E_DATABASE_URL}" != postgresql://* ]]; then
    say "SKIP  db: a real round trip (E2E_DATABASE_URL is not a postgresql:// address)"
    skipped=$((skipped + 1))
else
    db_roundtrip="$(invoke "$db_agent" handle-message \
        "$(message_arg "db roundtrip $db_table $E2E_DATABASE_URL")")"
    # A wrapped reply only ever gains whitespace (see the retry section), so the
    # checks compare a space-free copy against space-free substrings; the
    # database's own type names are not asserted (the harness pins the shape),
    # only the values, the column names and the WIT types the result carries.
    db_roundtrip="${db_roundtrip// /}"
    check "db: the create and the insert report their row counts" \
        "dbcreate=0insert=1" "$db_roundtrip"
    check "db: the inserted row comes back with its columns" \
        '"rows":[[{"tag":"int4","value":1},{"tag":"text","value":"hello db"}]]' "$db_roundtrip"
    check "db: the result names the selected columns" \
        '"name":"note"' "$db_roundtrip"
    check "db: the rolled-back transaction leaves the count alone" \
        'count2={"columns":[{"dbType":{"tag":"int8"}' "$db_roundtrip"
    check "db: and its row is not there" '"rows":[[{"tag":"int8","value":1}]]}begin=ok' "$db_roundtrip"
    check "db: the committed transaction moves the count" \
        '"rows":[[{"tag":"int8","value":2}]]}' "$db_roundtrip"
    check "db: the rolled-back row is not selectable" 'select2={"columns":[{"dbType":{"tag":"int4"}' "$db_roundtrip"
    check "db: the round trip dropped its table" 'drop=0' "$db_roundtrip"
fi

say "==> Permissions and the principal (golem:permissions)"
# The counter's `permission` commands drive the whole surface, and every answer
# here is the runtime's own: the agent's wallet, the card's lattice, a derivation
# that narrows it, one the card cannot cover, a revocation, and an installation
# the runtime only allows onto agents. A card is a host-side capability the app
# carries as a `U32` handle, so the commands also show the handle rules the
# platform adds on top of the interfaces (a borrow leaves a handle usable, a
# revoke spends it, a release gives it up).
#
# Replies here are long and the CLI wraps them, and a wrap only ever adds
# whitespace, so these checks compare space-free copies (`check_nospace`).
perm_agent="counter-agent(\"$RUN_ID-perm\")"

# What the runtime hands a method: a CLI invocation is the calling *user* (the
# CLI's static token names an account), and an invocation from another agent is
# that *agent* — which the effects agent's `principal-rpc` command shows by
# calling the counter and reporting what the callee saw.
perm_principal="$(invoke "$perm_agent" handle-message '"permission principal"')"
check "permission: a CLI invocation is a user principal" '"tag":"golem-user"' \
    "${perm_principal// /}"
check_true "permission: the principal carries the caller's account id" \
    "$([[ "${perm_principal// /}" =~ \"accountId\":\"[0-9a-f-]{36}\" ]] && echo true || echo false)"

rpc_principal="$(invoke "$effects_caller" handle-message \
    "$(message_arg "principal-rpc $perm_agent")")"
check_nospace "permission: an RPC caller is an agent principal" '"tag":"agent"' "$rpc_principal"
check_nospace "permission: the agent principal names the caller" \
    '"agentId":"effects-agent(' "$rpc_principal"
check_nospace "permission: and the caller's run id" "$RUN_ID" "$rpc_principal"

# The wallet: the counter declares no `initialCard`, so the runtime gives it the
# broad default card for the agent type — a template whose grants carry slots
# (`?agent`, `?env`) — and `permission wallet` hands the app a handle for it.
perm_wallet="$(invoke "$perm_agent" handle-message '"permission wallet"')"
check_nospace "permission: the default card is in the wallet" '"tag":"template"' "$perm_wallet"
check_nospace "permission: the wallet entry carries a card handle" '"handle":1' "$perm_wallet"
check_nospace "permission: the wallet version is a hash and a generation" '"walletIdHash":[' \
    "$(invoke "$perm_agent" handle-message '"permission version"')"

# The card itself: its lattice, its id, its parent in the derivation DAG and its
# expiry. The default card grants the agent's own filesystem and its environment,
# which is what `inspect` shows.
perm_inspect="$(invoke "$perm_agent" handle-message '"permission inspect 1"')"
check_nospace "permission: inspecting a card shows its grants" '"lowerPositive":[' "$perm_inspect"
check_nospace "permission: the card grants the agent's own filesystem" \
    '"class":"filesystem"' "$perm_inspect"
perm_id="$(invoke "$perm_agent" handle-message '"permission id 1"')"
check_true "permission: a card has a UUID id" \
    "$([[ "${perm_id// /}" =~ [0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12} ]] && echo true || echo false)"
check_nospace "permission: the card names its parent" 'parents:["0' \
    "$(invoke "$perm_agent" handle-message '"permission parents 1"')"
check "permission: a card with no bounded lifespan never expires" "permission expires: null" \
    "$(invoke "$perm_agent" handle-message '"permission expires 1"')"

# The kernel's own schema: the classes and verbs a grant may name, and the check
# of one concrete grant. A slot (`?agent`) is an `invalid-pattern` there — a slot
# only means something inside a card — so the valid grant is fully concrete.
perm_modules="$(invoke "$perm_agent" handle-message '"permission modules"')"
check_nospace "permission: the kernel lists its resource classes" \
    '"className":"filesystem"' "$perm_modules"
check_nospace "permission: and the verbs of a class" \
    '"verbs":["read","write","list","stat","delete"]' "$perm_modules"
perm_grant='{"class":"filesystem","owner":"alice@example.com/shop/prod/cart-svc/CartAgent","recipient":"alice@example.com/shop/prod/cart-svc/CartAgent","verb":"read","resourceId":"/data/**"}'
check "permission: the kernel accepts a concrete grant" "permission validate: accepted" \
    "$(invoke "$perm_agent" handle-message "$(message_arg "permission validate $perm_grant")")"
perm_unknown_grant="${perm_grant/\"class\":\"filesystem\"/\"class\":\"nope\"}"
check "permission: a class the kernel does not know is reported" \
    "unknown-resource-class: nope" \
    "$(invoke "$perm_agent" handle-message "$(message_arg "permission validate $perm_unknown_grant")")"

# Derivations: the counter builds the requests from the card's own inspection, so
# the narrowing one retains a grant the card really carries (a `network` grant
# narrowed to one destination) and the over-broad one asks for an `account`
# grant, which no card of this agent's lattice covers. The first is the runtime's
# acceptance, the second its refusal — a real enforcement check.
check "permission: a narrowing derivation is accepted" "permission narrowing: handle " \
    "$(invoke "$perm_agent" handle-message '"permission derive-narrow 1"')"
check "permission: a derivation the card cannot cover is refused" \
    "lower-bound-too-broad" \
    "$(invoke "$perm_agent" handle-message '"permission derive-broad 1"')"
check "permission: an invocation-local scope card is derived too" "permission scope: handle " \
    "$(invoke "$perm_agent" handle-message '"permission derive-scope-narrow 1"')"

# The handle rules, in one invocation on an agent of its own (so the handles are
# the ones the round trip expects): the child is revoked (one card, the child
# itself), the same handle is then reported rather than replayed, the scope card
# and the wallet card are released, and a released handle is gone.
perm_roundtrip="$(invoke "counter-agent(\"$RUN_ID-perm-rt\")" handle-message '"permission roundtrip"')"
check_nospace "permission: revoking the derived card reports the cut" \
    "permission revoke: revoked 1 card(s)" "$perm_roundtrip"
check_nospace "permission: a consumed handle is reported, not replayed" \
    "permission revoke failed: unknown permission card handle 2" "$perm_roundtrip"
check_nospace "permission: the scope card and the wallet card are released" \
    "permission release: released permission release: released" "$perm_roundtrip"
check_nospace "permission: a released handle is gone" \
    "permission release failed: unknown permission card handle 1" "$perm_roundtrip"
check_nospace "permission: the over-broad derivation is refused here too" \
    "permission over-broad failed: lower-bound-too-broad" "$perm_roundtrip"

# Installation: only agent targets are supported, and the interface consumes the
# card, so the refused attempt spends the handle like any other install does.
check_nospace "permission: only agent installation targets are supported" \
    "not-permitted: install-card currently supports only agent targets" \
    "$(invoke "$perm_agent" handle-message '"permission install-account 1"')"
check_nospace "permission: the refused installation spent the handle" \
    "unknown permission card handle 1" \
    "$(invoke "$perm_agent" handle-message '"permission install-account 1"')"

# And the installation the runtime accepts: a derived card onto this agent, which
# is then a concrete card in the wallet.
perm_install_agent="counter-agent(\"$RUN_ID-perm-install\")"
invoke "$perm_install_agent" handle-message '"permission wallet"' >/dev/null
check "permission: a narrowing card is derived" "permission narrowing: handle 2" \
    "$(invoke "$perm_install_agent" handle-message '"permission derive-narrow 1"')"
check "permission: the derived card installs onto this agent" "permission install: installed" \
    "$(invoke "$perm_install_agent" handle-message '"permission install-self 2"')"
check_nospace "permission: the installed card is a concrete card in the wallet" \
    '"tag":"concrete"' \
    "$(invoke "$perm_install_agent" handle-message '"permission wallet"')"

say "==> External durable streams (golem:agent/durable-streams)"
# The counter's `stream` commands drive the whole surface against a real
# service: the Durable Streams protocol's own reference server, the Node package
# `@durable-streams/server`, running in memory here. The stream itself is made
# with the protocol's `PUT` — the effects read and append to a stream, they do
# not create one — the writer appends two JSON messages, and the reader reads
# them back from the beginning and again from the checkpoint the first read
# answered with. A contributor without Node (or without the package) gets a
# documented SKIP instead of a red run; the CI runner has Node, so the section
# runs there.
DS_VERSION="${E2E_DS_VERSION:-0.3.9}"
DS_PORT="${E2E_DS_PORT:-4437}"
DS_BASE="http://127.0.0.1:$DS_PORT"

# Any HTTP answer means a Durable Streams server is listening.
ds_up() { [[ "$(curl -s -o /dev/null -w '%{http_code}' "$DS_BASE/")" != "000" ]]; }

ds_reason=""
if ds_up; then
    say "    reusing the durable-streams server already listening on :$DS_PORT"
elif ! command -v node >/dev/null 2>&1 || ! command -v npm >/dev/null 2>&1; then
    ds_reason="node and npm are not on PATH (the reference server is the Node package @durable-streams/server)"
else
    mkdir -p "$WORK_DIR/durable-streams"
    cp "$ROOT_DIR/tooling/durable_streams_server.mjs" "$WORK_DIR/durable-streams/server.mjs"
    # `--ignore-scripts` because the in-memory store needs no native build:
    # the one dependency with an install script (`lmdb`, for the file-backed
    # store the e2e does not use) ships prebuilt binaries either way, and this
    # keeps npm 10 and npm 11 on the same path.
    if ! npm install --no-audit --no-fund --silent --ignore-scripts \
        --prefix "$WORK_DIR/durable-streams" \
        "@durable-streams/server@$DS_VERSION" \
        >"$WORK_DIR/durable-streams/npm.log" 2>&1; then
        ds_reason="npm could not install @durable-streams/server@$DS_VERSION (see $WORK_DIR/durable-streams/npm.log)"
    else
        # The script is named by its absolute path so this run's server can be
        # told apart (and stopped) by that path; node resolves the package from
        # the script's own directory, which is where npm installed it.
        setsid node "$WORK_DIR/durable-streams/server.mjs" "$DS_PORT" \
            >"$WORK_DIR/durable-streams/server.log" 2>&1 < /dev/null &
        ds_pid=$!
        for _ in $(seq 1 20); do
            ds_up && break
            sleep 0.5
        done
        if ds_up; then
            say "    started the reference server on :$DS_PORT (data in memory)"
        else
            ds_reason="the server did not come up (see $WORK_DIR/durable-streams/server.log)"
        fi
    fi
fi

if [[ -n "$ds_reason" ]]; then
    say "SKIP  stream: the durable-streams round trip ($ds_reason)"
    skipped=$((skipped + 1))
else
    stream_agent="counter-agent(\"$RUN_ID-stream\")"
    json_stream="$DS_BASE/v1/stream/roc-e2e-${RUN_ID}-orders"
    bytes_stream="$DS_BASE/v1/stream/roc-e2e-${RUN_ID}-blob"
    missing_stream="$DS_BASE/v1/stream/roc-e2e-${RUN_ID}-missing"

    # The protocol's own create, which the effects do not perform: a `PUT` with
    # the stream's media type answers 201.
    check_true "stream: the protocol's create answers 201" \
        "$([[ "$(curl -s -o /dev/null -w '%{http_code}' -X PUT \
            -H 'Content-Type: application/json' "$json_stream")" == "201" ]] && echo true || echo false)"

    # The replies are long, so the checks compare space-free copies (the CLI
    # only wraps, and a wrap adds whitespace).
    stream_roundtrip="$(invoke "$stream_agent" handle-message \
        "$(message_arg "stream roundtrip $json_stream one two")")"
    stream_roundtrip="${stream_roundtrip// /}"
    check "stream: the writer's first append reports its sequence" \
        '"epoch":0,"sequence":0,"closed":false}' "$stream_roundtrip"
    check "stream: and the second one the next sequence" \
        '"epoch":0,"sequence":1,"closed":false}' "$stream_roundtrip"
    # The batch's payload is itself JSON text, so the CLI's one level of
    # unescaping (`unwrap_cli`) leaves one level behind: the reply carries
    # `[\\"one\\",\\"two\\"]` where the app saw `[\"one\",\"two\"]`.
    check "stream: the read returns both appended messages" \
        '"payload":"[\\"one\\",\\"two\\"]"' "$stream_roundtrip"
    # The reference server's offsets are zero-padded decimal tokens, so an
    # offset starting with `0` is one it generated rather than the `-1` the
    # first read asked from.
    check "stream: the read answers the service's checkpoint" \
        '"next":{"offset":"0' "$stream_roundtrip"
    check "stream: the second read starts from that checkpoint" \
        'from2=0' "$stream_roundtrip"
    check "stream: and finds the stream up to date, with an empty batch" \
        'read2={"payload":"[]","payloadBytes":2,"contentType":"application/json","next":{"offset":"0' \
        "$stream_roundtrip"
    check "stream: the writer's handle was released after the appends" \
        'released=okread1=' "$stream_roundtrip"
    check "stream: and the reader's after the reads" \
        '"closed":false}released=ok' "$stream_roundtrip"

    # A URL that names no stream is the service's own answer: `not-found`,
    # rendered as `<kind>: <message>` in the effect's error channel, which the
    # invocation survives.
    check_nospace "stream: a stream that does not exist is the service's not-found" \
        "stream read=error(not-found:" \
        "$(invoke "$stream_agent" handle-message "$(message_arg "stream read $missing_stream")")"

    # A byte stream, where an append can close: the receipt says so and the read
    # answers the bytes with the stream's `closed` flag.
    check_true "stream: the protocol's create answers 201 for the byte stream" \
        "$([[ "$(curl -s -o /dev/null -w '%{http_code}' -X PUT \
            -H 'Content-Type: application/octet-stream' "$bytes_stream")" == "201" ]] && echo true || echo false)"
    stream_bytes="$(invoke "$stream_agent" handle-message \
        "$(message_arg "stream bytes $bytes_stream e2e-bytes")")"
    stream_bytes="${stream_bytes// /}"
    check "stream: an append that closes is acknowledged as closed" \
        '"epoch":0,"sequence":0,"closed":true}released=okread={' "$stream_bytes"
    check "stream: the byte stream reads its bytes back" \
        'read={"payload":"e2e-bytes","payloadBytes":9' "$stream_bytes"
    check "stream: and the batch carries the stream's closure" \
        '"upToDate":true,"closed":true}released=ok' "$stream_bytes"
fi

say "==> Oplog and execution control (golem:api/host)"
# What an app can reach on 1.6: fail an invocation (`trap!`), name an external
# write durably (`idempotencyKey!`), fork the agent (`fork!`), wait for the oplog
# to reach replicas (`oplogCommit!`), and resolve components and agents
# (`resolveComponentId!` / `resolveAgentId!`). The 1.5-era cursor and idempotence
# switches are gone — see the README. `trap`, `oplog-commit` and the resolvers
# live on the counter (the same commands the ABI harness drives against stubs);
# `fork` and `idempotency` need the effects agent's state and RPC.
control_agent="counter-agent(\"$RUN_ID-control\")"

# `trap!` never returns: the invocation fails with the reason, and Golem leaves
# the agent holding that failed invocation until an operator reverts it (which is
# why `revert-agent` stays a CLI operation and is not an effect), so this uses an
# agent id no other check touches.
check "trap: the invocation fails with the reason" \
    "guest-requested trap: $RUN_ID trap reason" \
    "$(invoke "counter-agent(\"$RUN_ID-trap\")" handle-message "$(message_arg "trap $RUN_ID trap reason")")"

check "oplog-commit: reaching the replicas returns" "Committed the oplog to 2 replicas" \
    "$(invoke "$control_agent" handle-message '"oplog-commit 2"')"

# Both resolvers name a component of this deployment — the counter's own,
# `roc-golem:counter` — and the agent id the second one returns has to carry the
# component id the first one resolved.
resolved_component="$(invoke "$control_agent" handle-message '"resolve-component-id roc-golem:counter"')"
component_id="${resolved_component#Resolved component: }"
check_true "resolve-component-id: a deployed component resolves to a UUID" \
    "$([[ "$component_id" =~ ^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$ ]] && echo true || echo false)"
resolved_agent="$(invoke "$control_agent" handle-message \
    "$(message_arg "resolve-agent-id roc-golem:counter counter-agent(\"$RUN_ID-control\")")")"
check "resolve-agent-id: the resolved agent carries the component id" \
    "\"componentId\":\"$component_id\"" "$resolved_agent"
check "resolve-agent-id: the name the app asked for comes back" \
    "\"agentId\":\"counter-agent(" "$resolved_agent"
check "resolve-component-id: a reference that names no component is reported" \
    "no component named \"roc-golem:nope\"" \
    "$(invoke "$control_agent" handle-message '"resolve-component-id roc-golem:nope"')"

# `fork!` copies the agent at the call: the invocation the CLI is waiting for
# answers on the original side, and the copy — a new agent under the same id plus
# the phantom id both sides are told — runs the rest of the handler too. That
# handler calls the counter, so the counter's count is what shows the copy really
# continued, and the copy can then be invoked under its own id.
fork_agent="effects-agent(\"$RUN_ID-fork\")"
fork_calls="counter-agent(\"$RUN_ID-fork-calls\")"

first_fork="$(invoke "$fork_agent" handle-message "$(message_arg "fork $fork_calls handle-message increment")")"
check "fork: the original side reports the fork" "original side" "$first_fork"
first_phantom="${first_fork#Forked copy }"
first_phantom="${first_phantom%% *}"
check_true "fork: the reported phantom id is a UUID" \
    "$([[ "$first_phantom" =~ ^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$ ]] && echo true || echo false)"
check "fork: the forked copy continued and called the target" "Current count is 1" \
    "$(poll_reply "$fork_calls" '"get"' "Current count is 1")"

second_fork="$(invoke "$fork_agent" handle-message "$(message_arg "fork $fork_calls handle-message increment")")"
second_phantom="${second_fork#Forked copy }"
second_phantom="${second_phantom%% *}"
check_true "fork: each fork gets its own phantom id" \
    "$([[ -n "$second_phantom" && "$second_phantom" != "$first_phantom" ]] && echo true || echo false)"
check "fork: the original's state carries the previous fork" \
    "previous fork: $first_phantom" "$second_fork"
check "fork: the second copy continued too" "Current count is 2" \
    "$(poll_reply "$fork_calls" '"get"' "Current count is 2")"
check "fork: the forked copy is a live agent instance" "effects-agent is up" \
    "$(invoke "effects-agent(\"$RUN_ID-fork\")[$first_phantom]" handle-message '"status"')"

# `idempotencyKey!` is durable per call site: Golem derives the key from the
# call's position in the invocation's oplog and records it, so a replay of that
# invocation hands the same keys back. Two calls in one invocation are two
# positions (their keys differ), a later invocation is a new key sequence, and a
# second CLI call under the same idempotency key replays the recorded answer.
idem_agent="effects-agent(\"$RUN_ID-idem\")"
idem_key="$RUN_ID-idem-key"
first_idem="$(invoke_with_key "$idem_key" "$idem_agent" handle-message '"idempotency"')"
first_key="${first_idem#*first=}"
first_key="${first_key%% *}"
check_true "idempotency: the first key is a UUID" \
    "$([[ "$first_key" =~ ^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$ ]] && echo true || echo false)"
check "idempotency: each call in an invocation is its own position" "agree=false" "$first_idem"
check_true "idempotency: the same idempotency key replays the recorded keys" \
    "$([[ "$(invoke_with_key "$idem_key" "$idem_agent" handle-message '"idempotency"')" == "$first_idem" ]] && echo true || echo false)"
later_idem="$(invoke "$idem_agent" handle-message '"idempotency"')"
second_key="${later_idem#*first=}"
second_key="${second_key%% *}"
check "idempotency: a later invocation is a new key sequence" \
    "previous-first=$first_key" "$later_idem"
check_true "idempotency: and its keys differ from the previous invocation's" \
    "$([[ -n "$second_key" && "$second_key" != "$first_key" ]] && echo true || echo false)"

say "==> Webhooks (promise + POST callback)"
# `createWebhook!` registers a promise and returns the URL that completes it.
# The app stores that URL in its own state, so the POST can arrive between
# invocations; the callback route lives on the deployment's domain under the
# deployment's `webhookUrl` prefix (default `/webhooks`) and the agent type's
# name (the mount's `webhook-suffix` is empty in this platform).
webhook_agent="effects-agent(\"$RUN_ID\")"
webhook_reply="$(invoke "$webhook_agent" handle-message '"webhook-create"')"
check "webhook: create returns a callback URL" "/webhooks/effects-agent/" "$webhook_reply"
# The URL is one long token, so the CLI's wrap can split it and the un-wrapping
# above joins the halves with a space; URLs never contain one.
webhook_url="${webhook_reply#Webhook created: }"
webhook_url="${webhook_url// /}"

# The URL carries the deployment's domain (`roc-golem.localhost:<port>`), which
# resolves to the listener; the request is a POST with a body, no auth.
check "webhook: the callback POST answers 204" "204" \
    "$(curl -s -o /dev/null -w '%{http_code}' -X POST --data 'webhook payload' "$webhook_url")"

check "webhook: awaiting the URL returns the POSTed body" "webhook payload" \
    "$(invoke_within 120 "$webhook_agent" handle-message '"webhook-await"')"

# The id in the URL is a checksummed blob, so an id that decodes but does not
# verify is a 404 — it must not complete some other promise. Flip a character
# deep in the trailing checksum (not the last one: that would change the
# padding bits and the id would fail to decode, which is a 400 instead).
webhook_id="${webhook_url##*/}"
flip_at=$(( ${#webhook_id} - 5 ))
replacement="A"
[[ "${webhook_id:$flip_at:1}" == "A" ]] && replacement="B"
tampered_id="${webhook_id:0:$flip_at}${replacement}${webhook_id:$((flip_at + 1))}"
check "webhook: an unverifiable id answers 404" "404" \
    "$(curl -s -o /dev/null -w '%{http_code}' -X POST -H "Host: $API_DOMAIN" \
        --data 'payload' "http://127.0.0.1:$API_PORT/webhooks/effects-agent/$tampered_id")"

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
# A router with a handler owns the misses: the mount's file mappings are tried
# first, and a path none of them match is handed to `handleRequest!` instead of
# answering 404 (a router without a handler still 404s, as it did before the
# handler existed; the example's own 404 is checked below).
check "router: an unmatched file path reaches the handler" "200" \
    "$(fetch_status "/static/assets/missing.txt")"
# The router's OpenAPI provider method supplies the document for the mount, and
# Golem rebases its paths under the mount prefix.
check "router: openapi provider reaches the deployment spec" "The uploaded index page" \
    "$(fetch "/openapi.yaml")"

# The dynamic handler: a request the static bindings do not match is handed to
# the app's `handleRequest!`, which answers with a status, headers and a body.
# Golem builds the response from the value tree the handler returns, and a
# bodyless answer (204 here) is complete.
check "router: dynamic handler answers a bodyless response" "204" \
    "$(fetch_status "/static/ping")"
check "router: dynamic handler owns its statuses" "404" \
    "$(fetch_status "/static/missing")"
check "router: an unmatched path reaches the handler" "200" \
    "$(fetch_status "/static/unmatched")"
check "router: dynamic handler's headers reach the client" "content-type: text/plain" \
    "$(fetch_headers "/static/unmatched")"
# The response body arrives too. It travels as `list<u8>` chunks, and each chunk
# is a value tree of its own — the first version pointed a chunk's list at the
# response tree's byte nodes, the runtime dropped every item and the client saw
# an empty body (golemcloud/golem#4291, fixed here in `body_chunks`).
check "router: the handler's response body reaches the client" \
    "roc-golem router: GET /static/echoed" \
    "$(fetch "/static/echoed")"
check "router: the handler reads the request body and answers with it" \
    "body='posted payload'" \
    "$(curl -s -H "Host: $API_DOMAIN" -X POST -d 'posted payload' \
        "http://127.0.0.1:$API_PORT/static/echoed")"
# The body is written in bounded chunks, so a large answer exercises more than
# one of them.
big_body_bytes="$(python3 -c "print('A'*100000, end='')" \
    | curl -s -H "Host: $API_DOMAIN" -X POST --data-binary @- \
        "http://127.0.0.1:$API_PORT/static/big-body" | wc -c)"
check_true "router: a 100 KB response body arrives in chunks" \
    "$([[ "$big_body_bytes" -gt 100000 ]] && echo true || echo false)"
# The request body reaches the app through the same minimal JSON field reader the
# tool arguments use, which stops at the first `"` — so a posted body containing
# a quote is truncated before the app sees it. Tracked in the README's dynamic
# router section and the parity doc; gone once that reader is replaced.
say "SKIP  router: a request body containing a double quote (the JSON field reader truncates it)"
skipped=$((skipped + 1))

say "==> Snapshot-based agent update (incompatible revision)"
# `counter_v2` adds a tool and a state field, so the agent type changes: Golem
# leaves an existing agent on its old revision, and `update … manual` carries the
# state across through this platform's save/load-snapshot exports. It is deployed
# from a manifest derived from this one, so it is the same component and agent
# type — just a later, incompatible revision.
update_agent="counter-agent(\"$RUN_ID-update\")"
invoke "$update_agent" handle-message '"increment"' >/dev/null
invoke "$update_agent" handle-message '"increment"' >/dev/null
check "update: state before the revision change" "Current count is 2" \
    "$(invoke "$update_agent" handle-message '"get"')"

v2_manifest="$WORK_DIR/golem_v2.yaml"
python3 - "$MANIFEST" "$v2_manifest" <<'PY'
import os
import sys

import yaml

source, target = sys.argv[1], sys.argv[2]
root = os.path.dirname(os.path.abspath(source))
app = yaml.safe_load(open(source))

# The derived manifest lives outside the app, so every path it carries has to be
# absolute: component directories (build commands run in them), the initial-file
# sources, and the counter's wasm.
for component in app["components"].values():
    component["dir"] = os.path.join(root, component["dir"])
for agent in app["agents"].values():
    for entry in agent.get("files") or []:
        entry["sourcePath"] = os.path.join(root, entry["sourcePath"])

component = app["components"]["roc-golem:counter"]
component["componentWasm"] = "../../build/counter_v2_agent.wasm"
component["build"] = [{"command": "../../tooling/build.sh counter_v2"}]
app["tools"]["set-note"] = {"component": "roc-golem:counter"}
app["agents"]["counter-agent"]["tools"]["set-note"] = {}
yaml.safe_dump(app, open(target, "w"), sort_keys=False)
PY
v2_deploy="$("$CLI" $CLI_EXTRA -L -A "$v2_manifest" deploy --yes 2>&1 || true)"
check_true "update: the incompatible revision deployed" \
    "$(grep -qE 'Finished deploying|no changes required' <<<"$v2_deploy" && echo true || echo false)"

update_output="$("$CLI" $CLI_EXTRA -L -A "$MANIFEST" -Y agent update "$update_agent" manual 2>&1 || true)"
check_true "update: snapshot-based update applied" \
    "$(grep -qiE 'updated|succeeded|finished' <<<"$update_output" && echo true || echo false)"
check "update: state carried across the incompatible revision" "Current count is 2" \
    "$(invoke "$update_agent" handle-message '"get"')"
# The snapshot hands the *older* revision's state JSON across unchanged, so the
# new revision migrates it — which is exactly what the fixture demonstrates.
check "update: the new revision migrated the carried state" "Note is migrated" \
    "$(invoke "$update_agent" handle-message '"note"')"
check "update: a fresh agent on the new revision has its own default" "Note is v2" \
    "$(invoke "counter-agent(\"$RUN_ID-update-fresh\")" handle-message '"note"')"

# Leave the deployment on the app's own component revision.
"$CLI" $CLI_EXTRA -L -A "$MANIFEST" deploy --yes >/dev/null 2>&1 || true

say ""
if [[ "$failures" -eq 0 ]]; then
    if [[ "$skipped" -gt 0 ]]; then
        say "All end-to-end checks passed ($skipped skipped)."
    else
        say "All end-to-end checks passed."
    fi
else
    say "$failures end-to-end check(s) failed."
    exit 1
fi
