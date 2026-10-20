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
│       ├── retry_json.rs       # JSON mirror of the retry policy/predicate trees
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
> The platform is pinned to one Roc nightly — **`nightly-2026-10-19-601b54c`**,
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

**Agent-to-agent calls** beyond invoke-and-await are exposed as effects:
`rpcFireAndForget!` (trigger and return the invocation's identity),
`rpcSchedule!` (the same for a call scheduled `delayMillis` from now),
`rpcScheduleCancelable!`/`rpcCancel!` (schedule and cancel before the scheduled
time), and the `parseAgentId!`/`makeAgentId!` helpers `rpc!` resolves its target
with. The e2e drives all of them from the effects agent into the counter agent —
which is also what proves cross-component calls: on Golem 1.6
`parse-agent-id` falls back to the deployment's registered agent types when the
name is not one of the calling component's own, so `rpc!` (and the rest) reach
another component's agents. On 1.5.9 the same call answered
`Agent type not found`, which is why the effects example used to call itself.

Still open:

- **A layout-sensitive trap** has been seen once in a release build of the
  effects example: `initialize` trapped while slicing a host-provided heap string
  (`Str.split_first` on the agent config). The same source passes when rebuilt,
  and a smaller re[REDACTED] did not trigger it. The `RocStr` release path has
  since been rewritten against the compiler's own protocol (see the build section
  and `tooling/glue/README.md`), which removed the known way this class of trap
  could fire; if one reappears, keep the failing `.wasm` and compare `roc build`
  with and without `--debug`.
- **The ABI harness cannot instantiate the effects agent**: the harness world
  implements no p3 `wasi:http` interface, and the effects example imports
  `wasi:http/client@0.3.0`, so the e2e is its authoritative check. Async imports
  themselves are not the obstacle — the streaming agent's
  `websocket/client.receive` call passes in the harness (see
  `tooling/abi_harness/README.md`).
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

The `site` example is the worked case for a **regular** agent's filesystem mount:
`examples/site/main.roc`, `examples/site/static/`, and the `site:` checks in
`tooling/e2e.sh` (subtree and exact bindings, 200/404, the configured response
header, that the subtree does not expose other files, and that a first read
materializes the instance's filesystem).

### Router mounts (static sites)

A second kind of mount is served entirely by Golem. Declare
`agentKind: HttpRouter` and Golem registers an `http-router` agent type: it is
parameterless, ephemeral, has no filesystem, and the app never sees a request.
Two things it can serve:

- **`staticBindings`** map public paths onto files that `golem deploy` uploads
  from the agent type's manifest `files:` list. The runtime answers matching
  `GET`/`HEAD` requests from the content-addressed blob (so responses carry an
  immutable `etag`, and no agent is created).
- **`openapiProvider`** is the document Golem merges into the deployment's
  `/openapi.yaml`, and the agent type's only method returns it. Its paths are
  relative to the mount prefix, so `/index.html` in the document is
  `/static/index.html` on the domain. A document that is not OpenAPI 3.1 JSON
  with a `paths` object is rejected at `initialize` instead of breaking the spec
  route with a 502.

```roc
metadata: {
        name: "static-site-agent",
        // ...
        agentKind: HttpRouter,
        httpMount: Mount({
                pathPrefix: ["static"],
                staticBindings: [
                        Subtree({ publicPrefix: ["assets"], filesystemRoot: "/assets" }),
                        Exact({ publicPath: ["index.html"], filePath: "/index.html" }),
                ],
                filesystemBindings: [],
                fileResponseHeaders: [{ name: "referrer-policy", value: "no-referrer" }],
                corsAllowedPatterns: ["*"],
        }),
        openapiProvider: OpenApi("{\"openapi\":\"3.1.0\", ...}"),
}
```

Router mounts are literal-only (routers have no constructor parameter to capture)
and cannot declare tools; the host refuses those combinations, along with
`staticBindings` on a regular agent, before Golem sees them.

Still to do: the **dynamic** router handler (`handle(request) -> response`),
whose request and response bodies are `stream<u8>` values. That needs the schema
graph to carry streams and the host to read the incoming body and write the
outgoing one, which the synchronous Roc-facing effects do not model yet — so
router mounts today serve uploaded files and the OpenAPI document, not computed
responses.

The `static_site` example is the worked case: `examples/static_site/main.roc`,
`examples/static_site/static/`, and the `router:` checks in `tooling/e2e.sh`
(both binding kinds, the immutable ETag, 404, and that the provider document
reaches the deployment's spec).

---

## 🔔 Webhooks

A webhook lets an agent hand a callback URL to an external system and receive
the POST it makes later, durable across invocations: the pair is a Golem
**promise** plus a URL that completes it. Two effects (`platform/Golem.roc`):

| Effect | What it does |
| --- | --- |
| `createWebhook! : {} => Try(Str, Str)` | creates the promise, registers the webhook and returns the callback URL; `Err` carries Golem's `webhook-error` (`permission-denied`, or `internal-error: …`) |
| `awaitWebhook! : Str => Try(Str, Str)` | waits for the POST to that URL and returns its body as lossy UTF-8 text, the convention `http!` uses for bytes |

The promise id never reaches the app: the host keeps it in a per-instance map
keyed by the URL, so the URL the app stores in its own state is the handle that
survives between invocations (a URL this instance never created is an error, not
a hang).

**Prerequisites.** Golem only lets agent types that are *currently* deployed
through an HTTP API create webhooks, and a deployment entry needs a mount, so
the manifest needs both:

```yaml
httpApi:
  deployments:
    local:
      - subdomain: roc-golem
        # Optional: the prefix callback URLs come out under (default `/webhooks`).
        webhookUrl: /webhooks
        agents:
          effects-agent: {}
```

The agent's `metadata.httpMount` supplies the rest — any regular mount works,
including a binding-less one like `examples/effects/main.roc`'s. Calling
`createWebhook!` from an agent type that is not deployed that way fails with the
`webhook-error` rather than trapping.

**URL shape and contract.** The callback URL is
`<scheme>://<domain>/<webhookUrl>/<agent-type-name>/<id>`, where the id is an
opaque, checksummed blob. This platform leaves the mount's `webhook-suffix`
empty, so the agent type name is the suffix. The route is:

- **POST only** — other methods answer 404,
- **no auth** — the checksummed id is the capability,
- **204** on success, with the request body delivered to the waiting
  `awaitWebhook!` as bytes,
- **404** for an id Golem cannot verify (a URL that was never created, or one
  whose id was tampered with).

The `effects` example is the worked case: `webhook-create` creates the webhook
and stores the URL in its state, `webhook-await` returns the POSTed body, and
the `webhook:` checks in `tooling/e2e.sh` POST to the returned URL and assert
the 204 and 404 answers.

---

## 🎟️ Quotas and retry

Two more host surfaces, both *synchronous* — they need no `block_on`, unlike the
WebSocket receive or the p3 HTTP client — which is why the ABI harness can drive
them with recording stubs as well as the e2e running them for real.

### Quotas (`golem:quota/types`)

A quota is an environment-level resource limit. The manifest declares the
resource, and an agent *reserves* units before the work it wants accounted for
and *commits* what that work actually used:

```yaml
resourceDefaults:
  local:
    example-requests:
      limit:
        type: Capacity        # or Rate { value, period, max }, or Concurrency
        value: 10
      enforcementAction: reject   # or throttle, terminate
      unit: request
      units: requests
```

| Effect | What it does |
| --- | --- |
| `newQuotaToken! : Str, U64 => U32` | asks the host for a token on the named resource; `expectedUse` sizes the token's credit (it consumes nothing by itself). The number returned is the handle the other quota effects take |
| `quotaReserve! : U32, U64 => Try(U32, Str)` | reserves units; `Ok` is a reservation handle for `quotaCommit!`, `Err` is the `failed-reservation` a `reject`ing resource answered with — including the host's estimate of the wait for capacity when it has one (`quota reservation failed (retry after 250ms)`). A `throttle`ing resource waits here instead, and a `terminate`ing one fails the invocation |
| `quotaCommit! : U32, U64 => Try({}, Str)` | commits what the work used: less than was reserved returns the difference to the pool, more is charged to the token as debt, and a reservation that is never committed counts as zero. A handle is spent once |
| `quotaSplit! : U32, U64 => Try(U32, Str)` | splits part of a token's expected use off into a second token |
| `quotaMerge! : U32, U32 => Try({}, Str)` | merges one token back into another (they have to name the same resource) |

Tokens and reservations are host-side resources, so the app carries them as
`U32` handles, the way WebSocket connections are carried; the table behind them
lives for as long as the agent instance. `examples/counter/main.roc`'s `quota`
command is the worked case — `quota 6` reserves and commits six of the ten
requests, and a second call is answered with `quota reservation failed` — and the
`quota:` checks in `tooling/e2e.sh` run it against a real server.

### Retry policies (`golem:api/retry`)

The manifest declares named policies, each a predicate that selects when it
applies plus the delay schedule to use:

```yaml
retryPolicyDefaults:
  local:
    send-retries:
      priority: 100
      predicate:
        propEq:
          property: "verb"
          value: "send"
      policy:
        countBox:
          maxRetries: 2
          inner:
            periodic: "100ms"      # humantime, or {secs, nanos}
```

| Effect | What it does |
| --- | --- |
| `getRetryPolicies! : {} => Str` | every policy active for this agent, as a JSON array of named policies |
| `getRetryPolicy! : Str => Try(Str, Str)` | one policy by name, or `""` when no policy has that name |
| `resolveRetryPolicy! : Str, Str, Str => Try(Str, Str)` | `(verb, nounUri, properties JSON)` → the first policy whose predicate matches, as policy JSON, or `""` when none matches. A predicate the host cannot evaluate comes back as `Err` |
| `nextRetryDelay! : Str, Str, U64, Str => Try(Str, Str)` | advances the host's evaluator once: `(policy JSON or "" for the agent's default, state JSON, elapsed nanos, {verb, nounUri, properties})` → `{"state": […], "delayNanos": … \| null}` |
| `setRetryPolicy! : Str => Try({}, Str)` | adds or overwrites a named policy (the host records it in the oplog) |
| `removeRetryPolicy! : Str => {}` | removes a named policy (also recorded) |

Policies and predicates are flattened trees (`nodes`, with the root at
`nodes[0]` and children named by index), and Roc has no resources or recursive
variants to hand those to an app, so they travel as JSON text — the same
convention `http!` uses for its response. The text is the WIT shape:

- a node is `{"tag": payload}`, or a bare string for the variants that carry no
  payload (`"immediate"`, `"predTrue"`), with the WIT variant and field names in
  camelCase,
- the root is `nodes[0]` and indices stay numbers,
- a `duration` is its nanosecond count,
- a `predicate-value` is a plain JSON string, integer or boolean.

So the policy above renders as
`{"nodes":[{"countBox":{"maxRetries":2,"inner":1}},{"periodic":100000000}]}`,
and a decision as `{"delayNanos":100000000,"state":[…]}` (with `null` for a
delay when the policy is exhausted — a zero delay still permits another
attempt). Everything the read effects print can be handed straight back to
`setRetryPolicy!` and `nextRetryDelay!`.

`examples/counter/main.roc`'s `retry` command is the worked case: it reports the
policies the environment declared, resolves the rule that matches `send`, and
walks the resolved policy's schedule twice — threading the opaque `state` the
first decision returned back into the second call. The `retry:` checks in
`tooling/e2e.sh` cover it against a real server.

Besides whatever a manifest declares, an environment carries a `default` rule
(`predTrue`, the lowest priority), which is what `getRetryPolicies!` lists and
what an operation no declared rule matches resolves to — so a `""` from
`resolveRetryPolicy!` means the environment has no rules at all, and the
`""`-for-`nextRetryDelay!` case means "use that configured default".

---

## 🧭 Oplog and execution control

`golem:api/host` is where an app reaches Golem's own execution: failing an
invocation it must not continue, naming an external write, forking the agent,
waiting for the oplog, and resolving component and agent ids. All six effects are
*synchronous* host calls — no `block_on`, the way the quota and retry interfaces
are — which is why the ABI harness drives them against stubs as well as the e2e
running them for real.

| Effect | What it does |
| --- | --- |
| `trap! : Str => {}` | fails the invocation with the reason, and never returns. Golem turns the call into a trap, so the failure is permanent rather than a result the app could branch on — a recoverable condition belongs in the app's own error handling. The agent keeps the failed invocation until an operator reverts it, which is why `revert-agent` stays a CLI operation |
| `idempotencyKey! : {} => Str` | a durable key for an external write, as a UUID string (see below) |
| `fork! : {} => Try(Str, Str)` | copies the agent at this point in the invocation: both copies continue from here, and each gets its own result — `Ok("original:<phantom-id>")` on the copy the caller is invoking, `Ok("forked:<phantom-id>")` on the new one. The phantom id is the same on both sides and is what the forked copy's agent id carries (`effects-agent("a")[<phantom-id>]`); the copies rejoin through a promise or talk to each other over `rpc!` |
| `oplogCommit! : U8 => {}` | blocks until the oplog is durable on that many replicas (or on all of them). A replication barrier for this agent's own history — not a transaction, and no guarantee about an external system |
| `resolveComponentId! : Str => Try(Str, Str)` | the component id (UUID string) of a deployed component, looked up by reference: in Golem OSS the component's name as the manifest declares it (`"roc-golem:counter"`). A reference that names no component comes back as `Err` |
| `resolveAgentId! : Str, Str => Try(Str, Str)` | `(component reference, agent name)` → `{"componentId": …, "agentId": …}`. The host resolves the reference and attaches the name; whether that agent exists is not checked (that is `resolve-agent-id-strict`, which this platform does not expose) |

### What `idempotencyKey!` is for

Golem treats remote writes as eligible for re-execution: recovery can repeat an
unfinished body, and an incomplete call can be retried. The peer therefore has to
make the write duplicate-safe, and `idempotencyKey!` is the key to hand it. Golem
derives the key from the call's position in the invocation's oplog and records it
with the invocation's outcome, so:

- two calls in one invocation are two positions and get two different keys;
- a later invocation is a new sequence — its keys differ from the earlier one's;
- a replay of the invocation (recovery, or a re-invocation under the same
  idempotency key) hands the recorded keys back instead of fresh ones, which is
  what lets the peer recognise the repeated write;
- a header carrying the key is not a guarantee by itself — the peer has to
  enforce it.

`examples/effects/main.roc`'s `idempotency` command prints two keys from one
invocation plus the previous invocation's first key, and the `idempotency:`
checks in `tooling/e2e.sh` run it against a real server, replay included.

### What 1.6 removed, and why

These are not in the 1.6 host interface on purpose — an app that finds a
pre-1.6 example using them is looking at something Golem no longer offers:

| 1.5 effect | Why it is gone |
| --- | --- |
| `get-oplog-index`, `set-oplog-index`, `mark-begin-operation`, `mark-end-operation` | guests no longer read or move the execution cursor, and they no longer put their own markers in the oplog: durability belongs to the runtime, and recovery repairs an incomplete call at its own `Start` instead of rerunning a group of completed effects |
| `get-idempotence-mode`, `set-idempotence-mode` | there is no guest switch that declares host calls idempotent. Make writes duplicate-safe at the peer (above), and do not use `is-idempotent` in retry predicates — the host no longer supplies that property |

`update-agent`, `revert-agent` and `get-agent-metadata` do exist in the host
interface, but they are operator-facing: the CLI performs them (`golem agent
update`, `golem agent revert`), so this platform does not expose them to apps.

---

## 🗄️ Key-value and blobstore

Golem gives every environment two shared stores an agent can use without any
setup: a key-value store (`wasi:keyvalue`) and a blobstore (`wasi:blobstore`).
Both belong to the *environment* rather than to one agent — every agent in it
sees the same buckets, containers and objects — and both are
permission-checked and recorded in the caller's oplog by the runtime, so a
replayed invocation is answered from the recorded calls instead of reaching the
store again.

Values are bytes on the wire. The effects carry them as lossy-UTF-8 text, the
convention `http!` and `awaitWebhook!` already use for bodies: text in, text
out, binary data only on the way through. Buckets, keys, containers and objects
are named by strings, so no handle reaches the app.

| Effect | What it does |
| --- | --- |
| `kvGet! : Str, Str => Try(Str, Str)` | `(bucket, key)` → the value as text. A key the bucket does not hold reads as the JSON literal `null`, and an empty value reads as `""`; the one blind spot of that convention is a stored value that is itself the text `null` |
| `kvSet! : Str, Str, Str => Try({}, Str)` | `(bucket, key, value)` → stores the value's UTF-8 bytes, replacing what was there. Buckets are implicit: the first write makes one, so there is nothing to open |
| `kvDelete! : Str, Str => Try({}, Str)` | removes the key. Deleting one that is not there is not an error |
| `kvExists! : Str, Str => Try(Bool, Str)` | whether the key is there, without reading the value — the direct answer when the value's own text could be ambiguous |
| `blobCreateContainer! : Str => Try({}, Str)` | creates the container if it is not there yet. A container that already exists is left alone (and so are its objects), so an app can make sure of its container with this one call |
| `blobPut! : Str, Str, Str => Try({}, Str)` | `(container, object, value)` → stores the value's bytes as that object, replacing an object with the same name. The container has to exist |
| `blobGet! : Str, Str => Try(Str, Str)` | the whole object as text. The host reads the object's size first and asks for exactly that range, so a zero-length object comes back as `""` instead of as a range error |
| `blobHas! : Str, Str => Try(Bool, Str)` | whether the container holds that object |
| `blobList! : Str => Try(Str, Str)` | the object names as a JSON array of strings (`[]` for an empty container). The interface promises no order — sort the array if the app needs a stable one. `list-objects` answers with a `stream<object-name>`, which the host drains before returning |
| `blobDelete! : Str, Str => Try({}, Str)` | deletes the object. Deleting one that is not there is not an error |

A container that does not exist is an error (the host answers `Container not
found`) rather than an empty one, so `blobPut!` into a missing container is
reported instead of quietly making one — `blobCreateContainer!` is the call that
ensures it. The two stores are shared, so an app that runs the same key or
object name in different environments (or in parallel runs against one
environment) sees them collide.

The counter example carries both round trips as commands (`kv <bucket> <key>
<value>` and `blob <container> <object> <value>`), each reporting every step's
outcome in one reply; the `kv:` and `blob:` checks in `tooling/e2e.sh` run them
against a real server.

Not exposed: `wasi:keyvalue/cache` (every operation of it is async, and the
`eventual` effects cover the same data) and `wasi:keyvalue/eventual-batch`, and
on the blobstore side `delete-container`, `container-exists`, `copy-object`,
`move-object`, `delete-objects`, `clear` and the container/object `info`
readers. The effects above are the data plane this platform commits to.

---

## 📂 The agent's own filesystem

Every agent runs in its own sandboxed filesystem, and these effects are the guest
side of it: `wasi:filesystem@0.2.6`, the synchronous revision Golem 1.6 registers
alongside the p3 one (`wasi:filesystem@0.3.0`, whose descriptor calls are `async
func`s over component-model streams — that shape is a follow-up, not covered
here). The filesystem is *per agent* rather than shared the way the key-value
and blobstore stores are, and it is durable: the runtime records every write in
the invocation's oplog and replays it, so a file one invocation wrote is there
for the next one, and the files survive restarts and snapshots. A manual agent
update carries them across an incompatible revision with the snapshot.

| Effect | What it does |
| --- | --- |
| `readFile! : Str => Try(Str, Str)` | the whole file as lossy-UTF-8 text, the convention `http!` uses for bytes |
| `writeFile! : Str, Str => Try({}, Str)` | creates the file (truncating what was there) and writes the text's UTF-8 bytes |
| `appendFile! : Str, Str => Try({}, Str)` | writes the text's bytes at the end of the file, creating it when it is not there yet |
| `fileExists! : Str => Try(Bool, Str)` | whether anything is there. A path whose parent does not exist answers `False`, not `Err` |
| `fileSize! : Str => Try(U64, Str)` | the size in bytes (for a directory, its own record; for a symlink, the length of the path it holds) |
| `deleteFile! : Str => Try({}, Str)` | removes the file. A directory or a missing path is reported (`is-directory`, `no-entry`) rather than ignored |
| `makeDir! : Str => Try({}, Str)` | creates one directory — `mkdir`, not `mkdir -p` — so its parent has to exist already |
| `listDir! : Str => Try(Str, Str)` | the entries as a JSON array of `{name, type}`, sorted by name so the listing is stable. `type` is `"file"`, `"dir"`, `"symlink"`, or `"other"` for the kinds the interface leaves unclassified |

### The path rule

Paths resolve against the agent's root: a leading `/` names it — the spelling
the manifest's `targetPath` uses — so `/seed/a.txt` and `seed/a.txt` are the
same file; `.` and empty segments are dropped, and `..` steps back inside the
root. A path that would step *above* the root fails with `escapes the agent's
filesystem root` before the host is asked, and the runtime refuses a symlink
that would leave the sandbox. Everything else a host call refuses travels as
the WIT name of its `error-code` (`no-entry`, `not-permitted`, `is-directory`,
…), which is the vocabulary the interface's own documentation uses.

### Initial files, and how this differs from a mount

The manifest's per-agent `files:` list provisions the files the agent starts
with; `golem deploy` uploads them, so they are part of the component revision:

```yaml
agents:
  counter-agent:
    files:
      - sourcePath: examples/counter/seed/hello.txt
        targetPath: /seed/hello.txt
```

That is how files get into the sandbox *before* the agent runs — and it is a
different thing from an [HTTP mount](#-http-mounts), which *serves* files out
of the same filesystem to HTTP clients without invoking the agent. An app that
wants a download route over files it writes at runtime wants a mount; an app
that wants to read and write its own files wants these effects. Both surfaces
can point at one filesystem: the site example's mount serves files that
`files:` provisioned, while the counter's `fs` commands read a provisioned file
and write their own files next to it.

The counter example carries the effects as `fs <verb> <path> [text]` commands
(`write`, `append`, `read`, `exists`, `size`, `mkdir`, `delete`, `list`), and
the `fs:` checks in `tooling/e2e.sh` run them against a real server: the
provisioned file is read, a file written in one invocation is read in a later
one (that is what shows the durability), a listing carries both, and the path
rule is exercised both ways.

Not exposed: the rest of the interface — `rename-at`, `symlink-at`,
`readlink-at`, `remove-directory-at`, the metadata-hash readers, timestamps,
and the stream-shaped `read-via-stream` / `write-via-stream` / `append-via-
stream`. The eight operations above are the file plane this platform commits
to.

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
| `rpc!` | `golem:agent/host` | invoke-and-await; target is an agent id such as `counter-agent("{}")`, and the reply text comes back |
| `rpcFireAndForget!` | `golem:agent/host` | triggers the call and returns its invocation identity (`{"agentId": …, "idempotencyKey": …}`) right away |
| `rpcSchedule!` | `golem:agent/host` | the same, `delayMillis` from now; the wall clock read is durable, so a replay schedules the same instant |
| `rpcScheduleCancelable!` / `rpcCancel!` | `golem:agent/host` | schedules and returns a `U32` handle; `rpcCancel!` cancels the invocation before its scheduled time |
| `parseAgentId!` / `makeAgentId!` | `golem:agent/host` | an agent id split into `{"agentType", "constructor", "phantomId"}` and back (`makeAgentId!` always builds ids without a phantom id) |
| `createWebhook!` / `awaitWebhook!` | `golem:agent/host` + `golem:api/host` | creates a promise and the callback URL that completes it; a POST to the URL delivers its body to `awaitWebhook!` (see [Webhooks](#-webhooks)) |
| `websocketConnect!` / `websocketSend!` / `websocketReceive!` / `websocketClose!` | `golem:websocket/client` | connections are tracked by a `U32` handle returned from `websocketConnect!` |
| `newQuotaToken!` / `quotaReserve!` / `quotaCommit!` / `quotaSplit!` / `quotaMerge!` | `golem:quota/types` | reserve and commit units of a resource the manifest declares; tokens and reservations are `U32` handles (see [Quotas and retry](#-quotas-and-retry)) |
| `getRetryPolicies!` / `getRetryPolicy!` / `resolveRetryPolicy!` / `nextRetryDelay!` / `setRetryPolicy!` / `removeRetryPolicy!` | `golem:api/retry` | retry policies and their predicate/policy trees, rendered as JSON text (see [Quotas and retry](#-quotas-and-retry)) |
| `trap!` / `idempotencyKey!` / `fork!` / `oplogCommit!` / `resolveComponentId!` / `resolveAgentId!` | `golem:api/host` | fail an invocation, name an external write durably, copy the agent, wait for the oplog to reach replicas, and resolve component and agent ids (see [Oplog and execution control](#-oplog-and-execution-control)) |
| `kvGet!` / `kvSet!` / `kvDelete!` / `kvExists!` | `wasi:keyvalue` | the environment's key-value store; buckets are implicit and the value travels as text, with a missing key reading as `null` (see [Key-value and blobstore](#-key-value-and-blobstore)) |
| `blobCreateContainer!` / `blobPut!` / `blobGet!` / `blobHas!` / `blobList!` / `blobDelete!` | `wasi:blobstore` | the environment's blobstore: containers of named objects, bodies as text, and `blobList!`'s object names as a JSON array (see [Key-value and blobstore](#-key-value-and-blobstore)) |
| `readFile!` / `writeFile!` / `appendFile!` / `fileExists!` / `fileSize!` / `deleteFile!` / `makeDir!` / `listDir!` | `wasi:filesystem` (p2) | the agent's *own* filesystem: per-agent and durable, provisioned from the manifest's `files:` list, with paths resolved against the agent's root (see [The agent's own filesystem](#-the-agents-own-filesystem)) |

The linker only keeps the effects an app actually calls, so a component's import
list stays as small as the agent's behaviour.

---

## 🧪 Testing

Three layers, all of which run in CI and in the release workflow before
anything is published:

| Layer | Command | What it proves |
| --- | --- | --- |
| Host unit tests | `cargo test --manifest-path host/Cargo.toml --lib` | the Rust side of the ABI: `RocStr`/`Result` layouts, allocator round-trips, URL and method parsing, agent-type discovery against a stubbed guest, and the `httpMount` metadata mapping |
| ABI smoke test | `tooling/abi_harness` (see its README for the flags) | a real component instantiated outside Golem: agent metadata (including the mount path), `initialize`, message round-trips with state, tool calls, and that the effects an app uses reach the host — the host side of those is stubbed and recorded, quota, retry, oplog/execution-control calls (`trap`, `oplog-commit`, the resolvers), the key-value/blobstore round trips and the agent-filesystem round trip (write in one invocation, read in the next) included |
| End-to-end | `GOLEM_CLI=/path/to/golem ./tooling/e2e.sh` | all of the above plus a real Golem server: deploy, invoke, rendered results, durable state across invocations, the tool surface, HTTP mounts served out of an agent's filesystem, an HTTP router's uploaded files and OpenAPI document, a snapshot-based agent update across an incompatible revision, agent-to-agent calls into another component (RPC, fire-and-forget, scheduled and cancelable invocations, agent-id helpers), a webhook created from an agent and completed by a POST (204/404), a quota reservation that fits and one the resource rejects, retry policies read, resolved and walked, key-value and blobstore round trips through the environment's own stores, the agent's own filesystem (a manifest-provisioned file read back, a write in one invocation read in a later one, listings, and the path rule), and `golem:api/host`'s execution control: a trapped invocation, `oplog-commit`, both resolvers, a fork seen from the original and from the copy, and idempotency keys across invocations and a replay. No stubs |

`tooling/e2e.sh` reuses a server already listening on `:9881` and otherwise starts
one (stopping it again on exit). It creates agents with a run-specific id, so
their state starts fresh on every run. A quota resource's capacity is not per
agent, though: it is environment-level and durable, so the section that reserves
from `example-requests` reports a `SKIP` for the reservation that fits when the
data directory has already spent it. It needs a golem CLI whose local server
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
