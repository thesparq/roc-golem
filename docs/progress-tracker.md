# Progress Tracker

Keep this file short (about 150 lines). It is a status page, not a diary. Move finished sections to `docs/temp/progress-archive.md`. Historical audit narrative and workflow diagrams are preserved in `docs/temp/progress-archive.md`.

## Current Phase: Phase 0 — Make it real (2026-11-07)

Goal: prove the platform works end to end with real Roc code on a real Golem, and close the gaps where code only pretends to work. Units are in `docs/specs/00-build-plan.md`.

Status: **Unit 01 spec written** (`docs/specs/01-real-roc-build.md`), awaiting review.

## Done

- Rust host with real Golem calls for most effects, 13 native tests passing, `cargo fmt` and `clippy` clean (checked 2026-11-07, native only).
- Verified: `cargo build --manifest-path host/Cargo.toml --target wasm32-unknown-unknown --release` succeeds (51s), generating `libroc_golem_host.a` and `roc_golem_host.wasm`.
- Release packaging and eight tagged releases (v0.4.0 to v0.8.0). Latest tag v0.8.0 is six commits behind `main`.
- Golem 1.6 alignment on `main`: manifest 1.6.0, `golem:agent` interfaces at 2.0.0.
- Platform SDK abstractions implemented: `bundle`, `Method(input, output)` phantom types, `defineEphemeral`, `registerReadOnly`, and 53 low-level host effect declarations in `Effect.roc` and `Golem.roc`.
- Host bridge primitives implemented: Roc ABI types (`RocStr` with SSO, `RocList`, custom allocators), Golem 1.6 WIT guest exports, zero-serialization bypass.
- Packaging tool (`tooling/pack_platform`): BLAKE3 base64url checksum calculation, `.tar.zst`, `.tar.br`, `.tar.gz` reproducible tarball builder.
- Decisions (2026-11-07): the Roc nightly is pinned in `docs/architecture.md`; runtime tests run on a local Golem server 1.6.

## Known issues

Source: code review of 2026-11-07. "Certainty" says how it was established. The Roc compiler and the `wasm32` build target were **not available** during the review, so items marked *read* come from reading code and have not been reproduced.

| ID | Issue | Severity | Certainty |
| :--- | :--- | :--- | :--- |
| KI-01 | `build.sh` never compiles or links Roc. `build_app` only runs `roc fmt` and wraps the pre-built host, so every `*_agent.wasm` is the same host. | Critical | read (script is explicit) |
| KI-02 | Feature `stub-guest` is on by default, so the fake agent is compiled into `libhost.a` / `host.wasm` and is likely to clash with the real `roc__main_*` symbols. | Critical | read |
| KI-03 | `platform/main.roc` writes `{"state":"<state>",...}` without escaping the state, so any state with quotes makes the reply invalid JSON. The host then ignores the reply and never saves state. | Critical | read (the fake agent builds it correctly, which hides this) |
| KI-04 | `rocFxRpcSchedule` and `rocFxRpcScheduleBytes` return success without scheduling anything, on every target. `wit/deps/golem-agent/host.wit` offers `schedule-invocation`. | High | read |
| KI-05 | `rocFxGetSecretHandle` reads secrets from `std::env::var`, which does not exist on `wasm32-unknown-unknown`. Works only in tests. | High | read (target behavior inferred) |
| KI-06 | File effects (`readFile`, `writeFile`, bytes variants, append) use `std::fs`, unsupported on this target; `wasi:filesystem` is not in `wit/world.wit`. | High | read (target behavior inferred) |
| KI-07 | `rocFxLog` uses `println!`, which likely goes nowhere on this target. | Medium | read (target behavior inferred) |
| KI-08 | No test compiles Roc: CI never runs `roc check`, `roc build` or `roc test`. The 13 tests run against `guest_bridge.rs`. | Critical | read, plus ran `cargo test` (13 pass) |
Conflicts with progress file: "Building cdylib triggers GNU ld version-script failure with WIT mangled names when cargo test builds all-targets on Linux, whereas cargo test --lib passes."
| KI-09 | Examples parse JSON with ad-hoc text search. The README counter has `deserializeState: \|_json\| Ok({ count: 0 })`, which resets state on every load. `ai_tool` does not escape `lastTool`. | High | read |
| KI-10 | On any message-handler error the host retries as a tool call and reports `InvalidMethod` with the first error, hiding the real failure. | Medium | read |
| KI-11 | The host hard-codes `name == "canary-release"` to add an HTTP mount at `/canary/{id}`. | Medium | read |
| KI-12 | Two CI files named "CI" (`ci.yaml`, `ci.yml`) with different jobs; `wasm-tools` pinned at 1.220.0 and 1.222.0; one job packages as `v0.7.0`; neither CI nor `build.sh` installs or checks the pinned Roc nightly. | Medium | read |
| KI-13 | RPC always constructs the target with an empty constructor (`"{}"`), so agents with constructor arguments cannot be addressed. | Medium | read |
| KI-14 | Canary example fakes its timer (`timerHandle = 42`) and webhook URL, defines `renderDashboard` but never serves it, and has an unused variable. | Medium | read |
| KI-15 | README is stale: deploy command uses `golem:agent-platform/agent-api.{handle-message}`; hash points to v0.8.0; host comment still says "Golem 1.5.0". | Medium | read |
| KI-16 | `golem.yaml` has a top-level `tools:` key that does not appear in the 1.6.0 manifest schema as far as known. | Low | read, unverified |
| KI-17 | `.agents/skills/roc-golem.md` and `.gemini/skills/roc-golem.md` are identical copies. | Low | read |
| KI-18 | `host/Cargo.toml` still says version 0.1.0 and "Contributors". | Low | read |
| KI-19 | Native `cargo test --all-targets` fails to link on Linux: GNU `ld` version-script parser chokes on WIT exported symbol names containing `:`, `@`, and `#` when building `cdylib`. `cargo test --lib` passes cleanly. | Medium | verified by running |

## Open questions

- Should file effects be supported through `wasi:filesystem`, or removed from the SDK? (unit 07)
- Where should secrets come from on Golem: `wasi:config/store`, an agent config value, or another import? (unit 06)
- Should HTTP mounts be declared in agent metadata (replacing the `canary-release` special case)? (unit 10)

## Next steps

1. Commit the context pack.
2. Spec and build unit 01 (real Roc build), then units 02 to 04.
