# Architecture

What the repository contains today, the contracts between its parts, and the rules that keep it coherent. `docs/progress-tracker.md` holds the live state and the open work. Where the code currently breaks a rule below, the deviation is listed in "Known deviations" at the end, so this file stays honest.

## Stack

| Layer | Technology | Role |
| :--- | :--- | :--- |
| Agent code | Roc nightly `nightly-2026-09-29-7f11a82` (new compiler; syntax: `snake_case` standard library, `match`, `or`/`and`, `\|x\|` lambdas) | The application author's pure functions |
| Platform | Roc platform `platform/main.roc` with `Golem.roc`, `Effect.roc`, `Types.roc` | The SDK the author imports, and the entry points the host calls |
| Host | Rust (`host/`, crate `roc-golem-host`; crate types `staticlib`, `cdylib`, `rlib`; `wit-bindgen` 0.36.0; `serde_json`) | Translates between Roc text messages and Golem's WIT interfaces; performs effects |
| Interface definitions | WIT in `wit/` (`world.wit`, vendored `wit/deps/**`) | The contract with Golem: exports `golem:agent/guest@2.0.0`; imports Golem host APIs and WASI |
| Componentizing | `wasm-tools` (CI pins 1.220.0 in two workflows and 1.222.0 in one) | Wraps the core WebAssembly module as a Component Model component |
| Target | `wasm32-unknown-unknown` | Build target of the host and of every agent |
| Runtime | Local Golem server, version 1.6 (the runtime for all runtime tests), manifest `golem.yaml` (`manifestVersion: 1.6.0`); Golem Cloud in production | Runs components as durable workers |
| Packaging | `tooling/pack_platform` (Rust), BLAKE3 base64url checksums | Release archives `.tar.zst`, `.tar.br`, `.tar.gz` |
| CI / release | GitHub Actions (`.github/workflows/`) | Test, build, validate, publish |

**Pinned toolchain.** Roc: `nightly-2026-09-29-7f11a82`, from the `roc-lang/nightlies` releases (asset `roc_nightly-linux_x86_64-2026-09-29-7f11a82.tar.gz`), the same pin the school-management-system repository uses in its `Dockerfile.app`. This is the only place the Roc version is declared. `tooling/build.sh` and CI must use exactly this version (units 01 and 03 in `docs/specs/00-build-plan.md`). Changing it is a unit of its own: run `roc check` on the platform and on every example, and update this paragraph and the Stack table in the same change.

## Repository layout

```
AGENTS.md, PROMPTS.md     the entry point and the fixed prompts for coding agents
docs/                     these context files; docs/specs/ holds the unit specs
platform/
  main.roc                the platform definition: requires, provides, the 4 entry points, JSON envelopes
  Golem.roc               the SDK: Agent builders, Method, bundle, effect wrappers (public API)
  Effect.roc              low-level declarations of every host effect (1:1 with rocFx* in the host)
  Types.roc               shared data types: Metadata, MethodDeclaration, ToolDefinition, ToolCall, ToolResult, ...
host/
  src/lib.rs              the translation layer: WIT exports, discovery, dispatch, every rocFx* effect
  src/roc_std.rs          Roc ABI types for wasm32: RocStr, RocList, RocResult, allocator
  src/guest_bridge.rs     TEST-ONLY fake Roc agent (feature `stub-guest`)
  Cargo.toml              crate config; feature `stub-guest` (currently on by default)
wit/
  world.wit               the component's world (imports and the guest export)
  deps/                   vendored WIT packages (cli, clocks, http, io, keyvalue, blobstore, golem-*, ...)
examples/                 counter, ai_tool, streaming_agent, canary_release (multi-agent bundle)
tooling/
  build.sh                phases: platform, app, package, release
  pack_platform/          release archive and checksum tool
golem.yaml                Golem application manifest (canary_release component and HTTP deployment)
.github/workflows/        ci.yaml, ci.yml, release.yaml
.agents/skills/, .gemini/skills/   copies of the same Roc-Golem authoring skill for AI tools
```

## How a call travels

```
Golem  --WIT call-->  host (lib.rs)  --C ABI, text-->  Roc entry point (platform/main.roc)
                                                              |
                                                       target agent's handler (user code)
                                                              |
Golem  <--WIT result--  host  <--C ABI, text--  { state, response } JSON text
```

1. Golem calls the component's `golem:agent/guest@2.0.0` export (`initialize`, `invoke`, `get-definition`, `discover-agent-types`, ...).
2. The host finds the target agent type and builds a JSON **envelope** (see below).
3. The host calls one of four Roc entry points through the C ABI, passing the saved state as text.
4. Roc finds the agent by name, decodes inputs, runs the handler, encodes the new state, and returns one JSON text.
5. The host reads the reply, saves the new state if it is not empty, and returns the answer to Golem.

## The Roc-to-host contract

This contract is the most important thing in the repository. Changing it means changing both sides, the tests, and this section, in one unit.

### The four entry points

The exported symbol names are part of the ABI and are fixed:

| Symbol (Roc `provides`) | Roc function | Input | Output |
| :--- | :--- | :--- | :--- |
| `roc__main_init_for_host_1_exposed_generic` | `main_init_for_host` | `Str` config (or envelope with `agent_type` and `config`) | `Result(Str, Str)`: initial state JSON |
| `roc__main_handle_message_for_host_1_exposed_generic` | `main_handle_message_for_host` | state `Str`, envelope `Str` | `Result(Str, Str)`: reply envelope |
| `roc__main_handle_tool_call_for_host_1_exposed_generic` | `main_handle_tool_call_for_host` | state `Str`, tool-call JSON `Str` | `Result(Str, Str)`: tool reply |
| `roc__main_metadata_for_host_1_exposed_generic` | `main_metadata_for_host` | none | `Str`: metadata JSON |

### Envelopes

- **Request** to `handle_message`: `{"agent_type":"...","method":"...","input":"..."}`. An empty or `handle-message` / `message` method means "plain message"; any other name means a registered typed method.
- **Reply** from `handle_message`: `{"state":"<state text>","response":"<answer text>"}`.
- **Tool call** request: `{"id":"...","name":"...","arguments":"...","agent_type":"..."}`. **Tool reply**: `{"state":"...","success":true|false,"output":"..."}`.
- **Metadata**: `{"agents":[{"name","version","description","mode":"durable"|"ephemeral","methods":[{"name","description","prompt_hint","read_only"}],"tools":[{"name","description","parameters":[{"name","description","type","required"}]}]}]}`.

### State rules

- **State is a text string** (the agent's own JSON). The platform never interprets it.
- **An empty `state` string in a reply means "state unchanged"**. The host then skips saving. Read-only methods and ephemeral agents rely on this ("zero-serialization bypass").
- **Ephemeral agents** always receive `{}` as state and never have anything saved.
- **A worker initializes once** for one agent type. A second `initialize` fails; an unknown agent type fails.
- **Every string placed inside a JSON envelope must be valid JSON text.** That includes the state.

### Effects

- Each host ability is a function named `rocFx<Name>` exported by the host with the C ABI, declared in `platform/Effect.roc`, and wrapped for authors in `platform/Golem.roc`. The three lists must match one to one (53 effects today).
- Effects return `Result` with a text error. They never panic.
- The host has two builds of every effect: the **real** one (`#[cfg(target_arch = "wasm32")]`, calling Golem or WASI) and a **mock** one (native, used by tests). See principle 1.

## System boundaries

- **Roc agent code** knows nothing about WIT, Golem types, or the host. It only sees text and the effect wrappers.
- **The platform (`platform/`)** knows the envelope format and the agent builders. It knows nothing about any specific example.
- **The host (`host/`)** knows WIT and Golem. It knows nothing about any user's domain, and nothing about any specific example.
- **`wit/`** is the contract with Golem. It changes only when moving to a new Golem interface version.
- **`tooling/`** builds and packages. It never contains runtime logic.
- **Examples** use only the public SDK (`pf.Golem`, `pf.Types`).

## Principles

1. **A mock may never succeed where the real code would not.** Every `rocFx*` has a real `wasm32` branch and, if it needs one, a mock native branch. A function with no `wasm32` branch is a function that does nothing in production, and is forbidden. A mock that returns success without doing the work on `wasm32` is forbidden. (Example of a violation: scheduling returns "scheduled" but schedules nothing.)
2. **The fake agent is for tests only.** `guest_bridge.rs` (feature `stub-guest`) must never be compiled into a shipped library or component. Shipped builds use `--no-default-features`.
3. **Real Roc code is part of the build.** An agent is not "built" until the Roc compiler has compiled and linked it. A build step that only repackages the host is not a build.
4. **Every JSON envelope must be valid JSON for every possible input.** All dynamic text (state, responses, errors, names) is escaped by one shared function before it is placed in a string. Quotes, backslashes, newlines, tabs and non-ASCII characters must survive a round trip.
5. **State lives in the agent's own saved JSON, and nothing else.** No hidden globals that need to survive a restart. Process-local tables in the host (secret handles, WebSocket handles, promise handles, streams) are valid only within one invocation and must be documented as such.
6. **No JSON parser in the Roc platform.** Payloads are read with explicit, small scanners that live in one place and handle escapes. Examples must use the shared codecs, not their own `Str.split_first` tricks.
7. **The WebAssembly target has no operating system.** `wasm32-unknown-unknown` has no filesystem, environment variables, or standard output. Anything an effect needs from outside must come through a WIT import declared in `wit/world.wit` (for example `wasi:config/store`, `wasi:filesystem`, `wasi:cli`), or the effect must return a clear "unsupported" error.
8. **Secrets stay out of agent memory.** Agents hold opaque handles. The host injects values into outgoing requests. Revealing a secret is an explicit, opt-in call.
9. **Errors are surfaced, not hidden.** A failing handler returns its own error. The host must not retry a failed call as a different kind of call and report a different error.
10. **The platform and host contain no example-specific logic.** Behavior that an agent needs (for instance an HTTP mount) is declared in the agent's metadata, not switched on by agent name in the host.
11. **One public API, four views.** `Effect.roc`, `Golem.roc`, the README, and `.agents/skills/roc-golem.md` (and its copy in `.gemini/skills/`) describe the same API. Changing one means checking the others.
12. **Versions are declared once.** The WIT versions in `wit/world.wit`, the manifest version in `golem.yaml`, the `wasm-tools` version in CI, and the Roc nightly are each stated in one place and referenced from `docs/architecture.md` (this file).
13. **Verify against something that behaves like production.** A check that passes against a fake proves the fake. See `docs/testing.md` for what each layer proves.

## Known deviations from these principles

Tracked with IDs in `docs/progress-tracker.md`. As of 2026-11-07 they are:

| Principle | Deviation (ID in tracker) |
| :--- | :--- |
| 1 | `rpcSchedule` and its bytes variant do nothing on any target (KI-04). Secrets read from environment variables, which do not exist on this target (KI-05). Filesystem effects use `std::fs` (KI-06). Log lines use `println!` (KI-07). |
| 2, 3 | `build.sh` never compiles Roc; `stub-guest` is on by default and compiled into the shipped library (KI-01, KI-02). |
| 4 | The reply envelope puts the state inside a JSON string without escaping it (KI-03). |
| 6 | Examples parse JSON with ad-hoc text searches; the README counter's `deserializeState` ignores its input (KI-09). |
| 9 | On any handler error the host retries as a tool call and reports a different error (KI-10). |
| 10 | The host switches on the agent name `canary-release` to add an HTTP mount (KI-11). |
| 12 | CI uses two `wasm-tools` versions, and neither CI nor `build.sh` installs or checks the pinned Roc nightly (KI-12). |
