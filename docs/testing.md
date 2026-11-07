# Testing System

A passing test only proves something about the thing it actually ran. This project has four layers of checks. Each layer proves something different, and the lower layers cannot stand in for the higher ones. Use this file to know what a green result means, and what it does not.

## Quick Start

```bash
# Layer 1 — static checks (fast, always available with Rust installed)
cargo fmt --manifest-path host/Cargo.toml --all -- --check
cargo clippy --manifest-path host/Cargo.toml --all-targets -- -D warnings
roc check platform/main.roc
roc check examples/counter/main.roc          # repeat for every example

# Layer 2 — host tests (native, uses the FAKE Roc agent)
cargo test --manifest-path host/Cargo.toml --all-targets

# Layer 3 — real WebAssembly build and validation
./tooling/build.sh platform
./tooling/build.sh counter                   # repeat for every example
wasm-tools validate build/counter_agent.wasm

# Layer 4 — behavior of real Roc code on a local Golem server 1.6 (see "Layer 4")
```

## Architecture of the checks

```
Layer 1  static        Is the code well-formed?                      proves: syntax, types, lint
Layer 2  host native   Does the Rust translation layer behave?       proves: host logic, against a FAKE agent
Layer 3  wasm build    Does a real component get produced?           proves: it builds and is a valid component
Layer 4  runtime       Does a real Roc agent behave on Golem?        proves: the platform works end to end
```

**What each layer cannot prove**

- Layer 2 never runs Roc. Its "agent" is `host/src/guest_bridge.rs`, a Rust file that imitates the examples with hard-coded answers (feature `stub-guest`). A green Layer 2 says nothing about `platform/*.roc` or `examples/**/*.roc`.
- Layer 2 also never runs the real `wasm32` branch of any effect. Those branches (`#[cfg(target_arch = "wasm32")]`) are compiled only in Layer 3, and exercised only in Layer 4.
- Layer 3 proves a component was produced, not that it does anything correct.

## Layer 1: Static checks

| Check | Command | Passes when |
| :--- | :--- | :--- |
| Rust format | `cargo fmt --manifest-path host/Cargo.toml --all -- --check` | no diff |
| Rust lint | `cargo clippy --manifest-path host/Cargo.toml --all-targets -- -D warnings` | 0 warnings |
| Roc format | `roc fmt --check <file>` (note: `build.sh` currently runs `roc fmt`, which rewrites files; CI must use the check form) | no diff |
| Roc compile | `roc check <file>` for `platform/main.roc` and each `examples/*/main.roc` | 0 errors and 0 warnings of our own |
| README snippets | extract each Roc code block from `README.md` and `roc check` it | all compile |

If `roc` is not installed, these checks are **not run**. Say so. Do not substitute `cargo` results.

## Layer 2: Host tests (native, fake agent)

`cargo test --manifest-path host/Cargo.toml --all-targets` runs the tests in `host/src/lib.rs` and `host/src/roc_std.rs` (13 today). They cover:

- agent discovery and the generated definitions (methods, tools, HTTP endpoints);
- initialize / invoke / double-initialize / unknown-agent behavior;
- the ephemeral and read-only bypass of state saving;
- binary I/O, secret handles and header injection, promises and webhooks, WebSocket and oplog calls, RPC and timers, config / tracing / idempotence / database calls.

These tests use the **mock** branches of effects (native build) and the **fake agent**. They are good tests of the host's logic. They are not tests of Roc.

Rules:

- Name host tests honestly. If a test asserts on an answer that only the fake agent produces, say so in a comment.
- Do not make a Layer 2 test depend on the fake agent for behavior that the real Roc code is supposed to own.

## Layer 3: Real WebAssembly build

| Check | Command | Passes when |
| :--- | :--- | :--- |
| Host for wasm32, no fake agent | `cargo build --manifest-path host/Cargo.toml --target wasm32-unknown-unknown --release --no-default-features` | builds |
| No fake-agent symbols in shipped library | list the symbols of the built library; none from `guest_bridge` are present, and the four `roc__main_*` symbols come from Roc | true |
| Roc app builds and links | `./tooling/build.sh <example>` | exits 0 |
| Distinct components | `sha256sum build/*_agent.wasm` | one distinct hash per example |
| Component is valid | `wasm-tools validate build/<example>_agent.wasm` | exits 0 |
| Component world is right | `wasm-tools component wit build/<example>_agent.wasm` | exports `golem:agent/guest@2.0.0`; imports match `wit/world.wit` |
| Packaging | `./tooling/build.sh package vX.Y.Z thesparq/roc-golem` | `dist/` has the three archives, `checksums.txt`, `release_notes.md` |

If the `wasm32-unknown-unknown` target or `wasm-tools` is not installed, these checks are **not run**.

## Layer 4: Runtime behavior (real Roc, real component)

This layer does not exist yet. Building it is unit 04 of `docs/specs/00-build-plan.md`. **The runner is a local Golem server, version 1.6** (the same version the manifest targets). Layer 4 must:

1. deploy a component built from real Roc code (no fake agent) to the local Golem server;
2. send real invocations (message, typed method, tool call);
3. assert on the replies and on saved state;
4. restart the worker and assert the state survived;
5. run in CI.

Until Layer 4 exists, no statement of the form "the agent works on Golem" may be made in the docs, the README or a PR description.

## Feature checklist

`docs/feature-checklist.md` lists, capability by capability, a test row that a person (or a script) runs against the local Golem server 1.6. The matrix below is about the build and the platform; the checklist is about what an agent author can rely on. As Layer 4 automates a checklist row, say so in that row. A row is only ticked as passed when it was actually run.

## Required checks matrix

Use this like a checklist. "Today" shows what exists in the repository now (2026-11-07). A unit that adds a check updates its row.

| # | Test | How | Expected | Today |
| :--- | :--- | :--- | :--- | :--- |
| 1 | Rust format | `cargo fmt ... --check` | no diff | exists (in `ci.yml`) |
| 2 | Rust lint | `cargo clippy ... -D warnings` | 0 warnings | exists (in `ci.yml`) |
| 3 | Roc check, platform | `roc check platform/main.roc` | 0 errors, 0 own warnings | missing |
| 4 | Roc check, every example | `roc check examples/*/main.roc` | 0 errors, 0 own warnings | missing |
| 5 | Roc format check | `roc fmt --check` | no diff | missing (`build.sh` rewrites instead) |
| 6 | Host native tests | `cargo test ...` | all pass | exists (13 tests) |
| 7 | Host builds for wasm32 without fake agent | `cargo build --target wasm32-unknown-unknown --release --no-default-features` | builds | missing (fake agent on by default) |
| 8 | No fake-agent symbols shipped | inspect built library symbols | none from `guest_bridge` | missing |
| 9 | Real Roc linked into each example | `./tooling/build.sh <example>`; compare `sha256sum` | one distinct hash per example | missing (Roc is never compiled) |
| 10 | Component validates | `wasm-tools validate` | exit 0 | partial (only `host.wasm` is validated) |
| 11 | Component world | `wasm-tools component wit` | exports `golem:agent/guest@2.0.0` | missing |
| 12 | Golden envelopes | fixed request JSON in, expected reply JSON out, through real Roc | byte-for-byte equal | missing |
| 13 | State round trip | state with quotes, backslashes, newlines, tabs, emoji, empty string, 10 KB text; save then load | equal to the original | missing |
| 14 | Reply envelope is valid JSON | feed every golden reply to a strict JSON parser | parses; `state` and `response` are strings | missing |
| 15 | Empty state means unchanged | call a read-only method twice | saved state identical; no save performed | host-only (fake agent); real Roc missing |
| 16 | Ephemeral agents keep nothing | two calls to a stateless worker | second call starts from `{}` | host-only (fake agent); real Roc missing |
| 17 | Handler errors surface | handler returns `Err("boom")` | caller receives "boom", not a different error | missing |
| 18 | ABI parity | compare the `rocFx*` names in `host/src/lib.rs`, `platform/Effect.roc`, `platform/Golem.roc` | the three sets match | manual only |
| 19 | Real-versus-mock parity | for each `rocFx*`, confirm a `wasm32` branch exists and no mock claims success the real one would not | none missing | manual only; known gaps KI-04 to KI-07 |
| 20 | Effects on Golem | one scripted check per effect group (HTTP, key-value, blobstore, WebSocket, promises and webhooks, database, schedule, secrets, config) on a local Golem | matches the effect's documented behavior | missing |
| 21 | Counter survives restart | deploy `counter`; `increment` three times; restart the worker; `get` | 3 | missing |
| 22 | Canary example end to end | deploy `canary_release`; start, approve, rollback, status | matches the example's documented flow, with real timers and webhooks | missing |
| 23 | Release archives | `./tooling/build.sh package ...` | three archives, checksums, notes present | exists (in `ci.yml`) |
| 24 | README hash matches release | download the published archive; recompute BLAKE3 base64url | equals the README URL's hash | manual only |
| 25 | Docs parity | README, skills and `docs/` describe the same API and effects count | no contradictions | manual only |

## How to run a parity check (rows 18 and 19)

```bash
# Effect names on each side
grep -o 'rocFx[A-Za-z0-9]*' host/src/lib.rs      | sort -u > /tmp/host_fx.txt
grep -o 'rocFx[A-Za-z0-9]*' platform/Effect.roc  | sort -u > /tmp/roc_fx.txt
diff /tmp/host_fx.txt /tmp/roc_fx.txt            # must print nothing

# Effects with no wasm32 branch: list every rocFx function and check each body for
#   #[cfg(target_arch = "wasm32")]
# A function with no wasm32 branch must either not need one (pure) or return "unsupported".
```

Turning this into a script that runs in CI is a unit in the build plan.

## Writing tests

- **Golden tests** (rows 12 and 14) are the main tool for determinism: a fixed input must always give a fixed output. Store golden files next to the example, named `<example>.golden.json`. Change one only with an explanation in the PR.
- **Round-trip tests** (row 13) use a fixed list of awkward strings. Keep that list in one shared file and reuse it for every example.
- **Test the error path of every effect**, not only success: missing secret, bad URL, closed WebSocket handle, invalid promise handle.
- **Tests must be deterministic.** No dependence on the wall clock, network or random values. Mock time and ids where a test needs them.
- **A bug fix starts with a failing test** that reproduces it.

## Failure modes and what to report

| Symptom | Likely cause | Report |
| :--- | :--- | :--- |
| Layer 2 passes, Layer 3 fails | a `wasm32` branch does not compile (these are never built natively) | the compiler error, and which effect |
| `build.sh <example>` succeeds but every example gives the same hash | the Roc app is not being linked | "build is repackaging the host only" |
| State is empty or reset between calls | the reply envelope is invalid JSON (state not escaped), or the example's decode ignores its input | the raw reply envelope and the state before and after |
| An effect "succeeds" but nothing happens on Golem | mock behavior on a real target, or a missing `wasm32` branch | the effect name and the missing branch |
| `getSecretHandle` fails on Golem but passes in tests | secrets read from environment variables, which this target lacks | the missing WIT import |
| `roc check` shows warnings from the Roc platform bundle itself | warnings from third-party sources, not ours | list them separately; do not count them as ours |

## Usage flow

```bash
# 1. Make a change in one unit

# 2. Static checks
cargo fmt --manifest-path host/Cargo.toml --all -- --check
cargo clippy --manifest-path host/Cargo.toml --all-targets -- -D warnings
roc check platform/main.roc

# 3. Host tests
cargo test --manifest-path host/Cargo.toml --all-targets

# 4. Real build of every affected example, then validate
./tooling/build.sh <example>
wasm-tools validate build/<example>_agent.wasm

# 5. Before opening the PR: run the full matrix rows that apply, and list in the PR
#    which rows passed, which were not run, and why.
```

## Files

| File | Purpose |
| :--- | :--- |
| `host/src/lib.rs` (`mod tests`) | host tests (Layer 2) |
| `host/src/guest_bridge.rs` | the fake Roc agent used by Layer 2 only |
| `host/src/roc_std.rs` (`mod tests`) | tests of the Roc ABI types |
| `tooling/build.sh` | Layer 3 builds |
| `.github/workflows/ci.yml`, `ci.yaml` | current CI (to be merged into one workflow) |
| `examples/*/main.roc` | inputs for Layers 3 and 4 |
| `docs/feature-checklist.md` | per-capability acceptance rows, run against the local Golem server 1.6 |
| `docs/specs/00-build-plan.md` | where each missing check is scheduled |
