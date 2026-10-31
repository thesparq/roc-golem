---
name: roc-golem-platform
description: Comprehensive guide and best practices for developing Roc agents on Golem Cloud with the roc-golem platform, covering multi-agent bundling, ephemeral workers, zero-bottleneck serialization bypass, stateless tool handlers, raw binary I/O, opaque secret handles, and typed RPC.
---

# Roc Golem Platform Development Skill

This skill provides guidelines and patterns for building and deploying WebAssembly agents on Golem Cloud using the Roc programming language.

## Architecture & Core Concepts

- **Pre-compiled Host Architecture**: The Rust translation layer (`host/src/lib.rs`) is pre-compiled into `build/libhost.a` and `build/host.wasm` (`wasm32-unknown-unknown`), implementing the official `golem:agent/guest` component model interface.
- **Universal Pass-Through Bridge**: Guest methods, tools, and message handling pass through JSON/string representations across the Roc↔Rust boundary.
- **Worker Instance Lifecycle**: In Golem Cloud, a worker instance is initialized for one specific `agent-type` and stays hot in memory.
- **Pure Functional Agents in Roc**: Agents are implemented as pure state machines with idiomatic Roc syntax, decoupled from low-level WebAssembly Component Model plumbing.

---

## Agent Patterns

### 1. Stateful Durable Agent (Builder Pattern)

Use `define`, `register`, and `registerReadOnly` with phantom-typed `Method(input, output)`:

```roc
import pf.Golem exposing [Agent, Method, define, register, registerReadOnly, method, build]

State : {
    count : I64,
}

incrementMethod : Method(Str, Str)
incrementMethod = method({
    name: "increment",
    description: "Increments the counter",
    promptHint: "Call to increment",
})

statusMethod : Method(Str, Str)
statusMethod = method({
    name: "get_status",
    description: "Returns current status",
    promptHint: "Query status",
})

agent : Agent(State)
agent =
    define(
        {
            name: "counter-agent",
            version: "1.0.0",
            description: "Durable Roc counter agent",
        },
        |_config| Ok({ count: 0 }),
        |state| "{\"count\":${Num.to_str(state.count)}}",
        |_json| Ok({ count: 0 }),
    )
        |> register(incrementMethod, handleIncrement)
        |> registerReadOnly(statusMethod, handleStatus)
        |> build

handleIncrement : State, Str -> Result({ state : State, out : Str }, Str)
handleIncrement = |state, _input|
    Ok({ state: { count: state.count + 1 }, out: "Incremented" })

# Read-only handlers bypass serialization completely (stateChanged: False)
handleStatus : State, Str -> Result(Str, Str)
handleStatus = |state, _input|
    Ok("Current count is ${Num.to_str(state.count)}")
```

### 1b. Effectful Method Handlers (`registerEffectfulTyped`)

When mutating methods need to invoke host effects (such as `rpcSchedule`, `http`, or `writeFileBytes`), use `registerEffectfulTyped`:

```roc
agent =
    define(...)
        |> registerEffectfulTyped(startCanary, handleStartEffectful, decodeStartInput, encodeStartOutput)
        |> build

handleStartEffectful : State, StartInput -> Effect.Effect(Result({ state : State, out : StartOutput }, Str))
handleStartEffectful = |state, input|
    Effect.map(
        rpcSchedule("worker-id", timeoutMethod, "payload", 300000),
        |_| Ok({ state: nextState, out: startOutput })
    )
```

### 2. Stateless Ephemeral Worker

For stateless tasks (log analysis, computation, formatting), use `defineEphemeral` and `registerEphemeral`:

```roc
import pf.Golem exposing [Agent, Method, defineEphemeral, registerEphemeral, method, build]

processMethod : Method(Str, Str)
processMethod = method({
    name: "process",
    description: "Stateless processing",
    promptHint: "Execute pure processing task",
})

agent : Agent({})
agent =
    defineEphemeral({
        name: "worker-agent",
        version: "1.0.0",
        description: "Stateless worker",
    })
        |> registerEphemeral(processMethod, handleProcess)
        |> build

handleProcess : Str -> Result(Str, Str)
handleProcess = |input|
    Ok("Processed: ${input}")
```

- Handlers take `input -> Result(out, Str)` directly without threading dummy `State`.
- Emits `AgentMode::Ephemeral` in WIT discovery; the host avoids storing or caching state between calls.

### 3. Multi-Agent Bundling in One Wasm Component

Combine multiple agents with completely different state types into a single component:

```roc
app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [bundle]
import ReleaseCoordinator
import DiagnosticsWorker

agent = bundle([
    ReleaseCoordinator.agent,
    DiagnosticsWorker.agent,
])
```

- Golem worker instances are created for a specific agent type (`golem worker add ...`).
- Dispatch automatically routes initialization and method invocations to the designated agent.

### 4. Stateless vs. Stateful Tool Handlers

For AI agent tool integrations:
- **Stateful Tool Handlers**: Use `withToolHandler(handler)` when tool calls need to read or update the agent's internal state:
  ```roc
  handleToolStateful : State, ToolCall -> Result({ state : State, result : ToolResult }, Str)
  ```
- **Stateless Tool Handlers**: Use `withStatelessToolHandler(handler)` when tool execution is pure and does not require agent state:
  ```roc
  handleToolStateless : ToolCall -> Result(ToolResult, Str)
  ```

---

## Zero-Bottleneck Serialization Bypass Protocol

- **Mutating calls**: Return `{ state: nextState, out: ... }` with `stateChanged: True`. The host updates `current_state_json`.
- **Read-only calls (`registerReadOnly`)**: Return `out` with `stateChanged: False`. Roc guest returns `nextStateJson: ""`. The host checks for an empty state string and skips re-serializing or overwriting cached state in $O(1)$ time.
- **Ephemeral calls (`registerEphemeral`)**: Operate with `stateChanged: False` and empty state string; host bypasses state updates entirely.

---

## Typed Cross-Agent RPC & Codec Combinators

Use phantom-typed `Method(in, out)` handles with `rpc`, `rpcSchedule`, and `rpcFireAndForget`, or with typed codecs `rpcTyped`, `rpcScheduleTyped`, and `rpcFireAndForgetTyped`:

```roc
import pf.Golem exposing [
    rpc,
    rpcSchedule,
    rpcFireAndForget,
    rpcTyped,
    rpcScheduleTyped,
    rpcFireAndForgetTyped,
]

# Basic RPC with string payloads
res = rpc("other-worker-id", OtherAgent.someMethod, "payload")

# Typed RPC with custom encoder and decoder
resTyped = rpcTyped(
    "other-worker-id",
    RollbackCoordinator.emergencyRollback,
    { reason: "latency spike" },
    encodeRollbackInput,
    decodeRollbackOutput,
)

# Scheduled invocation with delay in milliseconds
_ = rpcSchedule("other-worker-id", OtherAgent.someMethod, "payload", 5000)

# Fire-and-forget
_ = rpcFireAndForget("other-worker-id", OtherAgent.someMethod, "payload")
```

---

## Raw Binary Data Boundary & Lossy UTF-8 Remediation

Converting arbitrary binary data (PNG images, zip/tar archives, protobuf streams, compiled WebAssembly modules) via UTF-8 string decoders causes irreversible data corruption, as invalid byte sequences are replaced with `U+FFFD`.

`roc-golem` provides clean, raw `List(U8)` byte primitives across all storage and communication subsystems (backed by live `wasi:keyvalue/eventual@0.1.0`, `wasi:blobstore/blobstore`, and `wasi:http/outgoing-handler@0.2.3` on wasm32):

- **Filesystem**:
  - `readFileBytes : Str -> Effect(Result(List(U8), Str))`
  - `writeFileBytes : Str, List(U8) -> Effect(Result({}, Str))`
  - `appendFileBytes : Str, List(U8) -> Effect(Result({}, Str))`
- **Key-Value Store**:
  - `kvGetBytes : Str, Str -> Effect(Result(List(U8), Str))`
  - `kvSetBytes : Str, Str, List(U8) -> Effect(Result({}, Str))`
- **Blobstore**:
  - `blobGetBytes : Str, Str -> Effect(Result(List(U8), Str))`
  - `blobPutBytes : Str, Str, List(U8) -> Effect(Result({}, Str))`
- **HTTP**:
  - `httpRequestBytes : Str, List(U8) -> Effect(Result(List(U8), Str))`

---

## Opaque Secret Handles & Host-Mediated Security

To ensure API tokens, cryptographic keys, and database passwords never reside in guest linear memory or leak into durable state snapshots:

1. **Acquire an Opaque Handle**:
   ```roc
   apiKeyHandle : Result(SecretHandle, Str)
   apiKeyHandle = getSecretHandle("OPENAI_API_KEY")
   ```
   Secrets are indexed as opaque `SecretHandle : U32` identifiers in the host's isolated memory table.

2. **Host-Mediated Outbound Injection**:
   Specify headers referencing the secret handle in HTTP requests:
   ```roc
   secretHeader : SecretHeader
   secretHeader = {
       name: "Authorization",
       prefix: "Bearer ",
       handle: apiKeyHandle,
   }
   ```
   When performing `http(requestJson)`, the Golem host resolves the secret handle directly into the WASI HTTP request headers without passing plaintext to the guest.

3. **Explicit Reveal (Opt-In)**:
   When plaintext is strictly required for cryptographic hashing or client signing:
   ```roc
   secretText : Result(Str, Str)
   secretText = revealSecret(handle)
   ```

---

## Durability, Timers & Golem 1.6 Host Primitives

- **Durable Sleep**: `sleep(millis)` uses Golem's durable timer subsystem; workers pause execution without consuming CPU, surviving restarts.
- **Monotonic Timestamp**: `now` yields the current Unix timestamp in milliseconds.
- **Persistence Modes**: `setPersistence(mode)` configures Golem persistence (`PersistNothing`, `PersistRemoteSideEffects`, `Smart`).
- **Durable Promises & Webhooks**:
  - `createPromise : Effect(Result(U64, Str))` — allocates a durable promise handle.
  - `createWebhook : U64 -> Effect(Result(Str, Str))` — returns an external webhook URL associated with the promise.
  - `completePromise : U64, List(U8) -> Effect(Result(Bool, Str))` — completes the promise with byte payload.
  - `getPromise : U64 -> Effect(Result(List(U8), Str))` — retrieves promise result or returns Err if still pending.
- **Idempotency & Oplog Controls**:
  - `generateIdempotencyKey : Effect(Result(Str, Str))` — generates unique idempotency UUID.
  - `getOplogIndex : Effect(U64)` — retrieves current oplog index.
  - `oplogCommit : U8 -> Effect({})` — commits oplog up to current index with replica quorum.
  - `getIdempotenceMode : Effect(Bool)` / `setIdempotenceMode : Bool -> Effect({})` — inspects and toggles idempotency mode.
  - `revertAgent : U64 -> Effect(Result({}, Str))` — rolls back agent state to specified oplog index.
- **WebSockets**:
  - `websocketConnect : Str -> Effect(Result(U32, Str))`
  - `websocketSend : U32, Str -> Effect(Result({}, Str))`
  - `websocketReceive : U32 -> Effect(Result(Str, Str))`
  - `websocketClose : U32 -> Effect(Result({}, Str))`
- **Durable RDBMS (PostgreSQL & MySQL)**:
  - `dbExecute : Str, Str -> Effect(Result(U64, Str))` — executes SQL statement returning modified rows count.
  - `dbQuery : Str, Str -> Effect(Result(Str, Str))` — executes SQL query returning JSON column/row data.
- **Distributed Tracing & Context**:
  - `traceId : Effect(Str)` / `spanId : Effect(Str)` — OpenTelemetry trace and span identifiers.
  - `traceHeaders : Effect(Result(Str, Str))` — W3C traceparent / tracestate headers JSON.
- **Configuration Store**:
  - `configGet : Str -> Effect(Result(Str, Str))` — queries dynamic runtime configuration values via `wasi:config/store`.

---

## Build & Testing Verification

1. **Pre-compile Platform Host**:
   ```bash
   ./tooling/build.sh platform
   ```
   Produces `build/libhost.a` and `build/host.wasm`.

2. **Run Host Unit Tests**:
   ```bash
   cargo test --manifest-path host/Cargo.toml --lib
   ```
   Verifies binary I/O, secret handle resolution, agent lifecycle, and read-only state bypass.

3. **Package Platform Release**:
   ```bash
   ./tooling/build.sh package v0.1.0 thesparq/roc-golem
   ```
   Generates reproducible `.tar.zst`, `.tar.br`, and `.tar.gz` bundles with BLAKE3 hashes in `dist/`.

4. **Build Application Component**:
   ```bash
   ./tooling/build.sh canary_release
   # or
   ./tooling/build.sh app examples/canary_release/main.roc
   ```
