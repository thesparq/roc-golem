#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_DIR="$ROOT_DIR/build"
DIST_DIR="$ROOT_DIR/dist"
HOST_DIR="$ROOT_DIR/host"
PLATFORM_DIR="$ROOT_DIR/platform"
PACK_DIR="$ROOT_DIR/tooling/pack_platform"
PRECOMPILED_HOST_A="$BUILD_DIR/libhost.a"
TARGET_HOST_A="$PLATFORM_DIR/targets/wasm32/libhost.a"

mkdir -p "$BUILD_DIR"
mkdir -p "$DIST_DIR"

# The platform's hand-written Roc ABI (`host/src/roc_std.rs`, `tooling/glue`)
# is calibrated against one Roc nightly: the layout of `Str`/`List`/`Try` and
# the allocator contract move between nightlies, and a mismatched compiler
# yields components that build fine and then trap at runtime (a `Str` dropped
# through the wrong deallocator) instead of failing here. Refuse to build with
# anything else; set ROC_ALLOW_VERSION_MISMATCH=1 to override.
ROC_TAG="${ROC_TAG:-nightly-2026-10-19-601b54c}"

check_roc_version() {
    local version
    version="$(roc version 2>/dev/null | head -n 1)"
    if [[ "$version" == *"${ROC_TAG#nightly-}"* ]]; then
        return 0
    fi
    echo "error: the roc on PATH is '${version:-unknown}', but this platform is" >&2
    echo "       pinned to $ROC_TAG (see README). Components built with another" >&2
    echo "       nightly can trap at runtime. Set ROC_ALLOW_VERSION_MISMATCH=1 to" >&2
    echo "       build anyway." >&2
    if [[ "${ROC_ALLOW_VERSION_MISMATCH:-0}" != "1" ]]; then
        exit 1
    fi
}

build_platform() {
    echo "============================================================"
    echo " [Phase: Platform Build] Pre-compiling Rust Host"
    echo "============================================================"
    if ! command -v cargo &> /dev/null; then
        echo "Error: cargo is required for platform build." >&2
        exit 1
    fi

    echo "==> Compiling Rust Host for WebAssembly target (wasm32-unknown-unknown)..."
    cargo build --manifest-path "$HOST_DIR/Cargo.toml" --target wasm32-unknown-unknown --release

    mkdir -p "$PLATFORM_DIR/targets/wasm32"
    cp "$HOST_DIR/target/wasm32-unknown-unknown/release/libroc_golem_host.a" "$TARGET_HOST_A"
    cp "$HOST_DIR/target/wasm32-unknown-unknown/release/libroc_golem_host.a" "$PLATFORM_DIR/wasm32.a"
    cp "$HOST_DIR/target/wasm32-unknown-unknown/release/libroc_golem_host.a" "$PLATFORM_DIR/libhost.a"
    cp "$HOST_DIR/target/wasm32-unknown-unknown/release/libroc_golem_host.a" "$PRECOMPILED_HOST_A"

    echo "==> Platform pre-compiled successfully:"
    echo "    Static Library: $TARGET_HOST_A"
}

build_app() {
    local TARGET_APP="${1:-counter}"
    local APP_FILE=""

    check_roc_version

    if [[ -f "$TARGET_APP" ]]; then
        APP_FILE="$TARGET_APP"
        TARGET_APP="$(basename "$(dirname "$APP_FILE")")"
        if [[ "$TARGET_APP" == "." ]]; then
            TARGET_APP="app"
        fi
    elif [[ -f "$ROOT_DIR/examples/$TARGET_APP/main.roc" ]]; then
        APP_FILE="$ROOT_DIR/examples/$TARGET_APP/main.roc"
    else
        echo "Error: Could not find application at '$TARGET_APP' or 'examples/$TARGET_APP/main.roc'" >&2
        exit 1
    fi

    local RAW_WASM="$BUILD_DIR/${TARGET_APP}_raw.wasm"
    local OUTPUT_COMPONENT="$BUILD_DIR/${TARGET_APP}_agent.wasm"

    echo "============================================================"
    echo " [Phase: App Build] Building Roc Golem Agent: $TARGET_APP"
    echo "============================================================"

    # Ensure pre-compiled host exists
    if [[ ! -f "$TARGET_HOST_A" ]]; then
        echo "==> Pre-compiled host not found. Building platform host first..."
        build_platform
    fi

    # Step 1: Format & validate Roc code
    if command -v roc &> /dev/null; then
        echo "==> Validating Roc Agent format ($APP_FILE)..."
        roc fmt --check "$APP_FILE" "$PLATFORM_DIR/main.roc" 2>/dev/null || roc fmt "$APP_FILE" "$PLATFORM_DIR/main.roc" 2>/dev/null || true
    fi

    # Step 2: Compile Roc application and link with the Rust platform host
    echo "==> Compiling and linking Roc app with Rust Host..."
    set +e
    roc build --target=wasm32 --output="$RAW_WASM" "$APP_FILE"
    local ROC_STATUS=$?
    set -e
    if [[ $ROC_STATUS -ne 0 && $ROC_STATUS -ne 2 ]]; then
        echo "Error: Roc build failed with status $ROC_STATUS" >&2
        exit "$ROC_STATUS"
    fi

    # Step 3: Componentize WASM module into Golem Component Model
    echo "==> Componentizing WASM module into Golem Component Model..."
    wasm-tools component new "$RAW_WASM" -o "$OUTPUT_COMPONENT"

    echo "==> Validating Component Model compatibility..."
    # `--features all` is needed from wasm-tools 1.248 on: components that use
    # `stream` types (the tool/guest surface) fail validation without the
    # component-model async feature enabled.
    wasm-tools validate --features all "$OUTPUT_COMPONENT"

    echo "============================================================"
    echo " App build finished successfully!"
    echo " Component: $OUTPUT_COMPONENT"
    echo " Size:      $(du -h "$OUTPUT_COMPONENT" | cut -f1)"
    echo " Deploy:    golem -L deploy            (see README; golem.yaml drives it)"
    echo " Check:     cargo run --release --manifest-path tooling/abi_harness/Cargo.toml"
    echo "            -- $OUTPUT_COMPONENT --type <agent-type> --method <tool>"
    echo "============================================================"
}

package_platform() {
    local TAG="${1:-v0.1.0}"
    local REPO="${2:-${GITHUB_REPOSITORY:-thesparq/roc-golem}}"

    build_platform

    echo "============================================================"
    echo " [Phase: Platform Package] Packaging Release Artifacts"
    echo " Tag:        $TAG"
    echo " Repository: $REPO"
    echo "============================================================"

    cargo run --manifest-path "$PACK_DIR/Cargo.toml" --release -- \
        --platform-dir "$PLATFORM_DIR" \
        --build-dir "$BUILD_DIR" \
        --out-dir "$DIST_DIR" \
        --tag "$TAG" \
        --repo "$REPO"
}

COMMAND="${1:-all}"

case "$COMMAND" in
    platform)
        build_platform
        ;;
    app)
        shift || true
        build_app "${1:-counter}"
        ;;
    package)
        shift || true
        package_platform "${1:-v0.1.0}" "${2:-${GITHUB_REPOSITORY:-thesparq/roc-golem}}"
        ;;
    release)
        shift || true
        build_platform
        build_app "counter"
        build_app "ai_tool"
        build_app "streaming_agent"
        build_app "effects"
        build_app "site"
        build_app "static_site"
        package_platform "${1:-v0.1.0}" "${2:-${GITHUB_REPOSITORY:-thesparq/roc-golem}}"
        ;;
    counter|ai_tool|streaming_agent|effects|site|static_site)
        build_app "$COMMAND"
        ;;
    all)
        build_platform
        build_app "counter"
        build_app "ai_tool"
        build_app "streaming_agent"
        build_app "effects"
        build_app "site"
        build_app "static_site"
        package_platform "v0.1.0" "${GITHUB_REPOSITORY:-thesparq/roc-golem}"
        ;;
    *)
        if [[ -f "$COMMAND" ]]; then
            build_app "$COMMAND"
        else
            echo "Usage: $0 [platform | app <name_or_file> | package <tag> <repo> | release <tag> <repo> | counter | ai_tool | streaming_agent | effects | site | static_site | all]" >&2
            exit 1
        fi
        ;;
esac
