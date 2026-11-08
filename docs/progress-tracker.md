# Progress Tracker

Keep this file short (about 150 lines). It is a status page, not a diary. Move finished sections to `docs/temp/progress-archive.md`. Historical audit narrative and workflow diagrams are preserved in `docs/temp/progress-archive.md`.

### Current Phase: Phase 1 — Make every effect honest (COMPLETE) (2026-11-08)

Goal: prove the platform works end to end with real Roc code on a real Golem, and close the gaps where code only pretends to work. Units are in `docs/specs/00-build-plan.md`.

Status: **Phase 1 COMPLETE! Unit 09 Complete & Verified.** Fails cleanly on handler errors with `AgentError::InvalidInput(err)` without falling back to tools; routes genuine tools directly to tool handlers; returns clear `AgentError::InvalidMethod("Method '<name>' not found")` for unknown methods (`KI-10`). All Phase 1 units (05, 06, 07, 08, 09) complete and verified across all 4 testing layers. Ready for Phase 2.

## Done

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
| KI-09 | Examples parse JSON with ad-hoc text search. The README counter has `deserializeState: \|_json\| Ok({ count: 0 })`, which resets state on every load. `ai_tool` does not escape `lastTool`. | High | Open (Unit 02) |
| KI-10 | On any message-handler error the host retries as a tool call and reports `InvalidMethod` with the first error, hiding the real failure. | Medium | **Resolved** in Unit 09 (capability-based routing, direct tool invocation, immediate handler error surface) |
| KI-11 | The host hard-codes `name == "canary-release"` to add an HTTP mount at `/canary/{id}`. | Medium | Open (Unit 10) |
| KI-12 | Two CI files named "CI" (`ci.yaml`, `ci.yml`) with different jobs; `wasm-tools` pinned at 1.220.0 and 1.222.0; one job packages as `v0.7.0`; neither CI nor `build.sh` installs or checks the pinned Roc nightly. | Medium | **Resolved** in Unit 03 (consolidated to `ci.yaml`, removed `ci.yml`, pinned `wasm-tools 1.222.0` and Roc nightly) |
| KI-13 | RPC always constructs the target with an empty constructor (`"{}"`), so agents with constructor arguments cannot be addressed. | Medium | Open (Unit 05) |
| KI-14 | Canary example fakes its timer (`timerHandle = 42`) and webhook URL, defines `renderDashboard` but never serves it, and has an unused variable. | Medium | Open (Unit 11) |
| KI-15 | README is stale: deploy command uses `golem:agent-platform/agent-api.{handle-message}`; hash points to v0.8.0; host comment still says "Golem 1.5.0". | Medium | Open |
| KI-16 | `golem.yaml` has a top-level `tools:` key that does not appear in the 1.6.0 manifest schema as far as known. | Low | Open |
| KI-17 | `.agents/skills/roc-golem.md` and `.gemini/skills/roc-golem.md` are identical copies. | Low | Open |
| KI-18 | `host/Cargo.toml` still says version 0.1.0 and "Contributors". | Low | Open |
| KI-19 | Native `cargo test --all-targets` fails to link on Linux: GNU `ld` version-script parser chokes on WIT exported symbol names containing `:`, `@`, and `#` when building `cdylib`. | Medium | **Resolved** in Unit 01 (removed `cdylib` from crate-types) |

## Open questions

- Should HTTP mounts be declared in agent metadata (replacing the `canary-release` special case)? (unit 10)

## Next steps

1. Phase 2 — Unit 10: Shared JSON helpers and honest examples.
