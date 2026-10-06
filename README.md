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

Use a published platform release in your Roc application by specifying its URL in the `app` header. The exact URL (with the content hash as the file name) is printed by the release workflow and written to `dist/release_notes.md` when packaging:

```roc
app [agent] {
    pf: platform "https://github.com/thesparq/roc-golem/releases/download/v0.6.0/<hash>.tar.zst",
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
│   ├── streaming_agent/
│   │   └── main.roc            # WebSocket streaming agent example
│   └── effects/
│       └── main.roc            # HTTP + agent-to-agent RPC example
├── tooling/
│   ├── build.sh                # Multi-phase build, package, & release script
│   ├── pack_platform/          # Platform tarball & BLAKE3 base64url packaging tool
│   ├── abi_harness/            # Runs built components outside Golem to check the Roc ABI
│   └── glue/                   # `roc glue` spec that prints the host-facing ABI surface
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

`golem.yaml` declares the example components, so deployment is manifest-driven:

```bash
# Build every component in golem.yaml and deploy it to the local environment
golem -L deploy

# Invoke an agent. Agent ids are `<agent-type>(<constructor params>)`, and
# arguments are component-model values, so strings are plain literals.
golem -L agent invoke 'counter-agent("{}")' handle-message '"increment"'
# Invocation result in fallback TypeScript syntax:
#   "Counter incremented to 1"
```

Agent type names come from your app's `metadata.name` (`golem agent-type list`
shows them). Re-running `golem deploy` leaves agents that already exist on their
original component revision; pass `-u automatic` to move idle agents to the new
revision. Golem refuses that update if an agent's recorded results no longer
type-check, which is what happens when a method's result schema changes.

#### Which CLI to use

| golem CLI | Local deploy | Notes |
| --- | --- | --- |
| 1.5.9 | works | verified end-to-end with these examples (deploy, invoke, state, tools) |
| 1.5.10 | **broken** | its own local server rejects the CLI's component upload (`parse multipart error: failed to parse field componentWasm ... No such file or directory`), for any app |
| 1.6.0-rc1+ | needs the 2.0 agent protocol | staging works, but the server expects `golem:agent/guest@2.0.0`; this platform implements `@1.5.0` |

> [!NOTE]
> The older `golem component add` / `golem worker add` commands no longer exist in
> golem-cli 1.5.x. Run `golem agent invoke --help` for the full agent-id syntax.

---

## ⚠️ Development status

The components built from this tree work on Golem again: `get-definition` /
`discover-agent-types` report the app's real agent type and tools, `initialize`
succeeds, and `handle-message` round-trips state (all three examples are covered
by the `tooling/abi_harness` smoke test in CI).

Three defects were fixed to get there, all in the hand-written Roc ABI in
`host/src/roc_std.rs`:

- `cabi_realloc` handed out pointers carrying the Roc allocation header, but
  wit-bindgen's glue drops those buffers through Rust's global allocator, so
  `initialize` aborted inside dlmalloc while dropping its incoming `DataValue`
  and poisoned the instance. It is now a plain allocation.
- A heap `Str` carries its length in word 3, not word 2, so every string longer
  than the 11-byte small-string limit came back garbled — which is why the agent
  metadata failed to parse and every component reported the fallback `roc-agent`
  type (`Wrapper agent type name roc-agent is defined by multiple components`).
- `Try` uses `1` for `Ok`, not `0`, so successful calls surfaced as errors.

The layout values now match the compiler's own emitted facts, dumped by
`tooling/glue/abi_facts.roc`.

Still open:

- **Cross-component RPC** resolves the target agent id through
  `golem:agent/host`, but the server answers `Agent type not found` because the
  platform does not emit `dependencies` in the agent type; declaring the target
  agent type as a dependency is the missing piece.
- **A layout-sensitive trap** has been seen once in a release build of the
  effects example: `initialize` trapped while slicing a host-provided heap string
  (`Str.split_first` on the agent config). The same source passes when rebuilt,
  and a smaller reproduction did not trigger it, so it looks like a Roc
  code-generation issue rather than a platform one. CI's ABI smoke test is the
  safety net; if it reappears, keep the failing `.wasm` and compare `roc build`
  with and without `--debug`.
- **Local deploy** needs golem CLI 1.5.9: 1.5.10's component upload is rejected
  by its own local server, and 1.6.0-rc1+ expects the `golem:agent/guest@2.0.0`
  protocol (see the deploy section for the version matrix).
- `host/src/roc_std.rs` is still hand-written; generated glue (`roc glue`) would
  retire the layout constants — see `tooling/glue/`.

## ✨ Effects

Every effect is declared in `platform/Effect.roc` and wrapped by `platform/Golem.roc`;
apps call them through `pf.Golem`.

| Effect | Backing host function | Notes |
| --- | --- | --- |
| `logTrace!` … `logError!` | `wasi:logging` | context is set to `roc-agent` |
| `workerId!` | `golem:api/host` | current agent id |
| `setPersistence!` | `golem:api/host` | `PersistNothing` / `PersistStateOnly` / `PersistEverything` |
| `now!` | `wasi:clocks/monotonic-clock` | monotonic milliseconds |
| `sleep!` | `wasi:clocks/monotonic-clock` | blocking wait; Golem records it in the oplog, so a resumed agent does not repeat it |
| `http!` | `wasi:http/outgoing-handler` | request/response as JSON; response is `{status, headers, body}` |
| `rpc!` | `golem:agent/host` | target is an agent id such as `counter-agent("{}")`; returns the reply text || `websocketConnect!` / `websocketSend!` / `websocketReceive!` / `websocketClose!` | `golem:websocket/client` | connections are tracked by a `U32` handle returned from `websocketConnect!` |

The linker only keeps the effects an app actually calls, so a component's import
list stays as small as the agent's behaviour.

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
