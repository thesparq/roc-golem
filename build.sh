#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
OUTDIR="${OUTDIR:-${ROOT}/out}"
WIT_DIR="${ROOT}/wit"
APP_DIR="${ROOT}/app"
HOST_DIR="${ROOT}/host"
PLATFORM_DIR="${ROOT}/platform"
TARGETS_DIR="${PLATFORM_DIR}/targets/wasm32"
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

mkdir -p "$OUTDIR" "$TARGETS_DIR"

# Step 1: Build Rust host (staticlib → .a, no linking step)
echo "==> Building Rust host (wasm32-unknown-unknown)"
(cd "$HOST_DIR" && cargo build --target=wasm32-unknown-unknown --release 2>&1) || true

# Step 2: Extract host object from static library archive
echo "==> Extracting host object"
HOST_A="$HOST_DIR/target/wasm32-unknown-unknown/release/libgolem_host.a"
if [ -f "$HOST_A" ]; then
  TMP_EXTRACT="$(mktemp -d)"
  (cd "$TMP_EXTRACT" && ar x "$HOST_A" 2>/dev/null)
  HOST_OBJ=$(find "$TMP_EXTRACT" -name "*golem_host*.o" -not -name "*.rmeta" 2>/dev/null | head -1)
  if [ -z "${HOST_OBJ:-}" ]; then
    HOST_OBJ=$(find "$TMP_EXTRACT" -name "*.o" 2>/dev/null | head -1)
  fi
fi
if [ -z "${HOST_OBJ:-}" ]; then
  HOST_OBJ=$(find "$HOST_DIR/target/wasm32-unknown-unknown/release" -name "*golem_host*.o" 2>/dev/null | head -1)
fi
if [ -z "${HOST_OBJ:-}" ]; then
  echo "ERROR: No host object file found!"
  exit 1
fi
cp "$HOST_OBJ" "$TARGETS_DIR/host.wasm"
echo "  -> host object placed at $TARGETS_DIR/host.wasm"

# Step 3: Build Roc app → linked WASM (host + app)
echo "==> Building Roc app (--target=wasm32 --opt=speed)"
roc build --target=wasm32 --opt=speed "$APP_DIR/main.roc" \
  --output="$TMPDIR/stage1.wasm"

# Step 4: Embed WIT metadata
echo "==> Embedding WIT metadata"
wasm-tools component embed "$WIT_DIR" "$TMPDIR/stage1.wasm" \
  -o "$TMPDIR/stage2.wasm"

# Step 5: Wrap as WASM component
echo "==> Creating component"
wasm-tools component new "$TMPDIR/stage2.wasm" \
  -o "$OUTDIR/golem-component.wasm"

# Step 6: Validate
echo "==> Validating"
wasm-tools validate "$OUTDIR/golem-component.wasm"

echo ""
echo "======= DONE ======="
echo "Component: $OUTDIR/golem-component.wasm"
ls -lh "$OUTDIR/golem-component.wasm"
echo "===================="
