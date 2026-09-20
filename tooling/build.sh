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
PRECOMPILED_HOST_WASM="$BUILD_DIR/host.wasm"

mkdir -p "$BUILD_DIR"
mkdir -p "$DIST_DIR"

build_platform() {
    echo "============================================================"
    echo " [Phase: Platform Build] Pre-compiling Rust Host"
    echo "============================================================"
    if ! command -v cargo &> /dev/null; then
        echo "Error: cargo is required for platform build." >&2
        exit 1
    fi

    echo "==> Compiling Rust Host for WebAssembly target (wasm32-unknown-unknown)..."
    cd "$HOST_DIR"
    cargo build --target wasm32-unknown-unknown --release

    cp "$HOST_DIR/target/wasm32-unknown-unknown/release/libroc_golem_host.a" "$PRECOMPILED_HOST_A"
    cp "$HOST_DIR/target/wasm32-unknown-unknown/release/roc_golem_host.wasm" "$PRECOMPILED_HOST_WASM"

    echo "==> Platform pre-compiled successfully:"
    echo "    Static Library: $PRECOMPILED_HOST_A"
    echo "    Core Module:    $PRECOMPILED_HOST_WASM"
}

build_app() {
    local TARGET_APP="${1:-counter}"
    local APP_FILE=""

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

    local OUTPUT_COMPONENT="$BUILD_DIR/${TARGET_APP}_agent.wasm"

    echo "============================================================"
    echo " [Phase: App Build] Building Roc Golem Agent: $TARGET_APP"
    echo "============================================================"

    # Ensure pre-compiled host exists
    if [[ ! -f "$PRECOMPILED_HOST_WASM" ]]; then
        echo "==> Pre-compiled host not found. Building platform host first..."
        build_platform
    fi

    # Step 1: Format & validate Roc code
    if command -v roc &> /dev/null; then
        echo "==> Validating Roc Agent format ($APP_FILE)..."
        roc fmt --check "$APP_FILE" "$PLATFORM_DIR/main.roc"
    fi

    # Step 2: Componentize WASM module with generic Golem WIT interfaces
    echo "==> Componentizing WASM module into Golem Component Model..."
    wasm-tools component new "$PRECOMPILED_HOST_WASM" -o "$OUTPUT_COMPONENT"

    echo "==> Validating Component Model compatibility..."
    wasm-tools validate "$OUTPUT_COMPONENT"

    echo "============================================================"
    echo " App build finished successfully!"
    echo " Component: $OUTPUT_COMPONENT"
    echo " Size:      $(du -h "$OUTPUT_COMPONENT" | cut -f1)"
    echo " Deploy:    golem component add --component-name $TARGET_APP $OUTPUT_COMPONENT"
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
        package_platform "${1:-v0.1.0}" "${2:-${GITHUB_REPOSITORY:-thesparq/roc-golem}}"
        ;;
    counter|ai_tool|streaming_agent)
        build_app "$COMMAND"
        ;;
    all)
        build_platform
        build_app "counter"
        build_app "ai_tool"
        build_app "streaming_agent"
        package_platform "v0.1.0" "${GITHUB_REPOSITORY:-thesparq/roc-golem}"
        ;;
    *)
        if [[ -f "$COMMAND" ]]; then
            build_app "$COMMAND"
        else
            echo "Usage: $0 [platform | app <name_or_file> | package <tag> <repo> | release <tag> <repo> | counter | ai_tool | streaming_agent | all]"
            exit 1
        fi
        ;;
esac
