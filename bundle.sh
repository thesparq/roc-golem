#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
OUTDIR="${OUTDIR:-${ROOT}/out}"
HOST_DIR="${ROOT}/host"
PLATFORM_DIR="${ROOT}/platform"
TARGETS_DIR="${PLATFORM_DIR}/targets/wasm32"
WIT_DIR="${ROOT}/wit"
APP_DIR="${ROOT}/app"
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

mkdir -p "$OUTDIR" "$TARGETS_DIR"

# Step 1: Build Rust host (wasm32-unknown-unknown)
echo "==> Building Rust host (wasm32-unknown-unknown --release)"
(cd "$HOST_DIR" && cargo build --target=wasm32-unknown-unknown --release 2>&1)

# Step 2: Extract host object from static library archive
echo "==> Extracting host object"
HOST_A="$HOST_DIR/target/wasm32-unknown-unknown/release/libgolem_host.a"
if [ -f "$HOST_A" ]; then
  TMP_EXTRACT="$(mktemp -d)"
  (cd "$TMP_EXTRACT" && ar x "$HOST_A" 2>/dev/null)
  HOST_OBJ=$(find "$TMP_EXTRACT" -name "*golem_host*.o" 2>/dev/null | head -1)
  if [ -z "${HOST_OBJ:-}" ]; then
    echo "  (golem_host .o not found, trying any .o)"
    HOST_OBJ=$(find "$TMP_EXTRACT" -name "*.o" 2>/dev/null | head -1)
  fi
fi
if [ -z "${HOST_OBJ:-}" ]; then
  echo "ERROR: No host object file found in $HOST_A"
  exit 1
fi
cp "$HOST_OBJ" "$TARGETS_DIR/host.wasm"
echo "  -> host object placed at $TARGETS_DIR/host.wasm (from $HOST_OBJ)"

# Step 3: Create platform source bundle
echo "==> Creating platform bundle"
BUNDLE_OUTPUT=$(roc bundle "$PLATFORM_DIR/main.roc" --output-dir "$OUTDIR" 2>&1)
echo "$BUNDLE_OUTPUT"

BUNDLE_FILE=$(echo "$BUNDLE_OUTPUT" | grep -oE '[A-Za-z0-9_-]+\.tar\.zst' | head -1)
if [ -n "$BUNDLE_FILE" ]; then
  mv "$OUTDIR/$BUNDLE_FILE" "$OUTDIR/platform-bundle.tar.zst" 2>/dev/null || true
fi

# Step 4: Build Roc app → linked WASM (host + app)
echo "==> Building Roc app (--target=wasm32 --opt=speed)"
roc build --target=wasm32 --opt=speed "$APP_DIR/main.roc" --output="$TMPDIR/stage1.wasm"

# Step 5: Create env stub for any unresolved core wasm imports
echo "==> Creating env stub"
cat > "$TMPDIR/env-stub.wat" << WAT
(module
  (func \$slice_index_fail (param i32 i32 i32 i32)
    unreachable
  )
  (export "_RNvNtNtCse6q680yZGje_4core5slice5index16slice_index_fail" (func \$slice_index_fail))
)
WAT
wasm-tools parse "$TMPDIR/env-stub.wat" -o "$TMPDIR/env-stub.wasm"

# Step 6: Embed WIT metadata
echo "==> Embedding WIT metadata"
wasm-tools component embed "$WIT_DIR" "$TMPDIR/stage1.wasm" -o "$TMPDIR/stage2.wasm"

# Step 7: Wrap as WASM component (with env stub)
echo "==> Creating component"
wasm-tools component new "$TMPDIR/stage2.wasm" --adapt env="$TMPDIR/env-stub.wasm" -o "$OUTDIR/golem-component.wasm"

# Step 8: Validate
echo "==> Validating"
wasm-tools validate "$OUTDIR/golem-component.wasm"

echo ""
echo "======= DONE ======="
echo "Platform bundle:      $OUTDIR/platform-bundle.tar.zst"
echo "Demo component:       $OUTDIR/golem-component.wasm"
ls -lh "$OUTDIR/" 2>/dev/null
echo "===================="
