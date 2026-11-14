#!/usr/bin/env bash
# ==============================================================================
# test_consumer.sh - Automated consumer verification for packaged Roc Golem platform
# ==============================================================================
# Verifies that an external consumer can unpack a packaged platform archive
# (dist/roc-golem-*.tar.zst), compile a Roc application against it, and produce
# a valid Golem WebAssembly Component conforming to the component model.
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

ARCHIVE="${1:-$(ls -t "$ROOT_DIR/dist/roc-golem-"*.tar.zst 2>/dev/null | head -1)}"
if [[ -z "$ARCHIVE" || ! -f "$ARCHIVE" ]]; then
    echo "==> No archive found in dist/. Building platform package first..."
    "$SCRIPT_DIR/build.sh" package
    ARCHIVE=$(ls -t "$ROOT_DIR/dist/roc-golem-"*.tar.zst 2>/dev/null | head -1)
fi

echo "=============================================================="
echo " [Consumer Smoke Test] Testing Platform Package: $(basename "$ARCHIVE")"
echo "=============================================================="

if [[ -z "$ARCHIVE" || ! -f "$ARCHIVE" ]]; then
    echo "Error: Archive '$ARCHIVE' does not exist." >&2
    exit 1
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT INT TERM

PLATFORM_DIR="$TMP_DIR/platform"
APP_DIR="$TMP_DIR/app"

mkdir -p "$PLATFORM_DIR" "$APP_DIR"

echo "==> Step 1: Unpacking archive to isolated directory..."
tar --zstd -xf "$ARCHIVE" -C "$PLATFORM_DIR"

echo "==> Step 2: Verifying critical target static libraries..."
if [[ ! -f "$PLATFORM_DIR/targets/wasm32/libroc_golem_host.a" ]]; then
    echo "Error: targets/wasm32/libroc_golem_host.a is missing from unpacked archive!" >&2
    exit 1
fi
echo "    Found targets/wasm32/libroc_golem_host.a ($(wc -c < "$PLATFORM_DIR/targets/wasm32/libroc_golem_host.a") bytes)"

if [[ ! -f "$PLATFORM_DIR/targets/wasm32/libc.a" ]]; then
    echo "Error: targets/wasm32/libc.a is missing from unpacked archive!" >&2
    exit 1
fi
echo "    Found targets/wasm32/libc.a ($(wc -c < "$PLATFORM_DIR/targets/wasm32/libc.a") bytes)"

echo "==> Step 3: Authoring minimal consumer Roc agent..."
cat << "EOF" > "$APP_DIR/main.roc"
app [agent] { pf: platform "../platform/main.roc" }

import pf.Golem exposing [Agent, defineAgent]

State : { count : I64 }

agent : Agent(State)
agent = defineAgent({
	init: |_config|
		Ok({ count: 0.I64 }),

	handleMessage: |state, message|
		match message {
			"get" =>
				Ok({
					state,
					response: "Current count is ${Str.inspect(state.count)}",
				}),

			_ =>
				Ok({
					state,
					response: "Unknown command",
				}),
		},

	handleToolCall: |state, toolCall|
		Ok({
			state,
			result: { id: toolCall.id, success: True, output: "{}" },
		}),

	metadata: {
		name: "consumer-smoke-agent",
		version: "1.0.0",
		description: "Consumer agent smoke test verifying packaged platform distribution",
		tools: [],
	},

	serializeState: |state|
		"{\"count\":${Str.inspect(state.count)}}",

	deserializeState: |_stateJson|
		Ok({ count: 0.I64 }),
})
EOF

echo "==> Step 4: Running roc check on consumer application..."
roc check "$APP_DIR/main.roc" || [ $? -eq 2 ]

echo "==> Step 5: Compiling consumer application to core WebAssembly module..."
roc build --target=wasm32 "$APP_DIR/main.roc" --output="$APP_DIR/core.wasm" || {
    STATUS=$?
    if [[ $STATUS -ne 2 || ! -f "$APP_DIR/core.wasm" ]]; then
        echo "Error: roc build failed with status $STATUS" >&2
        exit $STATUS
    fi
}

if [[ ! -f "$APP_DIR/core.wasm" ]]; then
    echo "Error: Expected core.wasm to be created at '$APP_DIR/core.wasm'" >&2
    exit 1
fi
echo "    Generated core.wasm ($(wc -c < "$APP_DIR/core.wasm") bytes)"

echo "==> Step 6: Componentizing core module into Golem Component Model..."
wasm-tools component new "$APP_DIR/core.wasm" -o "$APP_DIR/agent.wasm"

echo "==> Step 7: Validating final WebAssembly component..."
wasm-tools validate "$APP_DIR/agent.wasm"

echo "=============================================================="
echo " [Consumer Smoke Test] SUCCESS: Packaged platform fully operational!"
echo "=============================================================="
