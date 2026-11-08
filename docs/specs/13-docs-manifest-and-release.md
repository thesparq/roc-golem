# Specification: Unit 13 — Docs, Manifest and Release

- **Unit:** 13
- **File:** `docs/specs/13-docs-manifest-and-release.md`
- **Status:** Spec written
- **Phase:** Phase 3 — Release
- **Fixes:** `KI-15`, `KI-16`, `KI-17`, `KI-18`

---

## 1. Goal

Bring documentation, application manifest, crate metadata, and platform packaging into full alignment for the `v0.9.0` release:
1. **Refresh README & Deploy Instructions (`KI-15`)**: Update obsolete `golem:agent-platform/agent-api.{handle-message}` invocation commands to official `golem:agent/guest@2.0.0.{invoke}`; update platform URL in the quickstart snippet with the real `v0.9.0` BLAKE3 hash; update stale host comments referencing Golem 1.5.0 to Golem 1.6 / 2.0.0.
2. **Conform `golem.yaml` to Golem Schema (`KI-16`)**: Remove unsupported top-level keys (`manifestVersion`, `tools`, `environments`) that cause `golem app` parsing failures. Restructure manifest with valid `components`, `profiles`, and `httpApi` definitions matching the Golem application manifest schema.
3. **Deduplicate Skill Files (`KI-17`)**: Replace the duplicate file `.gemini/skills/roc-golem.md` with a symlink pointing to `.agents/skills/roc-golem.md`, establishing a single source of truth across agent harnesses.
4. **Update Crate Metadata (`KI-18`)**: Bump `host/Cargo.toml` version to `0.9.0`, specify license (`Apache-2.0`), repository URL, and proper author attribution.
5. **Platform Package & Release Notes v0.9.0**: Package platform archives (`.tar.zst`, `.tar.br`, `.tar.gz`) for tag `v0.9.0`, compute BLAKE3 base64url checksums, and generate release notes in `dist/`.

---

## 2. Design & Architecture

### 2.1 The Problem Statement

- **Stale README (`KI-15`)**:
  `README.md` lines 200–204 instruct users to invoke:
  ```bash
  golem worker invoke-and-await --component-name streaming-agent --worker-name stream-worker-1 \
    --function "golem:agent-platform/agent-api.{handle-message}" \
    --args '["connect"]'
  ```
  This function does not exist in the component model exports (which export `golem:agent/guest@2.0.0#invoke`). Additionally, the header references a legacy `v0.8.0` platform archive URL with an outdated hash, and `host/src/lib.rs` line 34 states `// Generate WIT bindings for the official Golem 1.5.0 agent world`.
- **Manifest Incompatibility (`KI-16`)**:
  Running `golem app` fails immediately:
  ```
  unknown field `manifestVersion`, expected one of `includes`, `tempDir`, `witDeps`, `templates`, `components`, `dependencies`, `customCommands`, `clean`, `httpApi`, `profiles`
  ```
  `golem.yaml` currently includes unrecognized fields `manifestVersion`, `app`, `environments`, and `tools`.
- **Duplicate Skills (`KI-17`)**:
  `.agents/skills/roc-golem.md` and `.gemini/skills/roc-golem.md` are separate copies that must be manually kept in sync, violating Principle 11 ("One public API, four views").
- **Crate Metadata (`KI-18`)**:
  `host/Cargo.toml` has `version = "0.1.0"` and generic authors.

---

## 3. Implementation Details

### 3.1 Golem Application Manifest (`golem.yaml`)
Restructure `golem.yaml` to conform strictly to Golem's application schema:
```yaml
# Schema for IDEA:
# $schema: https://schema.golem.cloud/app/golem/1.2.5/golem.schema.json
# Schema for vscode-yaml
# yaml-language-server: $schema=https://schema.golem.cloud/app/golem/1.2.5/golem.schema.json

components:
  canary-release:
    componentWasm: "build/canary_release_agent.wasm"
    build:
      - command: "./tooling/build.sh canary_release"
  counter:
    componentWasm: "build/counter_agent.wasm"
    build:
      - command: "./tooling/build.sh counter"
  ai-tool:
    componentWasm: "build/ai_tool_agent.wasm"
    build:
      - command: "./tooling/build.sh ai_tool"
  streaming-agent:
    componentWasm: "build/streaming_agent.wasm"
    build:
      - command: "./tooling/build.sh streaming_agent"

profiles:
  local:
    default: true
  cloud:
    cloud: true
```
Verify `golem app diagnose` or `golem app build --help` runs without schema errors.

### 3.2 Single Source of Truth for Skills (`.gemini/skills/roc-golem.md`)
- Remove `.gemini/skills/roc-golem.md`.
- Create relative symlink: `ln -s ../../.agents/skills/roc-golem.md .gemini/skills/roc-golem.md`.
- Verify both paths resolve to the identical file content.

### 3.3 Crate Metadata (`host/Cargo.toml`)
Update `host/Cargo.toml`:
```toml
[package]
name = "roc-golem-host"
version = "0.9.0"
edition = "2021"
description = "Rust Host translation layer for Roc Golem Cloud platform"
authors = ["Samuel Iyeh <thesparq@vmi3258764.contaboserver.net>", "Roc Golem Platform Contributors"]
license = "Apache-2.0"
repository = "https://github.com/thesparq/roc-golem"
```

### 3.4 Host Comment Update (`host/src/lib.rs`)
Update line 34 comment to reflect modern Golem 1.6 / guest 2.0.0 architecture:
```rust
// Generate WIT bindings for the official Golem 1.6 / 2.0.0 agent world
```

### 3.5 Platform Packaging & Release Generation (`v0.9.0`)
1. Update default tag in `tooling/build.sh` from `v0.1.0` to `v0.9.0`.
2. Run packaging:
   ```bash
   ./tooling/build.sh package v0.9.0 thesparq/roc-golem
   ```
3. Extract the computed BLAKE3 base64url checksum from `dist/checksums.txt`.
4. Update `README.md`:
   - Replace `v0.8.0/...` URL with the `v0.9.0` release URL and exact BLAKE3 hash.
   - Update deploy command to:
     ```bash
     golem component add --component-name streaming-agent build/streaming_agent.wasm
     golem worker add --component-name streaming-agent --worker-name stream-worker-1
     golem worker invoke-and-await --component-name streaming-agent --worker-name stream-worker-1 \
       --function "golem:agent/guest@2.0.0.{invoke}" \
       --args '["handle-message", "connect", "anonymous"]'
     ```
   - Update release tagging instructions to `v0.9.0`.

---

## 4. Dependencies

- **Prior Units:** Units 01 through 12.
- **Tooling required:**
  - Roc nightly `nightly-2026-09-29-7f11a82`.
  - `wasm-tools 1.222.0`.
  - `golem-cli 1.2.5`.
  - Rust toolchain.

---

## 5. Verification Checklist

To mark Unit 13 as complete, the following checks from `docs/testing.md` must be executed and reported:

1. [ ] **Manifest & Tooling Conformance:**
   - `golem app diagnose` or `golem-cli` successfully parses `golem.yaml` with 0 schema errors.
2. [ ] **Skill Deduplication:**
   - `.gemini/skills/roc-golem.md` is verified as a valid relative symlink to `.agents/skills/roc-golem.md`.
3. [ ] **Code Hygiene & Static Checks (Layer 1):**
   - `roc fmt --check` passes cleanly across all Roc files.
   - `roc check platform/main.roc` and `roc check examples/*/main.roc` report 0 errors.
   - `cargo fmt --check` and `cargo clippy --manifest-path host/Cargo.toml --features stub-guest --lib -- -D warnings` report 0 errors or warnings.
4. [ ] **Unit Tests (Layer 2):**
   - `cargo test --manifest-path host/Cargo.toml --features stub-guest --lib` passes all 53 unit tests.
   - `roc test tests/envelope.roc` and `roc test tests/json_roundtrip.roc` pass cleanly.
5. [ ] **Component Builds & Packaging (Layer 3):**
   - `./tooling/build.sh all` successfully builds all four agent components and produces release archives in `dist/`.
   - `wasm-tools validate` validates all 4 WebAssembly components.
   - `dist/checksums.txt` and `dist/release_notes.md` are populated for tag `v0.9.0`.
6. [ ] **Layer 4 Runtime Verification:**
   - `./tooling/test_runtime.sh` passes completely against a running local Golem server.
7. [ ] **Feature Checklist & Progress Tracker:**
   - `docs/progress-tracker.md` updated with `KI-15`, `KI-16`, `KI-17`, `KI-18` marked resolved.
