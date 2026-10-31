# Roc Platform for Golem Cloud

A durable, agent-native WebAssembly platform for writing [Golem Cloud](https://golem.cloud) stateful agents, AI tools, and streaming workflows in [Roc](https://www.roc-lang.org) with a pre-compiled **Rust Host** translation layer and the **Wasm Component Model**.

---

## 🌟 Highlights

- **Pure Functional Agents in Roc**: Write durable agents as pure state machines with idiomatic Roc syntax without needing Rust or WIT knowledge.
- **Multi-Agent Bundling & Composition**: Package multiple agents with distinct state models (or stateless ephemeral workers) into a single WebAssembly component via `bundle([Agent1, Agent2])`.
- **Zero-Bottleneck Serialization Bypass**: Read-only invocations (`registerReadOnly`) and ephemeral agents (`defineEphemeral`) bypass state re-serialization completely, achieving $O(1)$ zero-copy in-memory performance across invocations.
- **Typed Method & Cross-Agent RPC**: Phantom-typed `Method(input, output)` handles provide end-to-end compile-time safety for method registration and cross-agent RPC (`rpc`, `rpcSchedule`, `rpcFireAndForget`).
- **Universal Pass-Through Adapter**: The platform translation layer acts as a generic JSON/String bridge, letting your Roc application control strongly typed serialization and domain logic.
- **Pre-Compiled Host Architecture**: The Rust host is pre-compiled into a static library (`libhost.a` / `host.wasm`), allowing app developers to build and componentize directly.
- **WebSocket & Streaming Support**: Built-in Golem WebSocket Client API (`websocketConnect`, `websocketSend`, `websocketReceive`, `websocketClose`) for bi-directional live streaming.
- **Raw Binary I/O & Clean Boundary**: Full binary support (`List(U8)`) for filesystem, blobstore, key-value storage, and HTTP without lossy UTF-8 conversions (`readFileBytes`, `writeFileBytes`, `blobGetBytes`, `blobPutBytes`, `kvGetBytes`, `kvSetBytes`, `httpRequestBytes`).
- **Opaque Secret Handles & Host Injection**: Keep tokens and API keys out of guest memory with `SecretHandle` (`U32`) and host-mediated header injection (`SecretHeader`), plus opt-in `revealSecret`.
- **Durable Scheduling & Timers**: Durable execution-aware sleep (`sleepMillis`) and monotonic clock access (`nowMillis`).
- **Worker-to-Worker RPC & HTTP**: Call other Golem workers or external APIs directly from Roc effect functions.
- **Release-Ready Distribution**: Package platforms into `.tar.zst`, `.tar.br`, or `.tar.gz` with BLAKE3 base64url checksums for instant copy-paste consumption in any Roc project.

---

## 📦 Using in your Roc Code

You can use the official pre-packaged platform release in your Roc application by specifying the release URL in your `app` header:

```roc
app [agent] {
    pf: platform "https://github.com/thesparq/roc-golem/releases/download/v0.8.0/Nx4rJNk_kHTlxYCixZ-v5z0yY3mllisQFKI1mc1Vpp0.tar.zst",
}

import pf.Golem exposing [Agent, defineAgent]

State : { count : I64 }

agent : Agent(State)
agent = defineAgent({
    init: |_config| Ok({ count: 0 }),
    handleMessage: |state, msg|
        match msg {
            "increment" => Ok({ state: { count: state.count + 1 }, response: "Incremented" }),
            _ => Ok({ state, response: "Count is ${Num.to_str(state.count)}" }),
        },
    handleToolCall: |state, call|
        Ok({
            state,
            result: { id: call.id, success: Bool.true, output: "{\"count\":${Num.to_str(state.count)}}" },
        }),
    metadata: {
        name: "counter-agent",
        version: "1.0.0",
        description: "Durable Roc Counter Agent on Golem Cloud",
        tools: [],
    },
    serializeState: |state| "{\"count\":${Num.to_str(state.count)}}",
    deserializeState: |_json| Ok({ count: 0 }),
})
```

For local platform development, you can point to the local path:
```roc
app [agent] { pf: platform "../../platform/main.roc" }
```

### 🤝 Multi-Agent Bundling & Zero-Serialization (v0.7.0+)

Combine multiple stateful or stateless (ephemeral) agents with distinct `State` types into a single WebAssembly component using `bundle`:

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

- **Stateless Ephemeral Workers**: Define workers with `defineEphemeral({ ... }) |> registerEphemeral(method, handler) |> build` — no dummy `State` threading, and zero state persistence overhead.
- **Zero-Bottleneck Read-Only Calls**: Handlers registered via `registerReadOnly` emit `stateChanged: False` and an empty state update string, instructing the Golem host to preserve in-memory state in $O(1)$ time without re-serializing.
- **Stateless Tool Handlers**: Tools that don't need to read or mutate agent state can be registered with `withStatelessToolHandler(handleTool)` without threading dummy state.
- **Typed Methods & RPC**: Use phantom-typed `Method(input, output)` handles for compile-time safety across agent registrations and cross-worker RPC (`rpc`, `rpcSchedule`, `rpcFireAndForget`, or typed codecs `rpcTyped`, `rpcScheduleTyped`, `rpcFireAndForgetTyped`).

### 🔒 Opaque Secret Handles & Host-Mediated Injection

To prevent API tokens and sensitive credentials from leaking into guest memory or state snapshots:
- Acquire an opaque handle: `handle = getSecretHandle("OPENAI_API_KEY")` (returns `SecretHandle : U32`).
- Attach handles to outbound HTTP requests using `SecretHeader`:
  ```roc
  headers = [
      { name: "Authorization", prefix: "Bearer ", handle: apiKeyHandle }
  ]
  ```
- The Golem Rust host resolves the handle in host linear memory and injects the header directly into the outgoing WASI HTTP request.
- Opt-in plaintext access is available via `revealSecret(handle)` when cryptographic signing or local parsing is required.

### 💾 Raw Binary Data Boundary (`List(U8)`)

Lossy UTF-8 conversions replace arbitrary non-UTF-8 bytes with `U+FFFD`, corrupting binary data like images, archives, protobufs, or compiled WebAssembly. `roc-golem` provides dedicated byte-level I/O primitives:
- **Filesystem**: `readFileBytes(path)`, `writeFileBytes(path, bytes)`, `appendFileBytes(path, bytes)`
- **Key-Value Store**: `kvGetBytes(bucket, key)`, `kvSetBytes(bucket, key, bytes)` — backed by live `wasi:keyvalue/eventual@0.1.0`
- **Blobstore**: `blobGetBytes(container, object)`, `blobPutBytes(container, object, bytes)` — backed by live `wasi:blobstore/blobstore`
- **HTTP**: `httpRequestBytes(reqJson, bodyBytes)` — backed by live `wasi:http/outgoing-handler@0.2.3` with host-mediated secret header injection

### ⚡ Golem 1.6 Primitives (Promises, Webhooks, Idempotency, WebSockets & Oplog)

- **Durable Promises & External Webhooks**:
  - `createPromise : Effect(Result(U64, Str))` — allocate a durable promise handle.
  - `createWebhook : U64 -> Effect(Result(Str, Str))` — generate a public webhook URL routed to resume the promise.
  - `completePromise : U64, List(U8) -> Effect(Result(Bool, Str))` — fulfill a promise with payload bytes.
  - `getPromise : U64 -> Effect(Result(List(U8), Str))` — await and retrieve promise fulfillment data.
- **Idempotency & Oplog Controls**:
  - `generateIdempotencyKey : Effect(Result(Str, Str))` — acquire a unique Golem Cloud idempotency UUID.
  - `getOplogIndex : Effect(U64)` — retrieve current worker operation log index.
  - `oplogCommit : U8 -> Effect({})` — persist and commit oplog up to current index with specified replica quorum.
  - `getIdempotenceMode : Effect(Bool)` / `setIdempotenceMode : Bool -> Effect({})` — inspect and toggle idempotency replay protection.
  - `revertAgent : U64 -> Effect(Result({}, Str))` — roll back agent execution state to prior oplog index.
- **WebSockets & Real-Time Streaming**:
  - `websocketConnect : Str -> Effect(Result(U32, Str))` — connect to a WebSocket endpoint.
  - `websocketSend : U32, Str -> Effect(Result({}, Str))` — transmit text frame.
  - `websocketReceive : U32 -> Effect(Result(Str, Str))` — receive next frame from connection.
  - `websocketClose : U32 -> Effect(Result({}, Str))` — cleanly terminate WebSocket connection.
- **Durable RDBMS (PostgreSQL & MySQL)**:
  - `dbExecute : Str, Str -> Effect(Result(U64, Str))` — execute SQL statement on Postgres (`postgresql://...`) or MySQL (`mysql://...`) returning modified rows count.
  - `dbQuery : Str, Str -> Effect(Result(Str, Str))` — execute SQL query returning JSON column/row payload.
- **Distributed Tracing & W3C Context**:
  - `traceId : Effect(Str)` / `spanId : Effect(Str)` — retrieve OpenTelemetry hexadecimal trace & span IDs.
  - `traceHeaders : Effect(Result(Str, Str))` — retrieve W3C traceparent / tracestate headers JSON.
- **Configuration Store**:
  - `configGet : Str -> Effect(Result(Str, Str))` — query dynamic runtime configurations via `wasi:config/store`.

---

## 📁 Repository Structure

```
roc-golem/
├── .github/
│   └── workflows/
│       ├── ci.yaml             # CI test & validation workflow
│       └── release.yaml        # Automated release builder & publisher
├── wit/
│   └── world.wit               # Golem Component Model WIT interface definitions
├── host/
│   ├── Cargo.toml              # Rust host crate (wit-bindgen, serde)
│   └── src/
│       ├── lib.rs              # Universal WIT export/import & C-ABI translation layer
│       ├── roc_std.rs          # Wasm32 Roc ABI types (RocStr, RocList, Allocators)
│       └── guest_bridge.rs     # Guest dispatch & fallback bridge
├── platform/
│   ├── main.roc                # Roc Platform definition & host entry points
│   ├── Golem.roc               # Idiomatic Roc Golem SDK (Agent, WebSocket, Timers, RPC, HTTP)
│   ├── Types.roc               # Core data types (Metadata, ToolDefinition, ToolCall)
│   └── Effect.roc              # Low-level host effect declarations
├── examples/
│   ├── counter/
│   │   └── main.roc            # Stateful counter agent example
│   ├── ai_tool/
│   │   └── main.roc            # AI agent with tool declarations example
│   ├── streaming_agent/
│   │   └── main.roc            # WebSocket streaming agent example
│   └── canary_release/
│       ├── ReleaseCoordinator.roc  # Stateful canary coordinator with typed methods & read-only bypass
│       ├── DiagnosticsWorker.roc   # Stateless (ephemeral) diagnostics worker
│       └── main.roc                # Multi-agent component bundle (bundle([Coordinator, Worker]))
├── tooling/
│   ├── build.sh                # Multi-phase build, package, & release script
│   └── pack_platform/          # Platform tarball & BLAKE3 base64url packaging tool
├── LICENSE                     # Apache 2.0 License
└── golem.yaml                  # Golem Cloud application manifest
```

---

## 🔨 Build & Deployment Workflow

### 1. Pre-compile Platform Host (Once)
```bash
./tooling/build.sh platform
```
Produces `build/libhost.a` and `build/host.wasm`.

### 2. Build Any Roc Agent
```bash
# Build streaming agent
./tooling/build.sh streaming_agent

# Or build from path
./tooling/build.sh app examples/counter/main.roc
```

### 3. Deploy to Golem Cloud
```bash
golem component add --component-name streaming-agent build/streaming_agent_agent.wasm
golem worker add --component-name streaming-agent --worker-name stream-worker-1
golem worker invoke-and-await --component-name streaming-agent --worker-name stream-worker-1 \
  --function "golem:agent-platform/agent-api.{handle-message}" \
  --args '["connect"]'
```

---

## 🚀 Creating Platform Releases

### Automatic GitHub Release (Recommended)
1. Push a version tag to Git:
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```
2. GitHub Actions automatically builds the host, validates all agents, packages the `.tar.zst`, `.tar.br`, `.tar.gz` bundles with BLAKE3 hashes, and creates the GitHub release with ready-to-copy Roc code snippets!

### Local Release Packaging
You can generate release tarballs locally at any time:
```bash
./tooling/build.sh package v0.1.0 thesparq/roc-golem
```
Artifacts and `release_notes.md` will be placed in `dist/`.

---

## 📄 License
This project is licensed under the Apache 2.0 License - see the [LICENSE](LICENSE) file for details.
