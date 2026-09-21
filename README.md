# Roc Platform for Golem Cloud

A durable, agent-native WebAssembly platform for writing [Golem Cloud](https://golem.cloud) stateful agents, AI tools, and streaming workflows in [Roc](https://www.roc-lang.org) with a pre-compiled **Rust Host** translation layer and the **Wasm Component Model**.

---

## 🌟 Highlights

- **Pure Functional Agents in Roc**: Write durable agents as pure state machines with idiomatic Roc syntax without needing Rust or WIT knowledge.
- **Universal Pass-Through Adapter**: The platform translation layer acts as a generic JSON/String bridge, letting your Roc application control strongly typed serialization and domain logic.
- **Pre-Compiled Host Architecture**: The Rust host is pre-compiled into a static library (`libhost.a`), allowing app developers to compile and link their Roc apps directly.
- **Native Golem 1.5 Agent Protocol**: Fully compatible with `golem:agent/guest@1.5.0` (`discover-agent-types`, `initialize`, `invoke`, `get-definition`).
- **Serverless HTTP Endpoints**: Direct native HTTP endpoints configured in agent definitions (e.g. `/api/chat`).
- **WebSocket & Streaming Support**: Built-in Golem WebSocket Client API (`websocketConnect`, `websocketSend`, `websocketReceive`, `websocketClose`) for bi-directional live streaming.
- **Durable Scheduling & Timers**: Durable execution-aware sleep (`sleepMillis`) and monotonic clock access (`nowMillis`).
- **Worker-to-Worker RPC & HTTP**: Call other Golem workers or external APIs directly from Roc effect functions.
- **Release-Ready Distribution**: Package platforms into `.tar.zst`, `.tar.br`, or `.tar.gz` with BLAKE3 base64url checksums for instant copy-paste consumption in any Roc project.

---

## 📦 Using in your Roc Code

You can use the official pre-packaged platform release in your Roc application by specifying the release URL in your `app` header:

```roc
app [agent] {
    pf: platform "https://github.com/thesparq/roc-golem/releases/download/v0.5.7/Xpml5OkByUZVwj7YdxPP-hJ95TITqh8yttH3ojtzm-o.tar.zst",
}

import pf.Golem exposing [Agent, defineAgent]

State :: { count : I64 }

agent : Agent(State)
agent = defineAgent({
    init!: |_config|
        { count: 0 },
    handleMessage!: |state, msg|
        match msg {
            "increment" => {
                nextCount = state.count + 1
                { state: { count: nextCount }, replies: ["Counter incremented to ${I64.to_str(nextCount)}"] }
            }
            _ =>
                { state, replies: ["Current count is ${I64.to_str(state.count)}"] },
        },
    handleToolCall!: |state, call|
        match call.name {
            "get_count" =>
                {
                    state,
                    result: { id: call.id, success: True, output: "{\"count\":${I64.to_str(state.count)}}" },
                }
            _ =>
                {
                    state,
                    result: { id: call.id, success: False, output: "Unknown tool ${call.name}" },
                }
        },
    metadata: {
        name: "counter-agent",
        version: "1.0.0",
        description: "Durable Roc Counter Agent on Golem Cloud",
        tools: [
            {
                name: "get_count",
                description: "Returns current counter value",
                parameters: [],
            },
        ],
    },
    serializeState: |state| "{\"count\":${I64.to_str(state.count)}}",
    deserializeState: |stateJson|
        match Str.split_first(stateJson, "\"count\":") {
            Ok({ before: _, after }) =>
                match Str.split_first(after, "}") {
                    Ok({ before, after: _ }) =>
                        match I64.from_str(Str.trim(before)) {
                            Ok(c) => Ok({ count: c })
                            Err(_) => Err("Failed to parse count")
                        }
                    Err(_) => Err("Invalid JSON")
                }
            Err(_) => Err("Missing count field")
        },
})
```

For local platform development, you can point to the local path:
```roc
app [agent] { pf: platform "../../platform/main.roc" }
```

---

## 📁 Repository Structure

```
roc-golem/
├── .github/
│   └── workflows/
│       ├── ci.yaml             # CI test & validation workflow
│       └── release.yaml        # Automated release builder & publisher
├── wit/
│   └── world.wit               # Golem Component Model WIT interface definitions (golem:agent 1.5.0)
├── host/
│   ├── Cargo.toml              # Rust host crate (wit-bindgen, serde)
│   └── src/
│       ├── lib.rs              # Universal WIT export/import & C-ABI translation layer
│       ├── roc_std.rs          # Wasm32 Roc ABI types (RocStr, RocList, Allocators)
│       └── guest_bridge.rs     # Test stubs for unit tests
├── platform/
│   ├── main.roc                # Roc Platform definition & host entry points
│   ├── Golem.roc               # Idiomatic Roc Golem SDK (Agent, WebSocket, Timers, RPC, HTTP)
│   ├── Types.roc               # Core data types (Metadata, ToolDefinition, ToolCall)
│   ├── Effect.roc              # Low-level host effect declarations
│   └── Host.roc                # Low-level C-ABI effect signatures
├── examples/
│   ├── counter/
│   │   └── main.roc            # Stateful counter agent example
│   ├── ai_tool/
│   │   └── main.roc            # AI agent with tool declarations example
│   └── streaming_agent/
│       └── main.roc            # WebSocket streaming agent example
├── tooling/
│   ├── build.sh                # Multi-phase build, package, & release script
│   └── pack_platform/          # Platform tarball & BLAKE3 base64url packaging tool
├── LICENSE                     # Apache 2.0 License
└── golem.yaml                  # Golem Cloud application manifest
```

---

## 🔨 Build & Deployment Workflow

### 1. Pre-compile Platform Host
```bash
./tooling/build.sh platform
```
Produces `platform/targets/wasm32/libhost.a` (and copies to `build/libhost.a`).

### 2. Build Any Roc Agent
```bash
# Build counter agent
./tooling/build.sh counter

# Build streaming agent
./tooling/build.sh streaming_agent

# Or build from file path
./tooling/build.sh app examples/counter/main.roc
```

### 3. Deploy to Golem Cloud
```bash
golem component add --component-name counter build/counter_agent.wasm
golem worker add --component-name counter --worker-name counter-1
golem worker invoke-and-await --component-name counter --worker-name counter-1 \
  --function "golem:agent/guest@1.5.0.{handle-message}" \
  --args '["increment"]'
```

---

## 🚀 Creating Platform Releases

### Automatic GitHub Release (Recommended)
1. Push a version tag to Git:
   ```bash
   git tag v0.5.4
   git push origin v0.5.4
   ```
2. GitHub Actions automatically builds the host, compiles and validates all agents, packages the `.tar.zst`, `.tar.br`, `.tar.gz` bundles with BLAKE3 hashes, and creates the GitHub release with ready-to-copy Roc code snippets!

### Local Release Packaging
You can generate release tarballs locally at any time:
```bash
./tooling/build.sh package v0.5.4 thesparq/roc-golem
```
Artifacts and `release_notes.md` will be placed in `dist/`.

---

## 📄 License
This project is licensed under the Apache 2.0 License - see the [LICENSE](LICENSE) file for details.
