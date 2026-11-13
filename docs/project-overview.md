# Project Overview: Roc Golem Platform

## What this is

**roc-golem** lets you write [Golem Cloud](https://golem.cloud) agents in [Roc](https://www.roc-lang.org).

Golem is a platform that runs WebAssembly programs as long-lived, durable "workers": if a worker's machine dies, Golem replays its history and the worker carries on as if nothing happened. Normally you write those programs in Rust, TypeScript or MoonBit, and you have to understand WebAssembly component interfaces (WIT) to do it.

This project is the translation layer that removes that burden for Roc:

- You write a small, pure Roc program: "given my current state and a message, here is my new state and my answer."
- The **platform** (Roc code in this repo) turns that into plain text messages.
- A pre-built **Rust host** (also in this repo) turns those messages into the real Golem calls, and gives the Roc program access to Golem's abilities (HTTP, key-value, databases, timers, calls to other agents, and so on).

The mental model, top to bottom:

```
Your Roc agent         (pure functions: state + message -> new state + answer)
      |
Roc platform           (platform/*.roc: packs and unpacks text messages)
      |
Rust host              (host/src: speaks WIT / Golem, performs the real effects)
      |
Golem Cloud            (runs the component, stores history, replays on failure)
```

## Goals

1. **A Roc developer can ship a Golem agent without knowing Rust or WIT.** They write Roc, run one build command, and deploy.
2. **Agents behave the same locally and on Golem.** What a test shows is what production does. No effect may pretend to succeed.
3. **Typed and safe at the Roc boundary.** Methods are declared with types; mistakes are caught when the Roc code is compiled, not at runtime.
4. **Durable by default, cheap when possible.** Durable agents keep state across restarts; ephemeral agents and read-only methods skip state saving entirely.
5. **Easy to consume.** A release is a single archive with a checksum that can be pasted into a Roc `app` header.
6. **Deterministic to develop.** The repo carries its own written rules (this `docs/` folder) so that a human or an AI coding agent produces the same kind of change every time.

## Core User Flow (step by step)

1. The developer writes `main.roc` that declares an `agent` (state type, `init`, message/method handlers, tools, state encode/decode).
2. They build it with `./tooling/build.sh <app>`. The build compiles the Roc code, links it with the pre-compiled host, and produces a WebAssembly component.
3. They deploy the component with the Golem CLI (`golem component add ...`), or through the manifest in `golem.yaml`.
4. Golem creates a worker for an agent type. The host's `initialize` runs the Roc `init` once.
5. A caller sends a message, a typed method call, or a tool call. The host passes the saved state and the message to Roc, receives the new state and the answer, saves the state, and replies.
6. If the worker is restarted, Golem replays its history and the agent continues with the same state.
7. Agents can call other agents (RPC), call external APIs, wait on timers, create promises and webhooks, and talk to databases. Each of these goes through a named **effect**.

## Features Breakdown

### Agent authoring (Roc side, `platform/Golem.roc`)

- `define(...)` for durable agents with state; `defineEphemeral(...)` for stateless workers; `defineAgent(...)` as the simple single-agent form.
- Builders: `register`, `registerTyped`, `registerReadOnly`, `registerReadOnlyTyped`, `registerEphemeral`, `registerEphemeralTyped`, `registerEffectful`, `registerEffectfulTyped`, then `build`.
- Tools (for AI callers): `withTools`, `withToolHandler`, `withStatelessToolHandler`, `withEffectfulToolHandler`, `withEffectfulStatelessToolHandler`.
- `bundle([...])` puts several agents, each with its own state type, into one component.
- Phantom-typed `Method(input, output)` handles give compile-time checking for method registration and RPC.

### Agent kinds

- **Durable**: state is saved after each call that changes it.
- **Ephemeral**: no saved state; every call starts from `{}`.
- **Read-only methods**: report "state unchanged" with an empty state string, so the host skips saving.

### Platform effects (Roc functions backed by the Rust host)

| Group | What it gives an agent |
| :--- | :--- |
| Logging and identity | `logTrace` … `logError`, `workerId` |
| Agent-to-agent calls | `rpc`, `rpcSchedule`, `rpcFireAndForget`, typed (`...Typed`) and bytes (`...Bytes`) variants |
| HTTP | `http`, `httpRequestBytes`, streaming (`httpStreamOpen`, `httpStreamReadChunk`, `httpStreamClose`) |
| Storage | key-value (`kvGet`/`kvSet` + bytes), blobstore (`blobGet`/`blobPut` + bytes), files (`readFile`/`writeFile` + bytes) |
| Real-time | WebSocket connect / send / receive / close |
| Time | `sleep`, `now` |
| Secrets | `getSecretHandle`, `revealSecret`, host-injected secret HTTP headers |
| Durability controls | idempotency keys, oplog index and commit, idempotence mode, `revertAgent`, `setPersistence` |
| Waiting on the outside world | `createPromise`, `getPromise`, `completePromise`, `createWebhook` |
| Config and tracing | `configGet`, `traceId`, `spanId`, `traceHeaders` |
| Databases | `dbExecute`, `dbQuery`, and parameterized `dbExecuteParams`, `dbQueryParams` (Postgres and MySQL) |

### Build and distribution (`tooling/`)

- `build.sh` builds the host, builds apps, and packages releases.
- `pack_platform` creates `.tar.zst`, `.tar.br` and `.tar.gz` archives with BLAKE3 base64url checksums and release notes.
- GitHub Actions build, validate and publish releases on version tags.

### Examples (`examples/`)

`counter`, `ai_tool`, `streaming_agent`, and `canary_release` (a two-agent bundle: a stateful coordinator and a stateless diagnostics worker). Examples are teaching material and also serve as test inputs.

## Current state (honest summary)

The live list is in `docs/progress-tracker.md`.

- The platform is **production-ready and fully verified end to end** on Golem Cloud (v1.6.3), released as `v1.1.0`.
- The Rust host compiles for `wasm32-wasip2`, links against WASI `libc.a`, and translates WIT `golem:agent/guest@2.0.0` async Component Model interfaces into Roc handler calls.
- All four verification layers pass: Layer 1 static checks (`cargo fmt`, `clippy`, `roc fmt`, `roc check`), Layer 2 host unit tests and Roc tests, Layer 3 component builds, and Layer 4 live runtime tests on local Golem server (v1.6.3).
- Platform distribution archives (`.tar.zst`, `.tar.br`, `.tar.gz`) are packaged with BLAKE3 base64url checksums and verified via automated consumer smoke tests.

## In Scope

- The Roc platform definition and SDK (`platform/`).
- The Rust host translation layer and its WIT bindings (`host/`, `wit/`).
- Building, packaging and releasing the platform (`tooling/`, `.github/workflows/`).
- Examples that demonstrate and test each capability.
- The documentation and rules in `docs/`, `AGENTS.md` and `PROMPTS.md`.
- Compatibility with the Golem version named in `golem.yaml` and `wit/world.wit`.

## Out of Scope

- Changing Roc the language, or its compiler.
- Changing Golem itself.
- A general JSON library for Roc (the platform uses small, explicit helpers; see `docs/architecture.md`).
- Hosting or operating Golem Cloud, or any production application built on this platform (those live in their own repositories).
- Supporting more than one Golem major version at a time.
- Languages other than Roc for writing agents.

## Success Criteria

The project is "working" when all of these are true and demonstrated by the checks in `docs/testing.md`:

1. `./tooling/build.sh <example>` compiles real Roc code and produces a **different** component for each example.
2. `roc check` reports 0 errors and 0 warnings of our own for the platform and every example.
3. The counter example, deployed to a local Golem server (version 1.6), keeps its count across worker restarts.
4. State containing quotes, backslashes, newlines and non-ASCII text survives a save-and-load round trip unchanged.
5. Every effect either does the real thing on WebAssembly or returns a clear error. None reports success without doing the work.
6. A tagged release is built from a clean checkout by CI, and the checksum in the README matches the published archive.
7. The README, the examples and these docs describe the same API and the same behavior.
