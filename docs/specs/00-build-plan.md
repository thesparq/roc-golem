# Build Plan

Each unit is one change that can be verified on its own. Write a spec for a unit (see `PROMPTS.md`, "Spec Prompt") before implementing it. Status values: `Not started`, `Spec written`, `In progress`, `Implemented, awaiting verification`, `Complete`. Issue IDs (KI-nn) refer to `docs/progress-tracker.md`.

## Phase 0 — Make it real

**01. Real Roc build**
*What it builds:* `tooling/build.sh` compiles and links the Roc application with the pre-compiled host. Decouples `stub-guest` into an optional test-only feature in `host/Cargo.toml`; builds the shipped host library `libhost.a` with `--no-default-features` so the fake agent is excluded. Compiles Roc application code (`roc build --target wasm32 --no-link <app.roc>`), links with `wasm-ld` against `libhost.a`, and componentizes the resulting WebAssembly binary with `wasm-tools component new`. Each example produces a distinct component. The build script uses the Roc nightly pinned in `docs/architecture.md` and fails with a clear message if the installed `roc` is a different build.
*Dependencies:* none. Needs approval to touch `host/Cargo.toml` and `tooling/build.sh`.
*Fixes:* KI-01, KI-02, part of KI-12.
*Status:* Complete.

**02. Valid reply envelopes**
*What it builds:* Fixes the state JSON interpolation bug in `platform/main.roc` where unescaped state JSON (`"state":"${res.nextStateJson}"`) produces malformed envelope strings. Applies a shared JSON-escape step for every dynamic string in `platform/main.roc`, including the state (or outputs raw valid JSON envelopes). Adds golden-envelope and round-trip tests through real Roc.
*Dependencies:* 01.
*Fixes:* KI-03.
*Status:* Complete.

**03. Roc in CI**
*What it builds:* One CI workflow that installs the pinned Roc nightly and runs `roc fmt --check`, `roc check` for the platform and every example, the host tests, the `wasm32` build, real builds of every example, and component validation. Replaces `ci.yaml` and `ci.yml`. Pins one `wasm-tools` version. Resolves the Linux `cargo test` failure by gating `cdylib` or avoiding cdylib test link on Linux.
*Dependencies:* 01. Needs approval to change `.github/workflows/*`.
*Fixes:* KI-08, rest of KI-12, KI-19.
*Status:* Complete.

**04. Layer 4 runtime test**
*What it builds:* A runtime test that deploys the `counter` component built from real Roc to a local Golem server 1.6, invokes it, restarts the worker, and checks the count. The exact commands are recorded in `docs/feature-checklist.md`.
*Dependencies:* 01, 02, 03.
*Status:* Complete.

## Phase 1 — Make every effect honest

**05. Real scheduling**
*What it builds:* `rocFxRpcSchedule`, `rocFxRpcScheduleBytes` call the Golem scheduling operation on `wasm32`; the mock is a faithful stand-in. A test per variant.
*Dependencies:* 04.
*Fixes:* KI-04.
*Status:* Complete.

**06. Secrets from Golem**
*What it builds:* `getSecretHandle` reads from a source that exists on the target (`wasi:config/store` or agent configuration), returns a clear error when absent. Test for found and not-found.
*Dependencies:* 04. Needs approval for any change in `wit/world.wit`.
*Fixes:* KI-05.
*Status:* Complete.

**07. File effects**
*What it builds:* Real file effects (`readFile`, `writeFile`, `appendFileBytes`, `readFileBytes`, `writeFileBytes`) on `wasm32` through standard WASI 0.2 `wasi:filesystem/preopens` and `wasi:filesystem/types` using preopened directory descriptors, with native `std::fs` fallback for local testing.
*Dependencies:* 04. Needs approval for any change in `wit/world.wit` (Approved).
*Fixes:* KI-06.
*Status:* Complete.

**08. Logging**
*What it builds:* `rocFxLog` writes to a place that Golem captures on `wasm32` (`wasi:logging/logging` interface instead of unrouted `println!`).
*Dependencies:* 04. Needs approval for any change in `wit/world.wit` (Approved).
*Fixes:* KI-07.
*Status:* Complete.

**09. Surface handler errors**
*What it builds:* A failing message handler returns its own error; the fallback-to-tool-call path only runs when the method is genuinely a tool.
*Dependencies:* 02.
*Fixes:* KI-10.
*Status:* Complete.

## Phase 2 — Make the SDK and examples trustworthy

**10. Shared JSON helpers and honest examples**
*What it builds:* One shared set of encode/decode helpers used by every example, replacing ad-hoc text search (`Str.split_first`) that breaks on spaces and escapes. The README counter and `ai_tool` are fixed. Round-trip tests for every example.
*Dependencies:* 02.
*Fixes:* KI-09.
*Status:* Complete.

**11. Platform neutrality**
*What it builds:* HTTP mounts are declared in agent metadata; the `canary-release` special case in `host/src/lib.rs` is removed. RPC accepts constructor arguments so a specific agent instance can be addressed.
*Dependencies:* 04. Touches the metadata contract in `docs/architecture.md`.
*Fixes:* KI-11, KI-13.
*Status:* Complete.

**12. Canary example made real**
*What it builds:* Real durable timer, real promise and webhook for approval, the dashboard actually served, unused code removed.
*Dependencies:* 05, 11.
*Fixes:* KI-14.
*Status:* Complete.

## Phase 3 — Release

**13. Docs, manifest and release**
*What it builds:* README refreshed (deploy command, snippets compile, hash), `golem.yaml` checked against the 1.6.0 schema, duplicate skill files reduced to one source, crate metadata fixed, release v0.9.0 cut from a clean tag.
*Dependencies:* 03 and whichever of 05 to 12 are done.
*Fixes:* KI-15, KI-16, KI-17, KI-18.
*Status:* Complete.

**14. End-to-end Golem Cloud verification and v1.0.0 GA**
*What it builds:* Deploys compiled Roc agents to live Golem Cloud worker infrastructure; verifies durable state persistence across worker suspension and restarts; verifies bi-directional WebSocket streaming on live Golem infrastructure; tags `v1.0.0` Production Release.
*Dependencies:* 13.
*Status:* Not started.
