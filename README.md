# Roc Platform for Golem Cloud

A durable, agent-native WebAssembly platform for writing [Golem Cloud](https://golem.cloud) stateful agents, AI tools, and streaming workflows in [Roc](https://www.roc-lang.org) with a pre-compiled **Rust Host** translation layer and the **Wasm Component Model**.

---

## 🌟 Highlights

- **Pure Functional Agents in Roc**: Write durable agents as pure state machines with idiomatic Roc syntax without needing Rust or WIT knowledge.
- **Multi-Agent Bundling & Composition**: Package multiple agents with distinct state models (or stateless ephemeral workers) into a single WebAssembly component via `bundle([Agent1, Agent2])`.
- **Zero-Bottleneck Serialization Bypass**: Read-only invocations (`registerReadOnly`) and ephemeral agents (`defineEphemeral`) bypass state re-serialization completely, achieving $O(1)$ zero-copy in-memory performance across invocations.
- **Typed Method & Cross-Agent RPC**: Phantom-typed `Method(input, output)` handles provide end-to-end compile-time safety for method registration and cross-agent RPC (`rpc`, `rpcSchedule`, `rpcFireAndForget`).
- **Universal Pass-Through Adapter**: The platform translation layer acts as a generic JSON/String bridge, letting your Roc application control strongly typed serialization and domain logic.
- **Pre-Compiled Host Architecture**: The Rust host is pre-compiled into a static library (`platform/targets/wasm32/libroc_golem_host.a` and `platform/libhost.a`), allowing app developers to build and componentize directly.
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
    pf: platform "https://github.com/thesparq/roc-golem/releases/download/v1.2.0/7tvjAkNOXfbP1LmNSFknACmMXKul4-q3geVJkood0r8.tar.zst",
}

import pf.Golem exposing [Agent, agent, reply, readOnly, tool]

State : { count : I64 }

# Unified tool definition (schema + handler bundled together)
getCountTool = tool({
    name: "get_count",
    description: "Returns current counter value",
    params: [],
    execute: |state, _args| Ok("{\"count\":${Str.inspect(state.count)}}"),
})

agent : Agent(State)
agent = Golem.agent({
    name: "counter-agent",
    initial: { count: 0.I64 },
    tools: [getCountTool],
    onMessage: |state, message|
        match message {
            "increment" =>
                reply({ count: state.count + 1.I64 }, "Counter incremented to ${Str.inspect(state.count + 1.I64)}"),

            "decrement" =>
                reply({ count: state.count - 1.I64 }, "Counter decremented to ${Str.inspect(state.count - 1.I64)}"),

            "get" =>
                readOnly(state, "Current count is ${Str.inspect(state.count)}"),

            _ =>
                readOnly(state, "Unknown command '${message}'. Available: increment, decrement, get"),
        },
})
```

> **Developer Experience (DX) Features**:
> - **Unified Tools**: Tool schema (`ToolDefinition`) and execution logic are defined together. Tool ID forwarding, JSON envelope wrapping, and dispatch are automatic.
> - **Expressive Response Primitives**: `reply` (state mutated, oplog commit), `readOnly` ($O(1)$ in-memory response, safe oplog bypass), `stay`, and `fail`.
> - **Default Codecs**: Automatic JSON state serialization and deserialization without repetitive codec boilerplate.
> - **Stateless Workers**: Write stateless workers via `Golem.ephemeral({ name, onMessage, tools })` with `{}` state and zero serialization overhead.
> - **Full Backward Compatibility**: Legacy `defineAgent`, `define`, `defineEphemeral`, and `build` pipeline methods remain fully supported.

For local platform development, you can point to the local path:
```roc
app [agent] { pf: platform "../../platform/main.roc" }
```

### 🤝 Multi-Agent Bundling & Zero-Serialization (v0.7.0+)

Combine multiple stateful or stateless (ephemeral) agents with distinct `State` types into a single WebAssembly component using `bundle2` or `bundleAll`:

```roc
app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [bundleAll, erase]
import ReleaseCoordinator
import DiagnosticsWorker
import AuditWorker

agent = bundleAll([
    erase(ReleaseCoordinator.agent),
    erase(DiagnosticsWorker.agent),
    erase(AuditWorker.agent),
])
```

> [!NOTE]
> **Legacy `defineAgent` Notice**: The backwards-compatible `defineAgent` shim always triggers a full state re-serialization (`stateChanged: True`) on every message. For zero-cost read-only bypass, migrate to `Golem.agent` with `readOnly` or use the typed builder pipeline (`define` + `registerReadOnlyTyped`).

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
│   ├── Effect.roc              # Low-level host effect declarations
│   ├── Json.roc                # JSON serialization and decoding utilities
│   └── targets/
│       └── wasm32/
│           └── libroc_golem_host.a # Pre-compiled host static library for wasm32
├── examples/
│   ├── counter/
│   │   ├── main.roc            # Stateful counter agent example
│   │   └── wit/world.wit       # Component-specific WIT definition
│   ├── ai_tool/
│   │   ├── main.roc            # AI agent with tool declarations example
│   │   └── wit/world.wit       # Component-specific WIT definition
│   ├── streaming_agent/
│   │   ├── main.roc            # WebSocket streaming agent example
│   │   └── wit/world.wit       # Component-specific WIT definition
│   └── canary_release/
│       ├── ReleaseCoordinator.roc  # Stateful canary coordinator with typed methods & read-only bypass
│       ├── DiagnosticsWorker.roc   # Stateless (ephemeral) diagnostics worker
│       ├── main.roc                # Multi-agent component bundle (bundle([Coordinator, Worker]))
│       └── wit/world.wit       # Component-specific WIT definition
├── tooling/
│   ├── build.sh                # Multi-phase build, package, & release script
│   ├── test_consumer.sh        # Consumer package smoke test
│   ├── test_runtime.sh         # Layer 4 live runtime test harness
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
Produces `platform/targets/wasm32/libroc_golem_host.a` and `platform/libhost.a`.

### 2. Build Any Roc Agent
```bash
# Build all agents
./tooling/build.sh all

# Or build single agent
./tooling/build.sh streaming_agent

# Or build from path
./tooling/build.sh app examples/counter/main.roc
```

### 3. Deploy to Golem Cloud
```bash
# Using Golem CLI (v1.6.3+):
# 1. Start local server (if testing locally)
golem server run

# 2. Build and deploy using the application manifest (golem.yaml)
golem app build
golem app deploy

# Or deploy manually via REST API / curl:
curl -F "name=streaming" -F "component=@build/streaming_agent_agent.wasm" http://127.0.0.1:9881/v1/components

# 3. Create a worker and invoke
golem worker invoke "streaming/stream-worker-1" \
  'golem:agent/guest@2.0.0.{initialize}' \
  '"streaming-agent"' 'tuple([unstructured-text(inline({data: "{}"}))])' 'anonymous'
```

---

## 🚀 Creating Platform Releases

### Automatic GitHub Release (Recommended)
1. Push a version tag to Git:
   ```bash
   git tag v1.2.0
   git push origin v1.2.0
   ```
2. GitHub Actions automatically builds the host, validates all agents, packages the `.tar.zst`, `.tar.br`, `.tar.gz` bundles with BLAKE3 hashes, and creates the GitHub release with ready-to-copy Roc code snippets!

### Local Release Packaging
You can generate release tarballs locally at any time:
```bash
./tooling/build.sh package v1.2.0 thesparq/roc-golem
```
Artifacts and `release_notes.md` will be placed in `dist/`.

---

## 🤝 Contributing

Contributions are welcome!
- **Code Formatting**: Formatting is verified in CI using `roc fmt --check` and `cargo fmt --check`. Developers should format Roc source files manually or via editor integration (`roc fmt <path/to/file.roc>`). The build scripts do not automatically mutate source files.
- **Testing**: Run `./tooling/test_consumer.sh` and `cargo test --manifest-path host/Cargo.toml --features stub-guest --lib` prior to opening pull requests.

---

## 📄 License
This project is licensed under the Apache 2.0 License - see the [LICENSE](LICENSE) file for details.
