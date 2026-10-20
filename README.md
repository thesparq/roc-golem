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

> [!IMPORTANT]
> The platform is pinned to one Roc nightly — **`nightly-2026-09-19-d025939`**,
> the version CI and the release workflow install. The hand-written Roc ABI
> (`host/src/roc_std.rs`) encodes that compiler's `Str`/`List`/`Try` layout
> facts, so another nightly can produce components that *build* and then trap at
> runtime. `tooling/build.sh` refuses to build with a different `roc` on `PATH`
> (`ROC_ALLOW_VERSION_MISMATCH=1` overrides it for experiments); moving the pin
> means re-deriving the layout facts with `tooling/glue/abi_facts.roc` first. The
> runtime's *memory protocol* (refcount and slice conventions) is not part of
> those facts — it is taken from the compiler's builtins and mirrored in
> `roc_std.rs`, see `tooling/glue/README.md`.

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

This branch targets the **Golem 1.6 agent protocol** (`golem:agent/guest@2.0.0`).
Components built from it do not run on 1.5.x servers; use the `v0.5.x` line for
those.

| golem CLI | Local deploy | Notes |
| --- | --- | --- |
| 1.6.0 (GA) | works | verified end-to-end on this branch: deploy, invoke, state, tools, `http!` over WASI p3, HTTP mounts, snapshots |
| 1.6.0-rc2.1 | works | earlier pin; the same examples ran, with `http!` still on p2 HTTP (rc2.1 registered it) |
| 1.5.9 / 1.5.10 | not compatible | those servers expect `golem:agent/guest@1.5.0`, which this branch no longer exports |

> [!NOTE]
> The 1.6.0 release splits the download into two binaries: `golem-<target>`
> (the all-in-one server + CLI, which is what `tooling/e2e.sh` and CI use, and
> which owns `golem server run`) and `golem-cli-<target>` (client only, smaller,
> no `server` subcommand).

> [!NOTE]
> The older `golem component add` / `golem worker add` commands no longer exist in
> golem-cli 1.5.x. Run `golem agent invoke --help` for the full agent-id syntax.

---

## ⚠️ Development status

**On the 1.6 line** (this branch): the host speaks `golem:agent/guest@2.0.0`
with async exports, `schema-value-tree` values and a `schema-graph` agent type;
the examples deploy and run on a local `1.6.0` server (the e2e job in CI does
exactly that), and a **snapshot-based agent update** is verified: the
component exports `golem:api/save-snapshot`/`load-snapshot` backed by the state
JSON, so `golem agent update <id> manual` carries state across incompatible
revisions (the CLI needs `-Y`; it prompts for confirmation).

**HTTP mounts** are implemented from the app's `metadata.httpMount`: a
`filesystemBindings` mount publishes files from the agent's own durable
filesystem on the deployment's domain, and the runtime answers matching
`GET`/`HEAD` requests without invoking the agent (the `site` example is the
worked case — see [HTTP mounts](#-http-mounts)). Golem requires every
constructor parameter to be captured by the mount path, so the capture names
the agent's identity parameter; the host refuses a mount it cannot bind instead
of letting the deploy fail later.

The component also exports **`golem:tool/guest`**: the app's declared tools are
published under CLI-shaped names (`get_count` → `get-count`) and appear in
`golem tool list`; `golem tool invoke --agent <id> <tool>` routes to the app's
`handleToolCall!`. Two caveats: every tool must be declared in the manifest
(`tools:` plus an `agents:` binding — see `golem.yaml`), and tool invocations run
in a fresh instance without the agent's state, so stateless tools (the AI tool
example's `calculator`) work while stateful ones (`get-count`) fail with an
honest error — use the agent method surface for those.

**Config declarations** are supported: apps list keys in their metadata
(`config: [{ name, secret }]`), which become the agent type's config
declarations, and read them at runtime with `getConfig!("key")` — values come
from the manifest's per-agent `config:` section (or the secret store for
`secret: true`). The counter example reads its `greeting` this way.

Two toolchain notes live in `tooling/abi_harness/README.md` (`Str.trim` trips the
layout-sensitive codegen trap; components that import async host functions
cannot be exercised in the harness yet). `setPersistence!` is a no-op because 1.6
removed persistence levels, and `websocketReceive!` drives the now-async receive
through `wit_bindgen::block_on`.

The components built from this tree work on Golem again: `get-definition` /
`discover-agent-types` report the app's real agent type and tools, `initialize`
succeeds, and `handle-message` round-trips state (all four examples are covered
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

- **Cross-component RPC** works only inside a component: Golem resolves the
  target agent type from the calling component's own agent types, so
  `rpc!("effects-agent(\"other\")", …)` succeeds while
  `rpc!("counter-agent(\"{}\")", …)` from another component answers
  `Agent type not found`. Declaring the target in the agent type's
  `dependencies` did not change that on golem 1.5.9 (the server does not seem to
  use component-declared dependencies), so cross-component calls need a newer
  Golem or a different mechanism.
- **A layout-sensitive trap** has been seen once in a release build of the
  effects example: `initialize` trapped while slicing a host-provided heap string
  (`Str.split_first` on the agent config). The same source passes when rebuilt,
  and a smaller re[REDACTED] did not trigger it. The `RocStr` release path has
  since been rewritten against the compiler's own protocol (see the build section
  and `tooling/glue/README.md`), which removed the known way this class of trap
  could fire; if one reappears, keep the failing `.wasm` and compare `roc build`
  with and without `--debug`.
- **The ABI harness cannot drive async imports**: a component that imports an
  async host function traps inside the harness's wasmtime (a fork-vs-wasmtime
  async-ABI gap — see `tooling/abi_harness/README.md`). That is now the
  streaming agent (`websocket/client.receive`) and the effects agent
  (`wasi:http/client.send`), so both are covered by the e2e instead. The same
  components run on a real Golem server.
- **Local deploy** is verified with golem CLI 1.6.0 (GA), 1.6.0-rc2.1 and 1.5.9
  on the `v0.5.x` line; the version matrix is in the deploy section above.
- `host/src/roc_std.rs` is still hand-written; generated glue (`roc glue`) would
  retire the layout constants — see `tooling/glue/`.

## 🌐 HTTP mounts

An app can publish its agent type on its deployment's domain by declaring
`metadata.httpMount`. `platform/Types.roc` carries the shape:

```roc
httpMount: Mount({
        pathPrefix: ["site", "{id}"],
        filesystemBindings: [
                Subtree({ publicPrefix: ["assets"], filesystemRoot: "/assets" }),
                Exact({ publicPath: ["index.html"], filePath: "/site/index.html" }),
        ],
        fileResponseHeaders: [{ name: "referrer-policy", value: "no-referrer" }],
        corsAllowedPatterns: ["*"],
}),
```

- `pathPrefix` segments are URL literals; a `{name}` segment captures the
  agent's constructor parameter, so the URL selects the instance. Golem
  requires every constructor parameter to be captured exactly once — a mount
  that captures zero or several values is rejected at `initialize` with an
error naming the problem, rather than at deploy time.
- `filesystemBindings` map public paths onto files in the agent's **durable
  filesystem**. The runtime serves matching `GET`/`HEAD` requests itself: no
  method is invoked, and the agent does not have to be running.
  `Subtree` serves everything under a public prefix from a filesystem root;
  `Exact` serves one public path from one file. Anything unmapped answers 404.
- The files come from the manifest's per-agent `files` (initial file system),
  which `golem deploy` uploads and Golem provisions when the agent starts:

  ```yaml
  agents:
    file-site-agent:
      files:
        - sourcePath: examples/site/static/hello.txt
          targetPath: /assets/hello.txt
  ```

- The mount is reachable only if the manifest declares an HTTP API deployment
  for it, and requests are routed by `Host`:

  ```yaml
  localServer:
    customRequestPort: 9882
  httpApi:
    deployments:
      local:
        - subdomain: roc-golem
          agents:
            file-site-agent: {}
  ```

  ```shell
  curl -H 'Host: roc-golem.localhost:9882' \
    http://127.0.0.1:9882/site/site1/assets/hello.txt
  ```

  `subdomain` expands to `<subdomain>.localhost:<customRequestPort>` for local
  servers, so the running server has to bind the port the manifest names (the
  CLI warns when `--custom-request-port` disagrees).

Not implemented yet, and rejected by the server if declared today: `staticBindings`
and an `openapiProviderMethod`, both of which Golem only accepts on an
`http-router` agent type — that kind needs a `handle(request) -> response`
method whose request/response bodies are `stream<u8>` resources, which this
platform does not emit yet.

The `site` example is the worked case: `examples/site/main.roc`,
`examples/site/static/`, and the `site:` checks in `tooling/e2e.sh` (subtree and
exact bindings, 200/404, the configured response header, and that an id that was
never created has no files to serve).

---

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
| `http!` | `wasi:http/client` (p3) | request/response as JSON; response is `{status, headers, body}`. Buffered: the body is uploaded alongside the request through a p3 stream, and the response body is collected before returning |
| `rpc!` | `golem:agent/host` | target is an agent id such as `counter-agent("{}")`; returns the reply text || `websocketConnect!` / `websocketSend!` / `websocketReceive!` / `websocketClose!` | `golem:websocket/client` | connections are tracked by a `U32` handle returned from `websocketConnect!` |

The linker only keeps the effects an app actually calls, so a component's import
list stays as small as the agent's behaviour.

---

## 🧪 Testing

Three layers, all of which run in CI and in the release workflow before
anything is published:

| Layer | Command | What it proves |
| --- | --- | --- |
| Host unit tests | `cargo test --manifest-path host/Cargo.toml --lib` | the Rust side of the ABI: `RocStr`/`Result` layouts, allocator round-trips, URL and method parsing, agent-type discovery against a stubbed guest, and the `httpMount` metadata mapping |
| ABI smoke test | `tooling/abi_harness` (see its README for the flags) | a real component instantiated outside Golem: agent metadata (including the mount path), `initialize`, message round-trips with state, tool calls, and that the effects an app uses reach the host — the host side of those is stubbed and recorded |
| End-to-end | `GOLEM_CLI=/path/to/golem ./tooling/e2e.sh` | all of the above plus a real Golem server: deploy, invoke, rendered results, durable state across invocations, a tool call, HTTP mounts served out of an agent's filesystem, and agent-to-agent RPC. No stubs |

`tooling/e2e.sh` reuses a server already listening on `:9881` and otherwise starts
one (stopping it again on exit). It creates agents with a run-specific id, so
their state starts fresh on every run. It needs a golem CLI whose local server
works — the all-in-one `golem-<target>` binary, not `golem-cli-<target>` — see
the version matrix in the deploy section; CI and the release workflow install
`v1.6.0` for it. It reports a `SKIP` line, and a count at the end, for checks
that cannot run against the pinned server.

The ABI smoke test also runs against a debug build of the effects example, since
the platform has had build-sensitive ABI bugs before.

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
