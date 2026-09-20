# Roc Platform for Golem Cloud

A durable, agent-native WebAssembly platform for writing [Golem Cloud](https://golem.cloud) stateful agents, AI tools, and streaming workflows in [Roc](https://www.roc-lang.org) with a pre-compiled **Rust Host** translation layer and the **Wasm Component Model**.

---

## 🌟 Highlights

- **Pure Functional Agents in Roc**: Write durable agents as pure state machines with idiomatic Roc syntax without needing Rust or WIT knowledge.
- **Universal Pass-Through Adapter**: The platform translation layer acts as a generic JSON/String bridge, letting your Roc application control strongly typed serialization and domain logic.
- **Pre-Compiled Host Architecture**: The Rust host is pre-compiled into a static library (`libhost.a` / `host.wasm`), allowing app developers to build and componentize directly.
- **WebSocket & Streaming Support**: Built-in Golem WebSocket Client API (`websocketConnect`, `websocketSend`, `websocketReceive`, `websocketClose`) for bi-directional live streaming.
- **Durable Scheduling & Timers**: Durable execution-aware sleep (`sleepMillis`) and monotonic clock access (`nowMillis`).
- **Worker-to-Worker RPC & HTTP**: Call other Golem workers or external APIs directly from Roc effect functions.
- **Release-Ready Distribution**: Package platforms into `.tar.zst`, `.tar.br`, or `.tar.gz` with BLAKE3 base64url checksums for instant copy-paste consumption in any Roc project.

---

## 📦 Using in your Roc Code

You can use the official pre-packaged platform release in your Roc application by specifying the release URL in your `app` header:

```roc
app [agent] {
    pf: platform "https://github.com/thesparq/roc-golem/releases/download/v0.5.3/eeMpVwjUcEouRIA_ZvHSP4dvTO-GaNheHL1lUf91YLs.tar.zst",
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
