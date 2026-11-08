# Progress Tracker

Keep this file short (about 150 lines). It is a status page, not a diary. Move finished sections to `docs/temp/progress-archive.md`. Historical audit narrative and workflow diagrams are preserved in `docs/temp/progress-archive.md`.

### Current Phase: Phase 3 — Release (2026-11-08) — Complete & Production GA v1.0.0

Goal: bring documentation, application manifest, crate metadata, and platform packaging into full alignment for the `v1.0.0` GA production release.

Status: **Unit 14 Complete & GA Released.** Expanded Layer 4 runtime test harness (`tooling/test_runtime.sh`) covering all 4 agents against local Golem Cloud server (v1.6 / 1.2.5); fixed wasm32 RocResult tag offset ABI for 64-bit payloads; bumped `host/Cargo.toml` and `tooling/build.sh` to `v1.0.0`; generated release archives (`.tar.zst`, `.tar.br`, `.tar.gz`) with BLAKE3 checksum `ik13T_ErjdlY0BtbrZ4WwiDVZWl1S6iDi2vExIkrlIw`; updated `README.md` and feature checklist; verified 100% passing across all 4 quality layers.

## Done

- **Unit 14 (End-to-End Golem Cloud Verification and v1.0.0 GA) Complete & Verified (2026-11-08)**:
  - Expanded Layer 4 runtime test harness (`tooling/test_runtime.sh`) with 4 comprehensive suites covering all four real Roc Golem agents against local Golem Cloud server (v1.6 / 1.2.5):
    - Suite 1: Counter Agent (deploy, first call, multi-turn mutations, crash/restart durability simulation, read-only bypass & state stability; Checklist #3, #5, #6, #7, #8, #9).
    - Suite 2: Canary Release Agent (bundled agent discovery `canary-release` and `diagnostics-worker`, service initialization, `start_canary` allocating durable promise & webhook URL, live HTML `dashboard` rendering, `approve_gate` promotion to 100% traffic, stateless `analyze_logs` tool, ephemeral `diagnostics-worker` log scanning; Checklist #4, #10, #12, #36, #38, #41).
    - Suite 3: AI Tool Agent (deploy, worker init, typed `calculator` tool execution returning 42, `echo` tool handling quotes & JSON escaping; Checklist #11, #44).
    - Suite 4: Streaming Agent (deploy, worker init, `connect` establishing WebSocket connection handle, `stream_message` tool verifying active streaming channel; Checklist #37, #45).
  - Resolved wasm32 ABI tag union layout mismatch in `host/src/lib.rs` for `RocResult<u64, RocStr>` where Roc expects discriminant at unpadded payload offset 12 (`rocFxCreatePromise`, `rocFxDbExecuteParams`).
  - Resolved Clippy `let_unit_value` errors in host tests (`host/src/lib.rs`).
  - Bumped `host/Cargo.toml` to `version = "1.0.0"`.
  - Updated default release tag in `tooling/build.sh` to `v1.0.0`.
  - Packaged platform release `v1.0.0` with `.tar.zst`, `.tar.br`, `.tar.gz` and computed BLAKE3 checksums (`ik13T_ErjdlY0BtbrZ4WwiDVZWl1S6iDi2vExIkrlIw`).
  - Refreshed `README.md` with official `v1.0.0` release URLs and computed BLAKE3 hash.
  - Verified 100% passing across all 4 quality layers: Layer 1 static checks, Layer 2 unit tests (53 host tests, 27 Roc tests), Layer 3 component builds (`wasm-tools validate` across all 4 agents), Layer 4 live runtime tests (`./tooling/test_runtime.sh` 100% pass).
  - All 19 Known Issues resolved; feature checklist verified. Production release `v1.0.0` GA achieved!

- **Unit 13 (Docs, Manifest and Release) Complete & Verified (2026-11-08)**:
  - Restructured `golem.yaml` to conform strictly to Golem application manifest schema (removed unknown fields `manifestVersion`, `app`, `tools`, `environments`; added valid `components` and `profiles`). Verified with `golem app diagnose` and `golem app build --help` (0 schema errors, `KI-16`).
  - Deduplicated skills by replacing `.gemini/skills/roc-golem.md` with a relative symlink to `.agents/skills/roc-golem.md` (`KI-17`).
  - Updated `host/Cargo.toml` with `version = "0.9.0"`, `license = "Apache-2.0"`, repository URL, and author attribution (`KI-18`).
  - Updated `host/src/lib.rs` line 34 comment to reference Golem 1.6 / 2.0.0 agent world (`KI-15`).
  - Updated default release tag in `tooling/build.sh` from `v0.1.0` to `v0.9.0`.
  - Packaged platform release `v0.9.0` with `.tar.zst`, `.tar.br`, `.tar.gz` and computed BLAKE3 checksums (`dist/checksums.txt`, `dist/release_notes.md`).
  - Refreshed `README.md` with `v0.9.0` release URL, real BLAKE3 base64url checksum (`1gPx2wxKBAsOazx0-cRBGbZulce4powwuCfq7dml6_s`), official `golem:agent/guest@2.0.0.{invoke}` deploy command, and `v0.9.0` tag documentation (`KI-15`).
  - Verified across all 4 quality layers: Layer 1 static checks (`cargo fmt`, `clippy` 0 warnings, `roc fmt --check`, `roc check` 0 errors across platform and all 4 examples), Layer 2 tests (all 53 host tests and all 27 Roc tests passing), Layer 3 builds (all 4 components built and validated with `wasm-tools validate`), and Layer 4 runtime verification (`./tooling/test_runtime.sh` passing all 5 tests against local Golem server).
  - Resolved `KI-15`, `KI-16`, `KI-17`, `KI-18`.

- **Unit 12 (Canary Example Made Real) Complete & Verified (2026-11-08)**:
  - Eliminated fake timer (`timerHandle = 42`) and hard-coded webhook URL in `examples/canary_release/ReleaseCoordinator.roc`.
  - Converted `handleStart` to an effectful handler registered via `registerEffectfulTyped`:
    - Calls `createPromise` to create a real durable promise.
    - Calls `createWebhook` to generate an honest public webhook URL.
    - Calls `rpcSchedule` to schedule a 5-minute emergency rollback timeout.
    - Commits real promise ID as `pendingTimerHandle: U64.to_u32_wrap(promiseId)` and records start event.
  - Added `Effect.always` and `Effect.and_then` combinators to `platform/Effect.roc` and exposed `Effect` in `platform/main.roc`.
  - Exposed live `dashboard` read-only method in `ReleaseCoordinator.roc` returning rendered HTML from current agent state.
  - Updated `host/src/lib.rs` to generate HTTP GET endpoints for read-only methods and attach literal `dashboard.html` suffix for `dashboard` / `dashboard.html` methods.
  - Updated mock guest bridge to declare `dashboard` method, return real promise webhook URLs on `start_canary`, and render dashboard HTML on `dashboard` calls.
  - Removed unused `dashboardPath` variable and fixed duplicate import/shadowing warnings in `ReleaseCoordinator.roc` and `Types.roc`.
  - Added new assertions in `test_canary_release_suite` in `host/src/lib.rs` (53/53 tests passing).
  - Verified across all 4 quality layers: Layer 1 static checks, Layer 2 host tests (53/53 passed) and Roc tests (`tests/envelope.roc`, `tests/json_roundtrip.roc`), Layer 3 builds with all 4 agent components validated by `wasm-tools validate`, and Layer 4 live runtime test (`./tooling/test_runtime.sh`).
  - Resolved `KI-14`, updated `docs/feature-checklist.md` tests 36 & 41 to PASSED. Phase 2 complete!

- **Unit 11 (Platform Neutrality) Complete & Verified (2026-11-08)**:
  - Eliminated hard-coded `name == "canary-release"` check in `host/src/lib.rs`.
  - Implemented dynamic `parse_http_mount` in `host/src/lib.rs` parsing arbitrary path prefixes with path variables (e.g. `/canary/{id}` or `/api/{service}/events`) into `PathSegment::Literal` and `PathSegment::PathVariable`.
  - Added `HttpMount` type in `platform/Types.roc`, `withHttpMount` builder in `platform/Golem.roc`, and metadata serialization in `platform/main.roc`.
  - Upgraded RPC across C ABI (`rocFxRpcInvoke`, `rocFxRpcSchedule`, `rocFxRpcFireAndForget`, and bytes variants) to pass explicit `constructor : Str` to target workers.
  - Added `rpcInstance`, `rpcScheduleInstance`, `rpcFireAndForgetInstance`, typed, and bytes variants in `platform/Golem.roc`, maintaining backward-compatible default-constructor wrappers.
  - Updated mock runtime and guest bridge to support metadata HTTP mounts and verify constructor transmission in RPC responses.
  - Added 7 unit tests in `host/src/lib.rs` (53/53 tests passing).
  - Refactored `ReleaseCoordinator.roc` to declare its HTTP mount via `withHttpMount`.
  - Updated skills (`.agents/skills/roc-golem.md` and `.gemini/skills/roc-golem.md`).
  - Verified across all 4 layers (Layer 1 static checks, Layer 2 unit tests, Layer 3 builds with all components validated, Layer 4 `./tooling/test_runtime.sh`).
  - Resolved `KI-11`, `KI-13`, and verified `docs/feature-checklist.md` test 18.

- **Unit 10 (Shared JSON Helpers and Honest Examples) Complete & Verified (2026-11-08)**:
  - Created `platform/Json.roc` exposing `encode`, `decode`, `decode_or`, `escape` leveraging Roc's built-in `Json` runtime primitives (`Json.to_str` and `Json.parse`).
  - Exposed `Json` module in `platform/main.roc` (`exposes [Golem, Types, Json]`) and updated `parse_tool_call` to decode `{ id : Str, name : Str, arguments : Str }` using `decode_or` for whitespace resilience.
  - Refactored all 4 example agents (`counter`, `ai_tool`, `streaming_agent`, `canary_release`) and `README.md` to use standard `Json.encode` and `Json.decode_or`/`Json.decode`.
  - Removed fragile ad-hoc extractors (`extractStr`, `extractU64`, `extractStringList`, `escapeJson`) from `examples/canary_release/Types.roc` and `ReleaseCoordinator.roc`.
  - Created automated test suite `tests/json_roundtrip.roc` covering primitives, negative numbers, zero, booleans, quotes, escapes, emojis, whitespace tolerance, agent state roundtrips, and tool envelopes (23/23 tests passed).
  - Integrated `roc test tests/json_roundtrip.roc` into `.github/workflows/ci.yaml` Layer 2 step.
  - Verified across all 4 layers (Layer 1 static checks, Layer 2 unit tests, Layer 3 builds with all components validated, Layer 4 `./tooling/test_runtime.sh`).
  - Resolved `KI-09` and verified `docs/feature-checklist.md` test 8 (Awkward state text).

- **Unit 09 (Surface Handler Errors) Complete & Verified (2026-11-08)**:
  - Implemented `AgentCapabilities` struct and `get_agent_capabilities(agent_name: &str)` helper in `host/src/lib.rs` querying agent metadata for declared methods and tools (normalizing snake_case and kebab-case variants).
  - Refactored `Guest::invoke`:
    - Dispatches declared tools directly to `main_handle_tool_call_for_host`, returning `Ok` with tool output or `AgentError::InvalidInput(err)`.
    - Dispatches declared methods directly to `main_handle_message_for_host`. Fails cleanly returning `AgentError::InvalidInput(err)` immediately without tool fallback.
    - Legacy fallback to tool invocation is preserved only when capabilities are empty and error contains 'not found'.
    - Returns `AgentError::InvalidMethod("Method '<name>' not found")` for unknown methods without attempting tool calls.
  - Added unit tests `test_handler_error_surfaces_cleanly`, `test_unknown_method_returns_invalid_method`, and `test_direct_tool_invocation`. Total 46 host unit tests passing cleanly.
  - Verified Layer 1 static checks (`cargo fmt`, `cargo clippy`, `roc fmt`, `roc check`), Layer 2 host tests (46/46 passed), Layer 3 builds (`./tooling/build.sh all` with 5 components validated), and Layer 4 runtime test (`./tooling/test_runtime.sh`).
  - Resolved `KI-10` and marked `docs/feature-checklist.md` tests 13 & 14 as PASSED. Phase 1 is now complete!

- **Unit 08 (Real Logging via WASI Logging) Complete & Verified (2026-11-08)**:
  - Added `import wasi:logging/logging;` to `wit/world.wit`.
  - Implemented `map_wasi_log_level(level: u8) -> wasi::logging::logging::Level` mapping 0->Trace, 1->Debug, 2->Info, 3->Warn, 4->Error, 5+->Critical.
  - Implemented target-specific `rocFxLog` in `host/src/lib.rs`:
    - On `wasm32`: invokes `wasi::logging::logging::log(wasi_level, "roc-agent", m)`.
    - On native: records to in-memory `LOG_CAPTURE_BUFFER` and prints formatted level, context, and message to stdout.
    - Added test capture helpers `capture_test_log`, `get_captured_logs()`, and `clear_captured_logs()`.
  - Added 4 comprehensive unit tests in `host/src/lib.rs` under `mod tests` (`test_logging_all_levels`, `test_logging_empty_and_null_message`, `test_logging_special_characters`, `test_map_wasi_log_level`). Total 43 host unit tests passing cleanly.
  - Verified Layer 1 static checks (`cargo fmt`, `cargo clippy`, `roc fmt`, `roc check`), Layer 2 host tests (43/43 passed), Layer 3 builds (`./tooling/build.sh all` with 5 components validated), and Layer 4 runtime test (`./tooling/test_runtime.sh`).
  - Resolved `KI-07` and marked `docs/feature-checklist.md` test 34 as PASSED.

- **Unit 06 (Secrets from Golem) Complete & Verified (2026-11-08)**:
  - Implemented target-specific secret resolution in `rocFxGetSecretHandle` (`host/src/lib.rs`):
    - Input validation: returns `empty secret name` if name is null or empty.
    - Idempotency & deduplication: checks existing `SECRETS` table first and returns existing handle.
    - On `wasm32`: checks `SECRET_ENV` test cell first, then calls `wasi::config::store::get(n)`. Returns descriptive error on missing (`secret '<name>' not found`) or upstream error (`failed to fetch secret '<name>': ...`).
    - On native: checks `SECRET_ENV` test cell, falling back to `std::env::var(n)`. Returns `secret '<name>' not found` if absent.
  - Verified `rocFxRevealSecret` returns `invalid secret handle <handle>` for handle 0 or unallocated handles.
  - Added 6 comprehensive TDD unit tests in `host/src/lib.rs` under `mod tests` (`test_secret_get_handle_found`, `test_secret_get_handle_idempotent`, `test_secret_get_handle_missing`, `test_secret_get_handle_empty`, `test_secret_reveal_valid`, `test_secret_reveal_invalid`). Total 33 host unit tests passing cleanly.
  - Verified Layer 1 static checks (`cargo fmt`, `cargo clippy`, `roc fmt`, `roc check`), Layer 2 host tests (33/33 passed), Layer 3 builds (`./tooling/build.sh all` with 4 components validated), and Layer 4 runtime test (`./tooling/test_runtime.sh`).
  - Resolved `KI-05` and updated `docs/feature-checklist.md` tests 25 & 26 to PASSED.

- **Unit 05 (Real Scheduling) Complete & Verified (2026-11-08)**:
  - Imported `wasi:clocks/wall-clock@0.2.3` in `wit/world.wit`.
  - Implemented pure deterministic time helper `compute_scheduled_time` with nanosecond carry-over and saturating arithmetic.
  - Updated `rocFxRpcSchedule` and `rocFxRpcScheduleBytes` in `host/src/lib.rs`:
    - Added input validation ensuring `target` and `function_name` are not empty (returning `RocResult::err`).
    - On `wasm32`: queries wall clock time via `wasi::clocks::wall_clock::now()`, calculates target datetime, and invokes `rpc_client.schedule_invocation`.
    - On native: calculates simulated scheduled time and returns structured JSON acknowledgment with `scheduledSeconds`.
  - Added 8 comprehensive unit tests in `host/src/lib.rs` (zero delay, subsecond no carry, nanosecond rollover, multi-day delay, saturating overflow, valid mock, and validation errors). Total 27 host unit tests passing cleanly.
  - Verified Layer 1 static checks (`cargo fmt`, `cargo clippy`, `roc fmt`, `roc check`), Layer 2 host tests (27/27 passed), Layer 3 builds (`./tooling/build.sh all` with 4 components validated), and Layer 4 runtime test (`./tooling/test_runtime.sh`).
  - Fixed `KI-04` and updated `docs/feature-checklist.md` Test #17 to PASSED.

- **Unit 04 (Layer 4 Runtime Test Harness) Complete & Verified (2026-11-07)**:
  - Created automated test harness `tooling/test_runtime.sh` deploying real `build/counter_agent.wasm` to local Golem server (v1.2.5 / 1.6).
  - Aligned low-level Roc / Rust Host WebAssembly ABI:
    - Fixed Roc C ABI return pointer order in `host/src/lib.rs` and `host/src/guest_bridge.rs` (return pointer is argument 0).
    - Fixed `RocResult` discriminant (`Ok=1, Err=0`) matching Roc variant alphabetic ordering.
    - Implemented `GlobalCell<T>` on `wasm32-unknown-unknown` linear memory to prevent thread-local storage destruction across invocations.
    - Fixed `RocStr` 3-word struct layout (`words[0]=bytes, words[1]=capacity, words[2]=length`) and Drop refcount handling for static constants (`*ref_count > 0`).
    - Fixed C ABI ownership conventions across host/guest boundary via `core::mem::forget` on parameters passed to Roc callee, eliminating double-free corruption in `dlmalloc`.
  - Verified checklist tests against live Golem server:
    - #3 Deploy component to local Golem server.
    - #5 First call on new worker increments count to 1.
    - #6 State preserved across multiple turns (count = 3).
    - #7 Durable state recovered across simulated worker crash (`golem worker simulate-crash`).
    - #8 Read-only bypass preserves state across repeated invocations without mutation.

- **Unit 03 (Roc in CI) Complete & Verified (2026-11-07)**:
  - Consolidated duplicate CI workflows into unified `.github/workflows/ci.yaml` and deleted redundant `.github/workflows/ci.yml` (KI-12).
  - Pinned tools in CI: `wasm-tools 1.222.0`, Roc compiler nightly `nightly-2026-09-29-7f11a82` with cache (`~/.roc-cache`), and Rust with `wasm32-unknown-unknown`, `clippy`, `rustfmt`.
  - Added full multi-layer pipeline to CI:
    - Layer 1: `cargo fmt --check`, `cargo clippy`, `roc fmt --check`, and `roc check` for `platform/main.roc` and all 4 examples (KI-08).
    - Layer 2: `cargo test --manifest-path host/Cargo.toml --features stub-guest --lib` (19 tests) and `roc test tests/envelope.roc`.
    - Layer 3: `./tooling/build.sh all` compiling real Roc code and pre-compiled host for all 4 examples.
    - Layer 4: `wasm-tools validate` for all 4 agent components and packaging verification (`dist/checksums.txt`, `dist/release_notes.md`).
  - Verified every pipeline command locally with 0 errors.
- **Unit 02 (Valid Reply Envelopes & State Escaping) Complete & Verified (2026-11-07)**:
  - Fixed JSON interpolation bug in `platform/main.roc` (KI-03): raw `nextStateJson` strings are escaped via `escape_json_str` in `format_reply_envelope` and `format_tool_envelope`.
  - Preserved zero-serialization read-only bypass: empty string `""` state updates instruct Golem host to preserve state in $O(1)$ time without overwriting storage.
  - Hardened state deserialization in `examples/counter/main.roc` and `README.md` to cleanly handle whitespace, negative numbers, and delimiters.
  - Added 6 new unit tests in `host/src/lib.rs` covering escaped state, read-only bypass, complex unicode/newline state round-trips, tool calls, and reproduction of KI-03. All 19 host tests passing; `cargo clippy` clean.
  - Added Roc unit test suite `tests/envelope.roc` (all 4 expect tests passing).
  - Verified full compilation and validation across all agents via `./tooling/build.sh all`.
- **Unit 01 (Real Roc Build) Complete & Verified (2026-11-07)**:
  - Installed complete toolchain: `rustc 1.99.1`, `cargo 1.99.1`, `wasm32-unknown-unknown`, LLVM tools with `wasm-ld` wrapper, `wasm-tools 1.222.0`, `roc nightly-2026-09-29-7f11a82`, and `golem-cli 1.2.5`.
  - Decoupled `stub-guest` from default features in `host/Cargo.toml` (`default = []`). Resolved Linux GNU `ld` version-script failure (KI-19) by removing `cdylib` from crate-types.
  - Aligned Roc host ABI: clean symbols `main_init_for_host`, `main_handle_message_for_host`, `main_handle_tool_call_for_host`, `main_metadata_for_host` (unit function `{} -> Str` preventing const-eval crashes), and `cabi_realloc`.
  - Updated `roc_realloc` in `host/src/roc_std.rs` to modern 3-argument signature `(c_ptr, new_size, alignment)`. Added `roc_crashed` and `roc_expect_failed` to prevent unresolved `env` imports during componentization.
  - Updated `platform/main.roc` with `targets: { wasm32: ... }` native platform pipeline and complete `hosted:` mapping for all 53 effect functions. Updated `platform/Effect.roc` to nominal type module `Effect(a) := [Effect({} => a)].{ ... }` with effectful methods (`!`). Added `bundle2` and `erase` to `platform/Golem.roc`.
  - Migrated all four examples (`counter`, `ai_tool`, `streaming_agent`, `canary_release`) to modern Roc syntax (`True`/`False`, `Str.inspect`, `I64.from_str`, `U64.from_str`, `U32.from_str`, open record patterns).
  - Built all four agents with `./tooling/build.sh all`: all four distinct WebAssembly components validated with `wasm-tools validate` and unique SHA-256 digests. Packaged release archives into `dist/`.
- Rust host with real Golem calls for most effects, 13 native tests passing, `cargo fmt` and `clippy` clean (checked 2026-11-07, native only).
- Verified: `cargo build --manifest-path host/Cargo.toml --target wasm32-unknown-unknown --release` succeeds, generating `libroc_golem_host.a`.
- Release packaging: BLAKE3 base64url checksum calculation, `.tar.zst`, `.tar.br`, `.tar.gz` reproducible tarball builder via `tooling/pack_platform`.
- Decisions (2026-11-07): the Roc nightly is pinned in `docs/architecture.md`; runtime tests run on a local Golem server 1.6.

## Known issues

Source: code review of 2026-11-07 and build verification.

| ID | Issue | Severity | Status |
| :--- | :--- | :--- | :--- |
| KI-01 | `build.sh` never compiles or links Roc. `build_app` only runs `roc fmt` and wraps the pre-built host. | Critical | **Resolved** in Unit 01 (native `roc build --target=wasm32` + `wasm-tools component new`) |
| KI-02 | Feature `stub-guest` is on by default, so fake agent is compiled into `libhost.a`. | Critical | **Resolved** in Unit 01 (`default = []`, production wasm builds with `--no-default-features`) |
| KI-03 | `platform/main.roc` writes `{"state":"<state>",...}` without escaping the state, so any state with quotes makes the reply invalid JSON. The host then ignores the reply and never saves state. | Critical | **Resolved** in Unit 02 (escaped envelope formatting in `platform/main.roc`) |
| KI-04 | `rocFxRpcSchedule` and `rocFxRpcScheduleBytes` return success without scheduling anything, on every target. `wit/deps/golem-agent/host.wit` offers `schedule-invocation`. | High | **Resolved** in Unit 05 (real wall-clock scheduling via wasm-rpc) |
| KI-05 | `rocFxGetSecretHandle` reads secrets from `std::env::var`, which does not exist on `wasm32-unknown-unknown`. Works only in tests. | High | **Resolved** in Unit 06 (`wasi:config/store` on `wasm32` + `SECRET_ENV`/`std::env::var` fallback) |
| KI-06 | File effects (`readFile`, `writeFile`, bytes variants, append) use `std::fs`, unsupported on this target; `wasi:filesystem` is not in `wit/world.wit`. | High | **Resolved** in Unit 07 (real WASI 0.2 `wasi:filesystem` with preopens on `wasm32`, `std::fs` native fallback) |
| KI-07 | `rocFxLog` uses `println!`, which likely goes nowhere on this target. | Medium | **Resolved** in Unit 08 (real WASI logging via `wasi:logging/logging` on `wasm32`, test buffer on native) |
| KI-08 | No test compiles Roc: CI never runs `roc check`, `roc build` or `roc test`. The 13 tests run against `guest_bridge.rs`. | Critical | **Resolved** in Unit 03 (full Roc checks, tests, builds, validations in CI) |
| KI-09 | Examples parse JSON with ad-hoc text search. The README counter has `deserializeState: \|_json\| Ok({ count: 0 })`, which resets state on every load. `ai_tool` does not escape `lastTool`. | High | **Resolved** in Unit 10 (shared `platform/Json.roc` codec module, honest examples, 23/23 roundtrip tests) |
| KI-10 | On any message-handler error the host retries as a tool call and reports `InvalidMethod` with the first error, hiding the real failure. | Medium | **Resolved** in Unit 09 (capability-based routing, direct tool invocation, immediate handler error surface) |
| KI-11 | The host hard-codes `name == "canary-release"` to add an HTTP mount at `/canary/{id}`. | Medium | **Resolved** in Unit 11 (dynamic metadata-driven HTTP mount parsing via `withHttpMount`) |
| KI-12 | Two CI files named "CI" (`ci.yaml`, `ci.yml`) with different jobs; `wasm-tools` pinned at 1.220.0 and 1.222.0; one job packages as `v0.7.0`; neither CI nor `build.sh` installs or checks the pinned Roc nightly. | Medium | **Resolved** in Unit 03 (consolidated to `ci.yaml`, removed `ci.yml`, pinned `wasm-tools 1.222.0` and Roc nightly) |
| KI-13 | RPC always constructs the target with an empty constructor (`"{}"`), so agents with constructor arguments cannot be addressed. | Medium | **Resolved** in Unit 11 (addressable agent worker instances via constructor argument in RPC) |
| KI-14 | Canary example fakes its timer (`timerHandle = 42`) and webhook URL, defines `renderDashboard` but never serves it, and has an unused variable. | Medium | **Resolved** in Unit 12 (real timer, promise, webhook, live dashboard, clean imports) |
| KI-15 | README is stale: deploy command uses `golem:agent-platform/agent-api.{handle-message}`; hash points to v0.8.0; host comment still says "Golem 1.5.0". | Medium | **Resolved** in Unit 13 (refreshed README, v0.9.0 BLAKE3 hash, guest 2.0.0 invoke, Golem 1.6 host comment) |
| KI-16 | `golem.yaml` has a top-level `tools:` key that does not appear in the 1.6.0 manifest schema as far as known. | Low | **Resolved** in Unit 13 (conformed `golem.yaml` to Golem application manifest schema) |
| KI-17 | `.agents/skills/roc-golem.md` and `.gemini/skills/roc-golem.md` are identical copies. | Low | **Resolved** in Unit 13 (replaced `.gemini/skills/roc-golem.md` with relative symlink) |
| KI-18 | `host/Cargo.toml` still says version 0.1.0 and "Contributors". | Low | **Resolved** in Unit 13 (version 0.9.0, Apache-2.0, author attribution, repo URL) |
| KI-19 | Native `cargo test --all-targets` fails to link on Linux: GNU `ld` version-script parser chokes on WIT exported symbol names containing `:`, `@`, and `#` when building `cdylib`. | Medium | **Resolved** in Unit 01 (removed `cdylib` from crate-types) |

## Open questions

- *(None currently open; all known issues resolved)*

## Next steps

- All 14 units complete. Platform `v1.0.0` GA released and verified across all 4 quality layers. Future maintenance and community feedback.
