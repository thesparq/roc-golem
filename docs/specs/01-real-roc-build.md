# Specification: Unit 01 — Real Roc Build

- **Unit:** 01
- **File:** `docs/specs/01-real-roc-build.md`
- **Status:** Implemented, awaiting verification
- **Fixes:** `KI-01`, `KI-02`, part of `KI-12`

---

## 1. Goal

Enable real Roc compilation and linking in `tooling/build.sh` so that user Roc applications are genuinely compiled to WebAssembly, linked against a stub-free Rust host static library (`libhost.a`), and componentized into distinct Golem WebAssembly agents (`*_agent.wasm`).

---

## 2. Design

### Context & Problem
Currently, `./tooling/build.sh app` only executes `roc fmt` and then directly passes the precompiled `host.wasm` into `wasm-tools component new`. Because `host/Cargo.toml` enables `stub-guest` by default, the resulting WebAssembly component is simply the Rust host with a hardcoded mock guest (`guest_bridge.rs`). Every generated component is byte-for-byte identical, and no Roc application code is ever compiled or executed. Furthermore, the distributed `libhost.a` contains the mock symbols, which clash with real Roc application entry points.

### Structural Decisions
1. **Decouple Stubs from Host (`host/Cargo.toml`):**
   - Change `default = ["stub-guest"]` to `default = []`.
   - The fake agent in `host/src/guest_bridge.rs` is retained strictly as an opt-in feature for native host unit tests (`cargo test --features stub-guest --lib`).
   - The production host library (`libhost.a`) is compiled with `--no-default-features`, leaving the four `roc__main_*` symbols undefined in the host library archive so they can be provided by the compiled Roc application object.

2. **Toolchain Enforcement (`tooling/build.sh`):**
   - The build script verifies that `roc` is installed.
   - It checks that `roc version` matches the pinned toolchain declared in `docs/architecture.md`: `nightly-2026-09-29-7f11a82`.
   - If missing or mismatched, it exits with a descriptive error message instructing how to download the pinned release asset.

3. **Real Compilation & Linking Pipeline (`tooling/build.sh`):**
   - **Step 1 (Precompile Clean Host):** `cargo build --manifest-path host/Cargo.toml --target wasm32-unknown-unknown --release --no-default-features` generates `build/libhost.a`.
   - **Step 2 (Compile Roc Application):** `roc build --target wasm32 --no-link "$APP_FILE"` produces a compiled WebAssembly object file (`$APP_OBJ`).
   - **Step 3 (Link Core Module):** Link `$APP_OBJ` and `build/libhost.a` using `wasm-ld` (or Roc's built-in linker) to produce `$BUILD_DIR/${TARGET_APP}.wasm`.
   - **Step 4 (Componentize):** `wasm-tools component new "$CORE_WASM" -o "$OUTPUT_COMPONENT"` with the WIT definitions from `wit/`.
   - **Step 5 (Validate):** `wasm-tools validate "$OUTPUT_COMPONENT"`.

4. **Component Differentiation:**
   - Because user code is now linked, `counter_agent.wasm`, `ai_tool_agent.wasm`, `streaming_agent_agent.wasm`, and `canary_release_agent.wasm` will have distinct SHA-256 hashes and contain the actual Roc entry points and string data.

---

## 3. Implementation Details

### 3.1 Host Crate Configuration (`host/Cargo.toml`)
- Update `[features]`:
  ```toml
  [features]
  default = []
  stub-guest = []
  ```
- Adjust `[lib]` crate-types or target-specific settings so that `cdylib` is only built for `wasm32-unknown-unknown`, preventing GNU `ld` version-script symbol mangling errors during native Linux `cargo test`.
- Update native test invocations to pass `--features stub-guest --lib`.

### 3.2 Pre-compiled Platform Build (`tooling/build.sh:build_platform`)
- In `build_platform()`:
  - Run `cargo build --manifest-path "$HOST_DIR/Cargo.toml" --target wasm32-unknown-unknown --release --no-default-features`.
  - Copy `target/wasm32-unknown-unknown/release/libroc_golem_host.a` to `$PRECOMPILED_HOST_A`.
  - Ensure that `libhost.a` does **not** contain `roc__main_*` symbols from `guest_bridge.rs`.

### 3.3 Toolchain Verification (`tooling/build.sh:verify_toolchain`)
- Define `PINNED_ROC_VERSION="nightly-2026-09-29-7f11a82"`.
- Implement `check_toolchain()`:
  - Verify `cargo`, `rustc`, `wasm-tools`, `wasm-ld`, and `roc` exist.
  - Check that `roc version` contains or matches `$PINNED_ROC_VERSION`.
  - If `roc` is missing: fail with exit code 1 and URL to `roc-lang/nightlies` asset.

### 3.4 Application Compilation & Linking (`tooling/build.sh:build_app`)
- In `build_app()`:
  - Ensure `libhost.a` exists.
  - Compile Roc code to object file:
    ```bash
    roc build --target wasm32 --no-link "$APP_FILE" --output "$BUILD_DIR/${TARGET_APP}.o"
    ```
  - Link with `wasm-ld`:
    ```bash
    wasm-ld \
        --no-entry \
        --export-dynamic \
        --allow-undefined \
        "$BUILD_DIR/${TARGET_APP}.o" \
        "$PRECOMPILED_HOST_A" \
        -o "$BUILD_DIR/${TARGET_APP}_core.wasm"
    ```
  - Componentize core module:
    ```bash
    wasm-tools component new "$BUILD_DIR/${TARGET_APP}_core.wasm" \
        -o "$OUTPUT_COMPONENT"
    ```
  - Validate component:
    ```bash
    wasm-tools validate "$OUTPUT_COMPONENT"
    ```

### 3.5 Host Tests (`host/src/lib.rs`)
- Verify that `cargo test --manifest-path host/Cargo.toml --features stub-guest --lib` runs all 13 unit tests cleanly against the mock bridge.

---

## 4. Dependencies

- **Prior Units:** None (Unit 01 is the first implementation unit).
- **Tooling required:**
  - `rustc` & `cargo` with target `wasm32-unknown-unknown` (installed).
  - `wasm-tools` (needs to be available in PATH or tooling environment).
  - `wasm-ld` (from `lld` package or LLVM toolchain).
  - `roc` pinned nightly (`nightly-2026-09-29-7f11a82`).

---

## 5. Verification Checklist

To mark Unit 01 as complete, the following checks from `docs/testing.md` must be executed and reported:

1. [ ] **Rust Host Build (Clean):**
   `cargo build --manifest-path host/Cargo.toml --target wasm32-unknown-unknown --release --no-default-features` exits 0.
2. [ ] **Symbol Verification on `libhost.a`:**
   Inspect symbols using `llvm-nm` or `wasm-tools`; confirm no mock symbols from `guest_bridge.rs` are in `libhost.a`.
3. [ ] **Host Unit Tests:**
   `cargo test --manifest-path host/Cargo.toml --features stub-guest --lib` passes all 13 tests.
4. [ ] **Toolchain Check in `build.sh`:**
   Running `./tooling/build.sh app examples/counter/main.roc` checks the pinned Roc version.
5. [ ] **Real Roc App Build:**
   `./tooling/build.sh counter` compiles `examples/counter/main.roc` and links it with `libhost.a`.
6. [ ] **Distinct Component Hashes:**
   Run `./tooling/build.sh counter` and `./tooling/build.sh canary_release`; verify `sha256sum build/*_agent.wasm` produces different hashes.
7. [ ] **Component Model Validation:**
   `wasm-tools validate build/counter_agent.wasm` exits 0.
8. [ ] **Component World Verification:**
   `wasm-tools component wit build/counter_agent.wasm` exports `golem:agent/guest@2.0.0`.
9. [ ] **Documentation Sync:**
   Update `docs/progress-tracker.md` and `docs/specs/00-build-plan.md` to reflect Unit 01 progress/completion.
