# Specification: Unit 14 — End-to-End Golem Cloud Verification and v1.0.0 GA

- **Unit:** 14
- **File:** `docs/specs/14-e2e-verification-and-v1-ga.md`
- **Status:** Spec written
- **Phase:** Phase 3 — Release
- **Fixes:** Final feature checklist verification & v1.0.0 GA Production Release

---

## 1. Goal

Execute comprehensive end-to-end runtime verification across all four compiled Roc agent components (`counter`, `canary_release`, `ai_tool`, `streaming_agent`) against a local Golem server (v1.6 / 1.2.5), validating durable persistence, multi-agent bundling, ephemeral execution, tool calling, live dashboard serving, and WebSocket streaming. Mark all verified items in `docs/feature-checklist.md` as PASSED, and cut the official `v1.0.0` GA Production Release with release archives and updated platform hashes.

---

## 2. Design & Architecture

### 2.1 Multi-Agent End-to-End Verification Harness (`tooling/test_runtime.sh`)

Extend `tooling/test_runtime.sh` from testing only `counter` to validating the entire suite:

1. **Counter Agent (Durable State Persistence)**:
   - Deploy `build/counter_agent.wasm`.
   - Verify initial increment (Checklist #5).
   - Verify multi-turn increment and state persistence (Checklist #6).
   - Verify durable state survival across worker simulated crash and restart (Checklist #7).
   - Verify read-only bypass (`get` preserves state with zero re-serialization) (Checklist #8, #9).

2. **Canary Release (Multi-Agent Bundling, Ephemeral & Gate Approval)**:
   - Deploy `build/canary_release_agent.wasm`.
   - Verify agent type discovery lists both `canary-release` (durable) and `diagnostics-worker` (ephemeral) (Checklist #4).
   - Invoke `diagnostics-worker`: verify stateless ephemeral execution with 0 faults for clean logs and faults detected for error logs (Checklist #10).
   - Invoke `canary-release` `start_canary`: verify healthy start produces a real webhook URL (`webhooks/promise/...`) and non-zero promise handle (Checklist #36).
   - Invoke `canary-release` `dashboard`: verify rendered HTML contains `<h1>Release Guardian</h1>`, service name, versions, and events (Checklist #41).
   - Invoke `canary-release` `approve_gate`: verify promotion to 100% traffic weight (Checklist #38).
   - Invoke `canary-release` `analyze_logs` tool: verify tool call handling (Checklist #12).

3. **AI Tool Agent (Tool Invocations & Escaped IO)**:
   - Deploy `build/ai_tool_agent.wasm`.
   - Invoke `calculator` tool: verify mathematical evaluation.
   - Invoke `echo` tool with special characters, quotes, and newlines: verify clean JSON roundtrip and escaping.

4. **Streaming Agent (WebSocket & Ephemeral Messaging)**:
   - Deploy `build/streaming_agent.wasm`.
   - Invoke `connect` message: verify connection state transition.
   - Invoke `stream_message` tool: verify active stream response.

### 2.2 v1.0.0 GA Production Release Packaging

1. **Version Bump**:
   - `host/Cargo.toml`: update `version = "1.0.0"`.
   - `tooling/build.sh`: update default tag to `v1.0.0`.
2. **Platform Packaging**:
   - Run `./tooling/build.sh package v1.0.0 thesparq/roc-golem`.
   - Generate release archives: `.tar.zst`, `.tar.br`, `.tar.gz`.
   - Extract the computed BLAKE3 base64url checksum from `dist/checksums.txt`.
3. **Documentation Alignment**:
   - Update `README.md` platform import snippet with the `v1.0.0` URL and BLAKE3 hash.
   - Update `docs/progress-tracker.md` and `docs/specs/00-build-plan.md` marking Unit 14 and all project phases complete.
   - Update `docs/feature-checklist.md` marking all runtime-verified tests as PASSED.

---

## 3. Implementation Details

### 3.1 Harness Upgrades (`tooling/test_runtime.sh`)
- Add functions:
  - `test_counter_agent`: runs Tests #3, #5, #6, #7, #8, #9.
  - `test_canary_agent`: runs Tests #4, #10, #12, #36, #38, #41.
  - `test_ai_tool_agent`: tests calculator and echo tools.
  - `test_streaming_agent`: tests connection and streaming tool calls.
- Log colored `[PASS]` / `[FAIL]` indicators for each checklist item.

### 3.2 Feature Checklist Updates (`docs/feature-checklist.md`)
Mark checklist tests 4, 9, 10, 11, 12, 36, 37, 38, 41, 42 as PASSED with execution date.

### 3.3 Platform Release Tagging & Packaging
- Package `v1.0.0` archives into `dist/`.
- Update `README.md` to reference the final `v1.0.0` release.

---

## 4. Dependencies

- **Prior Units:** Units 01 through 13.
- **Tooling required:**
  - Roc nightly `nightly-2026-09-29-7f11a82`.
  - `wasm-tools 1.222.0`.
  - `golem-cli 1.2.5`.
  - Rust toolchain.

---

## 5. Verification Checklist

To mark Unit 14 as complete, the following checks from `docs/testing.md` must be executed and reported:

1. [ ] **Multi-Agent Runtime Verification (Layer 4):**
   - `./tooling/test_runtime.sh` passes all end-to-end tests for `counter`, `canary_release`, `ai_tool`, and `streaming_agent` against a local Golem server.
2. [ ] **Unit Tests (Layer 2):**
   - `cargo test --manifest-path host/Cargo.toml --features stub-guest --lib` passes all 53 host unit tests.
   - `roc test tests/envelope.roc` and `roc test tests/json_roundtrip.roc` pass cleanly.
3. [ ] **Code Hygiene & Static Checks (Layer 1):**
   - `roc fmt --check` passes cleanly across all Roc files.
   - `roc check platform/main.roc` and `roc check examples/*/main.roc` report zero errors.
   - `cargo fmt --check` and `cargo clippy --manifest-path host/Cargo.toml --features stub-guest --lib -- -D warnings` report zero warnings or errors.
4. [ ] **Component Build & Packaging (Layer 3):**
   - `./tooling/build.sh all` successfully builds all 4 agent components and produces release archives in `dist/` for `v1.0.0`.
   - `wasm-tools validate` validates all 4 WebAssembly components.
   - `dist/checksums.txt` and `dist/release_notes.md` are populated for `v1.0.0`.
5. [ ] **Documentation & Milestone Completion:**
   - `README.md` references `v1.0.0` platform release URL and valid hash.
   - `docs/feature-checklist.md` updated with all runtime-verified tests marked PASSED.
   - `docs/progress-tracker.md` and `docs/specs/00-build-plan.md` updated marking Unit 14 Complete and all project phases complete.
