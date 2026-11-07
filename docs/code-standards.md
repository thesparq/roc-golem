# Code Standards

## General

- **Keep modules small and single-purpose.** Every file has one clear responsibility. If a module does two unrelated things, split it. (`host/src/lib.rs` is already very large; do not make it larger without a unit that explicitly splits it.)
- **Fix root causes, do not layer workarounds.** When a bug appears, trace it to its source. Do not compensate in the examples, in the README, or in tests for a defect in the platform or the host.
- **Do not mix concerns across boundaries.** Roc agent code never touches WIT. The host never contains domain logic. The platform never contains example-specific logic. Tooling never contains runtime logic. (Boundaries are defined in `docs/architecture.md`.)
- **Explicit is better than implicit.** All contracts (the four entry points, the envelopes, the effects, the WIT world) are written down and typed. Defaults and fallback behavior are documented where they are defined.
- **Fail fast at the boundary.** Validate input where it enters: the envelope in the platform, the WIT values in the host, the effect arguments in each `rocFx*`.
- **No fake success.** Code must never return a success value for work it did not do. If something is unsupported on a target, return an error that says so.
- **No TODO markers in finished work.** The only allowed marker is `TODO(SPEC)` for a blocked requirement, per `docs/ai-workflow-rules.md`.

## Roc

- **Use the current Roc syntax of the pinned toolchain.** `snake_case` standard library names (`Str.split_first`, `List.map`), `match` for branching, `or` / `and` instead of `||` / `&&`, `|args| body` lambdas, `?` and `??` as in the existing code. The repo's history shows several syntax migrations; do not reintroduce old forms.
- **Formatting.** Code must be `roc fmt` clean. Use tabs, as the existing files do.
- **Types on every top-level definition.** Every function in `platform/` and `examples/` has a type annotation. Public items are listed in the module header and documented with `##` comments.
- **Names.** Public SDK functions follow the existing `camelCase` (`registerReadOnlyTyped`). Internal helpers and platform entry points use `snake_case` (`main_init_for_host`, `escape_json_str`). Types are `PascalCase`. Do not rename public SDK items without a unit that says so (it breaks every consumer).
- **Handlers are pure.** An agent handler takes state and input and returns a `Result`. Side effects go through the effect wrappers only.
- **Return `Result`, never crash.** No `crash`, no `expect` in platform code. Convert every failure into `Err(Str)` with a message that names what failed.
- **JSON in Roc.**
  - There is no JSON parser. Use the shared helpers in the platform or in `examples/canary_release/Types.roc` style codecs. Do not write a new `Str.split_first` field search inside an agent.
  - Every dynamic string put into JSON goes through the one shared escape function. Escape quotes, backslashes, newlines, carriage returns and tabs, and control characters.
  - A state string embedded in an envelope is escaped like any other string.
- **Typed methods.** Declare each method once as `Method(input, output)`, and register it with the matching `register*Typed` builder with explicit encode/decode functions.
- **Read-only means read-only.** `registerReadOnly*` handlers must not change state and must return an empty state string in the reply.
- **State encode/decode are a pair.** `deserializeState(serializeState(s))` must equal `s` for every reachable `s`. Every agent in `examples/` has a test for this (see `docs/testing.md`).

## Rust host

- **Stable ABI names.** `#[no_mangle] pub unsafe extern "C" fn rocFx...` names and the four `roc__main_*` symbols are fixed. Never rename or change a signature without a unit that changes `platform/Effect.roc` in the same step.
- **Every `rocFx*` follows the same shape:**
  1. read arguments, treating a null pointer as the documented default;
  2. the `#[cfg(target_arch = "wasm32")]` branch performs the real work;
  3. a `#[cfg(not(target_arch = "wasm32"))]` branch exists only if tests need it, and it must be a faithful stand-in (principle 1);
  4. write exactly one result to `out`.
- **Never `panic!`, `unwrap()` or `expect()` on external input** in code that runs in the shipped component. Return a `RocResult::err(...)` with a clear message.
- **Memory ownership.** Strings and lists crossing the ABI are `RocStr` / `RocList` from `roc_std.rs`. Document who allocates and who frees at every `unsafe` block, and keep `unsafe` blocks as small as possible.
- **No standard library features the target lacks.** Do not use `std::fs`, `std::env`, `std::thread` or `println!` in the `wasm32` path unless a WASI import in `wit/world.wit` provides it.
- **Feature flags.** The feature `stub-guest` is for tests. Shipped builds use `--no-default-features`. Do not add other features without a unit.
- **Formatting and lint.** `cargo fmt` clean and `cargo clippy --all-targets -- -D warnings` clean. Do not add `#[allow(...)]` to silence a real problem; if an allow is justified, add a comment saying why.
- **Dependencies.** Add a crate only through a unit that names it. Keep `wit-bindgen` and `serde_json` as they are unless a unit upgrades them.

## WIT and the manifest

- **`wit/world.wit` is the contract.** Imports and exports change only through a unit, and only together with the host code that uses them.
- **`wit/deps/**` is vendored.** Do not hand-edit files in it. Update them only by a documented refresh from the upstream Golem WIT, in a unit that says which version.
- **One version of each interface.** The versions used in `world.wit` (for example `golem:agent/guest@2.0.0`, `golem:api/host@1.5.0`) are listed in `docs/architecture.md`. Mixed versions need a comment explaining why.
- **`golem.yaml` follows the manifest schema** named at its top (`manifestVersion`). Do not add keys that the schema does not define.

## Tooling and CI

- **`tooling/build.sh`** must `set -euo pipefail`, fail on any missing prerequisite with a clear message, and never silently skip a step (for example, "if `roc` is installed, run it" is not acceptable for a build step).
- **One CI workflow per purpose.** `ci.yaml` / `ci.yml` duplicates are not allowed. Release is separate from CI.
- **Pinned tools.** CI installs a specific version of each tool (Rust target, `wasm-tools`, Roc nightly).
- **Releases.** A release is built only from a tag by `release.yaml`. The README's snippet (URL and hash) is updated in the same unit as the release.

## Examples

- Examples use only the public SDK and **must compile with the real Roc compiler**.
- Examples must not fake results. A timer handle, webhook URL or similar value that looks real but is hard-coded is not allowed; use the real effect or remove the claim.
- No unused definitions or variables. Code that is written but not wired in (for example a defined-but-never-served page) must be wired in or deleted.
- Each example states, in a comment at the top, which capabilities it demonstrates.

## Documentation

- The README, `.agents/skills/roc-golem.md` and `.gemini/skills/roc-golem.md` must show code that compiles with the current SDK. The README's code snippets are checked by `roc check` (see `docs/testing.md`).
- **Keep documentation concrete.** Use the real names: `rocFxGetSecretHandle`, `registerReadOnlyTyped`, `golem:agent/guest@2.0.0`. Do not write "the secret function" or "the interface".
- Doc comments in Roc use `##`. Rust items that are `pub` and unsafe have a `# Safety` section.

## Naming, files and commits

- **Branches:** `feat/NN-short-name`, `fix/NN-short-name`, `docs/NN-short-name`, matching the unit number in `docs/specs/`.
- **Commits:** Conventional style, as in the history: `feat(scope): ...`, `fix(scope): ...`, `docs: ...`, `chore(scope): ...`. Scopes in use: `golem`, `roc`, `build`, `wit`, `manifest`, `release`, `host`, `platform`.
- **One unit, one branch, one PR.** Do not combine units.
- **File placement:** platform code in `platform/`, host code in `host/src/`, examples in `examples/<name>/main.roc`, specs in `docs/specs/`, scratch notes in `docs/temp/` (never referenced by other docs).

## Testing

The full strategy is in `docs/testing.md`. Standards that apply to all code:

- **Every change ships with a test that fails without it**, or with a written reason why that is not possible.
- **Every effect has a test for both its error path and its success path.**
- **Every example has a state round-trip test** and a test of at least one handler, run through real Roc.
- **Tests must not assert on a fake.** A test of agent behavior that only exercises `guest_bridge.rs` proves nothing about Roc code and must be labeled as a host test.
- **Never mark a check as passed that was not run.**
