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

PINNED_ROC_VERSION="nightly-2026-09-29-7f11a82"

mkdir -p "$BUILD_DIR"
mkdir -p "$DIST_DIR"

verify_toolchain() {
    echo "==> Verifying build toolchain..."
    if ! command -v cargo &> /dev/null; then
        echo "Error: 'cargo' is required but not installed or not in PATH." >&2
        exit 1
    fi

    if ! command -v rustc &> /dev/null; then
        echo "Error: 'rustc' is required but not installed or not in PATH." >&2
        exit 1
    fi

    if command -v rustup &> /dev/null; then
        if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
            echo "Error: Rust target 'wasm32-unknown-unknown' is not installed." >&2
            echo "Install it via: rustup target add wasm32-unknown-unknown" >&2
            exit 1
        fi
    fi

    if ! command -v wasm-tools &> /dev/null; then
        echo "Error: 'wasm-tools' is required but not installed or not in PATH." >&2
        echo "Install it via: cargo install wasm-tools" >&2
        exit 1
    fi

    if ! command -v wasm-ld &> /dev/null; then
        echo "Error: 'wasm-ld' is required but not installed or not in PATH." >&2
        echo "Install it via your system LLVM/lld package (e.g. apt install lld)." >&2
        exit 1
    fi

    if ! command -v roc &> /dev/null; then
        echo "Error: 'roc' is required but not installed or not in PATH." >&2
        echo "Download the pinned nightly ($PINNED_ROC_VERSION) from:" >&2
        echo "  https://github.com/roc-lang/nightlies/releases/download/$PINNED_ROC_VERSION/roc_nightly-linux_x86_64-$PINNED_ROC_VERSION.tar.gz" >&2
        exit 1
    fi

    local ROC_VER
    ROC_VER="$(roc version 2>&1 || true)"
    if [[ "$ROC_VER" != *"$PINNED_ROC_VERSION"* ]]; then
        echo "Error: 'roc' version mismatch." >&2
        echo "  Expected: $PINNED_ROC_VERSION" >&2
        echo "  Found:    $ROC_VER" >&2
        echo "Download the pinned nightly from:" >&2
        echo "  https://github.com/roc-lang/nightlies/releases/download/$PINNED_ROC_VERSION/roc_nightly-linux_x86_64-$PINNED_ROC_VERSION.tar.gz" >&2
        exit 1
    fi
}

check_toolchain() {
    verify_toolchain
}

build_platform() {
    echo "============================================================"
    echo " [Phase: Platform Build] Pre-compiling Rust Host"
    echo "============================================================"
    if ! command -v cargo &> /dev/null; then
        echo "Error: cargo is required for platform build." >&2
        exit 1
    fi

    if command -v rustup &> /dev/null; then
        if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
            echo "Error: Rust target 'wasm32-unknown-unknown' is not installed." >&2
            echo "Install it via: rustup target add wasm32-unknown-unknown" >&2
            exit 1
        fi
    fi

    echo "==> Compiling Rust Host for WebAssembly target (wasm32-unknown-unknown)..."
    cargo build --manifest-path "$HOST_DIR/Cargo.toml" --target wasm32-unknown-unknown --release --no-default-features

    cp "$HOST_DIR/target/wasm32-unknown-unknown/release/libroc_golem_host.a" "$PRECOMPILED_HOST_A"

    echo "==> Platform pre-compiled successfully:"
    echo "    Static Library: $PRECOMPILED_HOST_A"
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

    verify_toolchain

    local OUTPUT_COMPONENT="$BUILD_DIR/${TARGET_APP}_agent.wasm"
    local APP_OBJ="$BUILD_DIR/${TARGET_APP}.o"
    local CORE_WASM="$BUILD_DIR/${TARGET_APP}_core.wasm"

    echo "============================================================"
    echo " [Phase: App Build] Building Roc Golem Agent: $TARGET_APP"
    echo "============================================================"

    # Ensure pre-compiled host exists
    if [[ ! -f "$PRECOMPILED_HOST_A" ]]; then
        echo "==> Pre-compiled host static library not found. Building platform host first..."
        build_platform
    fi

    # Step 1: Format & validate Roc code
    echo "==> Formatting Roc code ($APP_FILE)..."
    roc fmt "$APP_FILE" "$PLATFORM_DIR/main.roc"

    # Step 2: Compile Roc code to WebAssembly object file
    echo "==> Compiling Roc application to WebAssembly object ($APP_OBJ)..."
    roc build --target wasm32 --no-link "$APP_FILE" --output "$APP_OBJ"

    # Step 3: Link Roc object and host static library into core WebAssembly module
    echo "==> Linking core WebAssembly module with wasm-ld ($CORE_WASM)..."
    wasm-ld \
        --no-entry \
        --export-dynamic \
        --allow-undefined \
        "$APP_OBJ" \
        "$PRECOMPILED_HOST_A" \
        -o "$CORE_WASM"

    # Step 4: Componentize WASM module into Golem Component Model
    echo "==> Componentizing WASM module into Golem Component Model ($OUTPUT_COMPONENT)..."
    wasm-tools component new "$CORE_WASM" -o "$OUTPUT_COMPONENT"

    # Step 5: Validate Component Model compatibility
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
        build_app "canary_release"
        package_platform "${1:-v0.1.0}" "${2:-${GITHUB_REPOSITORY:-thesparq/roc-golem}}"
        ;;
    counter|ai_tool|streaming_agent|canary_release)
        build_app "$COMMAND"
        ;;
    all)
        build_platform
        build_app "counter"
        build_app "ai_tool"
        build_app "streaming_agent"
        build_app "canary_release"
        package_platform "v0.1.0" "${GITHUB_REPOSITORY:-thesparq/roc-golem}"
        ;;
    *)
        if [[ -f "$COMMAND" ]]; then
            build_app "$COMMAND"
        else
            echo "Usage: $0 [platform | app <name_or_file> | package <tag> <repo> | release <tag> <repo> | counter | ai_tool | streaming_agent | canary_release | all]"
            exit 1
        fi
        ;;
esac
