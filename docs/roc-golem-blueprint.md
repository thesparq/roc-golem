# roc-golem: blueprint for external review

A single self-contained document describing what this platform is, what the
reference application built on it looks like, which of the features an upstream
blueprint asked for actually exist, and what the remaining work would cost.

It is written to be handed to a reader (human or agent) who has **no access to
the repository**, so every claim either quotes real code or says plainly that it
is a proposal. Nothing here is invented: each code block is an exact copy of a
file in the repository, expanded mechanically rather than retyped (§0.4), and §5
lists which claims are tested and how.

---

## 0. Orientation

### 0.1 What this project is

**roc-golem is a Roc platform for Golem Cloud.**

Golem runs *agents*: WebAssembly components that hold state between calls, are
addressable by name, survive restarts by replaying what they did, and can expose
methods, tools and HTTP routes, plus access to databases, key-value stores and a
durable filesystem. Golem's own SDKs are Rust, TypeScript, Go and Scala. This
project is the bridge that lets you write those agents in
[Roc](https://www.roc-lang.org) instead.

You write an ordinary Roc value describing your agent — its state, its methods
and their typed handlers, the tools it publishes, the config it reads, the HTTP
surface it exposes. The platform you compile against turns that value into a
component Golem can run. You never write JSON by hand: a handler takes and
returns typed Roc values, and the platform derives the codecs.

**Three things to hold in mind, because they explain everything else.**

1. **A Roc platform is a compile-time contract, not a library.** An app declares
   `pf: platform "..."`; the platform declares what it needs back — here, one
   value named `agent`. Everything else (effects, types, encoders) flows from the
   platform into the app.
2. **There are two halves, in two languages.** The Roc half is what an app imports
   and what an app author reads. The Rust half is compiled to wasm and linked in;
   it speaks Golem's component-model interfaces and calls back into the Roc half.
   Between them the boundary is JSON text, which is why the platform's central job
   is making that boundary invisible.
3. **`golem.yaml` is the deployment half.** Code decides what an agent *is*; the
   manifest decides what gets *deployed*, where it is reachable over HTTP, and
   which tools the runtime will route. A working system needs both, which is why
   this document covers both.

**What you can build today**, in one sentence: durable stateful agents with typed
methods, read-only methods, config, scheduled and fire-and-forget self-calls,
agent-to-agent RPC, webhooks, tools, HTTP mounts and routers, their own durable
filesystem, key-value and blob storage, three SQL databases, permission cards,
quotas, retry policies and external durable streams — every one of them exercised
against a real Golem server on every change (§5).

**Where to start.** §1 for the inventory and the honest score. §2 for a working
agent you can read top to bottom. §3 for the platform itself. §4 for the parts
that surprise people. §6 for the open questions.

### 0.2 Provenance

An earlier document (generated with Gemini) laid out a target for this project:
a Roc platform for Golem 1.6 reaching *100% in-code feature parity* with Golem's
Rust and Go SDKs, demonstrated by a reference application called the **Release
Guardian** — a canary-release orchestrator with a log-analyzer tool and a
stateless diagnostics worker. It is the reason this platform has the shape it
does, and this document keeps its structure on purpose.

One thing to keep in mind when porting code from it: the Roc it writes is not
always the Roc this project compiles against. Builder chains
(`.with_middleware!(...)`), derivation-slot blocks (`.{ encoder_for : _, parser_for
: _ }`), defaulted record fields, `?field` presence access, `var`/`for`/`return`,
the `?` error operator and an opaque `Secret(Str)` are not part of it. §1.3 is a
one-to-one mapping, so a reader coming from that document can translate rather
than guess.

This document is a **replica**: the same shape — a feature matrix, a reference
application laid out across files, the platform source, and explanations — but
built from code that exists, compiles, and is covered by tests. Where the
reference document described something this platform cannot do today, that is
recorded as a gap with the real alternative named, not quietly dropped.

### 0.3 Status key

Every capability and every code sample in this document is labelled with one of
three words, and the labels are used strictly:

| Label | Meaning |
| --- | --- |
| **CURRENT** | In the repository today. Compiles, is exercised by the end-to-end suite against a real Golem server, and (where it is a language feature) is used by real code quoted here. |
| **PARTIAL** | Part of it exists and works; a named piece does not. The missing piece is described, not hand-waved. |
| **TARGET** | Designed but not built. No code in this document is TARGET code unless it is marked as such; where it is, it is clearly separated from the verbatim listings. |

### 0.4 What is quoted verbatim, and what is summarised

| Part | Verbatim from the repository |
| --- | --- |
| The reference application | all three `.roc` files and `golem.yaml`, in full |
| The Roc platform | all five `.roc` files, in full |
| The Rust host | **summarised** — inventory, role, and the ABI contract. It is ~13,000 lines of Rust (`host/src/`); quoting it in full would triple this document and drown the parts a reviewer needs. The file list and the exact roles are given in §3.2 so nothing is hidden. |

Code blocks below were expanded mechanically from the files at commit `5e08694`,
so they are exact copies rather than retyped approximations.

### 0.5 The release this describes

The platform is published: **Golem 1.6.0 GA**, Roc nightly
`nightly-2026-10-27-de79a0f`, platform release **v0.6.0**. The reference
application in §2 is the version shipped in that release, plus one later commit
that split it from one file into three (§2.2).

---

## 1. The support inventory

### 1.1 Feature matrix: the fifteen in-code capabilities

Golem 1.6 splits its features between `golem.yaml` (manifest) and component code.
The gemini blueprint identified fifteen capabilities that live *in code*. Here is
each one, the spelling the blueprint gave it, what this platform actually offers,
and where that leaves us.

| # | Capability (Golem 1.6) | The blueprint's spelling | This platform today | Status |
| --- | --- | --- | --- | --- |
| 1 | Agent definition & modes | `Agent.define(Config.{ mode: Durable \| Ephemeral })` | `define` / `defineAgent`, `agentKind` is `Regular` or `HttpRouter` | **PARTIAL** |
| 2 | Typed config & opaque secrets | `Config<T>`, `Secret<T>` | `getConfig!` and `config: [{ name, secret }]` — secrets arrive as **values**, not handles | **PARTIAL** |
| 3 | Method metadata & prompt hints | `Agent.method(MethodConfig.{ name, description, prompt_hint })` | `method({ name, description, promptHint })` | **CURRENT** |
| 4 | Read-only methods & caching | `#[read_only(cache = ...)]` | `registerReadOnly`, `readOnly: True` on tools, until-write cache | **CURRENT** |
| 5 | Streaming methods & durable streams | `Stream<T>`, `DurableStream<T>`, `Agent.method_stream` | `durableStreamReader!` / `Writer!` / `Read!` / `Append!` / `Release!` — **external** streams only | **PARTIAL** |
| 6 | Custom snapshots | `save_snapshot` / `load_snapshot` | `serializeState` / `deserializeState` plus `golem:api/save-snapshot` / `load-snapshot` | **CURRENT** |
| 7 | Typed cross-agent RPC & triggers | `.call()` / `.trigger()` | `rpc!` (invoke-and-await), `rpcFireAndForget!` | **CURRENT** |
| 8 | Scheduled invocations | `schedule_at(...)` / `schedule_in(...)` | `rpcSchedule!`, `rpcScheduleCancelable!`, `rpcCancel!` | **CURRENT** |
| 9 | Runtime reflection | `reflection::discover_agent_type(...)` | id resolution only: `resolveComponentId!`, `resolveAgentId!` | **ABSENT** |
| 10 | Permission cards | `PermissionCard`, `Permission.verify_card!` | the whole card surface: `permissionsWallet!`, `permissionInspect!`, `permissionDerive!`, `permissionRevoke!`, `permissionValidateGrant!`, … | **CURRENT** |
| 11 | Exported tools & tool middleware | `#[tool]`, `ToolMiddleware` | tools published on `golem:tool/guest`; **no middleware** (deliberate) | **PARTIAL** |
| 12 | Calling imported / built-in tools & MCP | generated tool client | not exposed: an agent can call another *agent*, not another component's *tool* | **ABSENT** |
| 13 | Artifacts: routers, static & live files | `Artifact.router(...).static_dir(...).live_file(...)` | HTTP mounts — `staticBindings`, `filesystemBindings`, an OpenAPI provider, and a dynamic router handler | **CURRENT** |
| 14 | Durability controls & atomic regions | `atomically(|| ...)`, `Durability.with_retry_policy!` | `setPersistence!`, `trap!`, `idempotencyKey!`, `fork!`, `oplogCommit!` — but **no atomic region wrapper** | **PARTIAL** |
| 15 | Golem promises (external webhooks) | `Promise.create!()` / `Promise.await!` | `createWebhook!` / `awaitWebhook!` — the POST callback completes a durable promise | **CURRENT** |

**The count: 8 CURRENT, 5 PARTIAL, 2 ABSENT, out of 15.**

| | Capabilities |
| --- | --- |
| CURRENT | 3, 4, 6, 7, 8, 10, 13, 15 |
| PARTIAL | 1, 2, 5, 11, 14 |
| ABSENT | 9, 12 |

Read the number carefully: this is a *feature* count, not a measure of how much
work is left. Eight of the fifteen are done, and the two ABSENT rows are
capabilities nobody has needed yet (#9 and #12). The five PARTIAL rows are the
interesting ones, and each is a different kind of gap — one is a missing piece of
an otherwise-working feature (#5, #11), one is a deliberate design decision
(#14), one is a shape difference (#1), and one is the platform's largest open
question (#2).

### 1.2 The five gaps, in plain language

**#1 Agent modes — durable yes, ephemeral only as a router.**
A `Regular` agent is durable: it has state, it is snapshotted, and the state
survives restarts (that is what §2's coordinator is). An `HttpRouter` is
ephemeral — no state, no filesystem — but a router is a *website*, not a
short-lived agent you can also call by name. So "run this work in a throwaway
agent" is not something this platform offers outside the router shape. The
gemini blueprint's `DiagnosticsWorker` — a stateless agent called by RPC for
fan-out — has no counterpart here.

**#2 Config and secrets — values, not capabilities.**
An app declares the config keys it reads (`{ name: "region", secret: False }`)
and reads them with `getConfig! : Str => Try(Str, Str)`. A key declared
`secret: True` is resolved from the environment's secret store — but what reaches
the app is the secret's **text**. There is no `Secret<T>` handle: an app cannot
hand a secret to the host and have the host use it, so it cannot, for example,
authenticate an HTTP request without the plaintext passing through its own
memory. This is the platform's most consequential gap, because it is also what
blocks authenticated external durable streams (#5) and any host-mediated secret
use. Closing it means surfacing Golem's `secret` resource as a Roc handle.

**#5 Streams — external ones work; methods that stream do not.**
"Durable streams" is three things. (a) Reading and writing *external* services
that speak the Durable Streams protocol: **done**, five effects, exercised in the
end-to-end suite against the protocol's own reference server. (b) A method whose
*parameter* is a stream: the machinery is proven (a router's request body is
exactly a host-created stream, and the suite covers it) but the surface is not
exposed. (c) A method whose *result* is a stream, and the HTTP streaming
endpoints derived from it: likewise unbuilt. (b) and (c) are each "one more
declared method plus one more hook" — the same shape of change as
`handleRequest!` and `handleMultimodal!` were — plus the durable-session route
options the platform does not build.

**#11 Tools — publishing works, middleware is out of reach.**
An app publishes tools, they appear in `golem tool list`, and they are
invokable — §2.6 covers the manifest rule that makes that work, and §4.4 covers
the stateless-instance rule that a caller must know. What is missing is
*middleware* (a component that intercepts another component's tool calls). It is
deliberately unsupported: Golem identifies a middleware by its exported
interface name, so it cannot be reached from an agent-shaped component — it needs
a component shape of its own, and it buys nothing for an app that just wants to
publish tools.

**#14 Durability — the checklist works, the wrapper does not.**
`setPersistence!` chooses whether an invocation persists nothing, state only, or
everything; `trap!` fails an invocation deliberately; `idempotencyKey!` gives a
replay a stable identity; `fork!` forks the oplog; `oplogCommit!` commits it.
What is absent is the `atomically(|| { ... })` region — a way to wrap a group of
effects so they commit or fail together. Golem's `golem:durability` interface
carries it, and the platform's inventory records the decision to leave it
unexposed on purpose: everything this platform does is already oplog-recorded,
so the wrapper would add a schema-value ceremony for no app-level benefit.
Recorded so the decision is visible rather than forgotten.

### 1.3 Language reality check: what the blueprint assumed Roc was

This is the part most likely to mislead a reader of the original blueprint, so it
is stated as a table. The middle column is what the blueprint wrote; the right
column is what this platform's code does.

| Concept | The blueprint's Roc | This platform's Roc |
| --- | --- | --- |
| Files and modules | `module [ ... ]` header at the top of each file | **superseded** — the header is deprecated. A *type module* is a headerless file whose top-level nominal type matches the filename, and it hides better: associated items inside `.{ }` are public, everything else at the top level is private (§2.2). |
| Naming a type | `Id := { ... }` | **same** — used as `Types := {}.{ ... }` to group a file's types |
| A structural alias | — | `State : { service : Str, ... }` — used pervasively |
| An opaque type | `Secret(a)`, `Card` | `Host :: [].{ ... }` — used once, for the host-effect signatures |
| An inline method block | `.{ encoder_for : _, parser_for : _ }` | `.{ ... }` exists, but it holds **type aliases and function signatures**, not derivation slots. There is no `encoder_for : _`. |
| Deriving JSON codecs | `_` placeholders in the block above | **abilities**: `build : Spec(state) -> Agent(state) where [state.Json.Encodable(_), state.Json.Parseable(...)]` — the compiler derives the codec from the type (§4.3) |
| Naming a record's template | Builder chains: `.with_middleware!(...)`, `.register(...)`, `.implement_configured!(...)` | **not Roc** — Roc has no method-call syntax. The platform uses pipelines: `define(...) \|> withConfig(...) \|> register(...) \|> build` |
| Optional fields | `field ? T`, then `plan.?notify_agent_type` | **not Roc** — the platform uses `Try(T, [Missing])` in the parsed shape and `?? default` to read it |
| Defaulted fields | `field : T = default` in the type | **not Roc** — records are structural; a default lives in the value that creates it |
| Mutable locals and loops | `var x = 0`, `for line in lines { ... }` | **not Roc** — no `var`, no `for`, no `return`. The canary counts occurrences by recursion (`countOccurrences`, §2.4) |
| Errors | `(...)?` after a call, `return Err(...)` | **not Roc** — values are `Try(ok, err)` and the code `match`es on them |
| Pure vs effectful | both spellings appear, undifferentiated | **real and load-bearing**: `->` is pure, `=>` may perform effects. `registerReadOnly` accepts only a `->` function, so the compiler *forbids* a read-only method from writing state or calling an effect (§4.2) |
| Tag unions | `[Healthy, Degraded(Str), Critical(Str)]` | **same** — see `AgentKind : [Regular, HttpRouter]`, `PersistenceMode`, `HttpMountOption` |
| Record update | implied by the builder chains | `{ ..state, weight: 100, events: events }` — used throughout |
| String interpolation | `"... ${x} ..."` | **same** — used throughout |
| Calls from the blueprint that have no counterpart | `Tool.invoke_mcp!`, `Tool.invoke_local!`, `Stream.emit!`, `Stream.fork!`, `Promise.create!`, `Reflection.discover_agent!`, `Artifact.write_live_file!`, `Durability.atomic!`, `Secret.format_header!`, `Permission.verify_card!`, `Rpc.invoke!`, `Log.info!` | `rpc!`, `rpcFireAndForget!`, `createWebhook!`, `awaitWebhook!`, `logInfo!`, `writeFile!`, `getConfig!`, … — the effect names differ and the set is smaller |

**The count: of the fifteen *construct* rows above, eight are real Roc spelled
exactly as the blueprint wrote it (`module`, `:=`, `::`, structural aliases,
`->`/`=>`, tag unions, record update, string interpolation); two are real Roc
that means something different there (`.{ }` holds type aliases and function
signatures here rather than derivation slots, and codecs come from ability
bounds instead); and five are not part of Roc at all — builder chains, `?field`
presence access, defaulted record fields, `var`/`for`/`return`, and the `?`
error operator.**

The last row is a separate matter, and the blunter one: it lists calls, not
syntax, and none of them exist here under those names.

Read that as a translation table rather than a verdict: the reference document's
*architecture* carries over almost intact and its *spelling* does not — five of
the fifteen constructs above are not part of Roc at all. Anything ported from it
needs that pass before it will compile. §2 is what it looks like already spelled
correctly.

### 1.4 What "peak" means here

Three tiers, used consistently below:

- **CURRENT** — shipping and verified. 8 of the 15 capabilities, the whole
  reference application in §2, and the whole platform in §3.
- **PARTIAL** — the honest middle. The five rows above live here: something real
  exists, and a specific, nameable piece does not.
- **TARGET** — the peak state: all fifteen capabilities at CURRENT, spelled in
  real Roc, with no PARTIAL rows.
- §4.6 walks the distance from here to there, one capability at a time.

There is no fourth tier, and nothing in this document is described as TARGET
without saying what it would cost.

---

## 2. The reference application: the canary rollout agent

### 2.1 What it does

The application is called **Release Guardian**. It is the operational half of the
gemini blueprint's story: a durable agent that orchestrates a canary release and
then guards it.

A canary release means sending a small percentage of traffic to a new version
before committing to it. The agent holds the rollout's state, and a human
approves or rejects the promotion:

1. **Start.** A caller invokes `start_canary` with a version and a target
   percentage. The agent records the rollout in its state, writes a dashboard
   file, and schedules a safety timer: a self-call in five minutes that will roll
   the release back if nobody answers the gate.
2. **Watch.** `get_status` reports the rollout. It is declared *read-only*, so
the runtime may answer it from a cache and the compiler forbids it from writing
   anything.
3. **Decide.** A human invokes `approve_gate` with their name and a decision.
   Approval promotes the canary to 100% and rewrites the dashboard; rejection
   returns it to 0%.
4. **Rescue.** `emergency_rollback` takes the canary out of rotation immediately.
   This is the same method the scheduled safety timer calls.
5. **Inspect.** A log-analyzer tool becomes available to any caller through
   Golem's tool surface, and the dashboard is served over HTTP as it is rewritten.

The whole thing is one agent type in one component, and everything it does is
recorded in Golem's invocation log, so a crash mid-release replays rather than
losing the rollout's history.

### 2.2 The three files

The example is deliberately small, and it is laid out the way a multi-file Roc
app is laid out:

| File | What it holds |
| --- | --- |
| `Types.roc` | The state the platform snapshots, and the typed inputs and outputs the methods exchange. A headerless *type module*: a file whose top-level type is named after the file. |
| `ReleaseCoordinator.roc` | Everything behavioural: the config declaration, the dashboard mount, the tool, the four methods and their handlers, the dashboard renderer, and the `Agent` value itself. |
| `main.roc` | The `app` header and a single binding. The platform looks for a value named `agent` in the app file. |

Two facts about this split were learned the hard way, and both are worth knowing
before writing a second Roc file:

- **The app header must *define* the entry point.** `app [agent]` with
  `import ReleaseCoordinator exposing [agent]` is rejected — "exposed but not
  defined". Re-exporting an imported name does not satisfy the header; a plain
  binding does: `agent = ReleaseCoordinator.agent`.
- **`module [...]` is deprecated, and its replacement hides *better*, not worse.**
  The compiler is explicit: *"The module header is deprecated. ... Type modules
  (headerless files with a top-level type matching the filename) are now the
  preferred way to define modules. Remove the module header and ensure your file
  defines a type that matches the filename."*

  A **type module** is a headerless file whose top-level *nominal* type — `:=`, or
  `::` to make it opaque — is named after the file. Its **associated items**
  (anything inside that type's `.{ ... }` block) are what other files can import;
  every other top-level definition is private to the file. Verified in both
  directions rather than assumed: an associated value imported cleanly from the
  app, and a top-level helper was refused with `value not exposed`.

  `ReleaseCoordinator.roc` therefore uses that form. Its entire public surface is
  one associated value, `agent`; the config declaration, the mount, the tool, the
  methods and their handlers all sit below the block and are private.

  One platform file is still on the old form: `platform/Golem.roc` carries a
  deprecated `module [...]` header over roughly 200 exports, and warns. Moving it
  is a separate change — it is the interface every example imports — and §1.2
  does not depend on it.

  Note the two distinct roles in this app, because Roc's categories are per-file:
  `main.roc` is an *application module* and must expose the entry point the
  platform requires, while `Types.roc` and `ReleaseCoordinator.roc` are *type
  modules* with only their nominal type visible.

### 2.3 `examples/canary_release/Types.roc`

```roc
## The release coordinator's types: the state the platform snapshots, and the
## typed inputs and outputs its methods exchange.
##
## A headerless file whose top-level type is named after the file is a Roc "type
## module": every name inside the block below is importable from the other files
## in this app (`import Types exposing [State, ...]`). The platform groups its own
## types the same way (`platform/Types.roc`).
Types := {}.{
	## The rollout's own record. `events` is the durable log the read-only status
	## method reports, so the app's history lives in the state the platform
	## snapshots rather than anywhere else.
	State : {
		service : Str,
		canary : Str,
		weight : U64,
		events : List(Str),
		region : Str,
	}
	StartInput : { version : Str, weight : U64 }
	StartOutput : { version : Str, weight : U64 }
	DecisionInput : { approved : Bool, reviewer : Str }
	RollbackInput : { reason : Str }
	Status : { service : Str, canary : Str, weight : U64, events : List(Str), region : Str }
}
```

### 2.4 `examples/canary_release/ReleaseCoordinator.roc`

```roc
## The release coordinator agent: its methods, the typed handlers bound to them,
## the tool it publishes on `golem:tool/guest`, and the mount that serves the
## dashboard it writes.
##
## This file is a Roc *type module*: a headerless file whose top-level nominal
## type is named after the file. Everything associated with that type — inside
## `ReleaseCoordinator.{ ... }` — is public, and everything else at the top level
## of the file is private to it. Here that means exactly one public value,
## `agent`, while the helpers below stay helpers. `main.roc` is the app entry
## point and re-exposes `agent`.

import pf.Golem exposing [
	Agent,
	build,
	define,
	getConfig!,
	logInfo!,
	method,
	register,
	registerReadOnly,
	rpcSchedule!,
	withConfig,
	withMount,
	withToolHandler,
	withTools,
	workerId!,
	writeFile!,
]
import Types exposing [DecisionInput, RollbackInput, StartInput, StartOutput, State, Status]

ReleaseCoordinator := {}.{
	## The agent the platform's `requires` looks for. This annotation is also the
	## whole public surface of this module.
	agent : Agent(State)
	agent =
		define(
			{
				name: "canary-release",
				version: "1.0.0",
				description: "Canary rollout orchestrator: typed methods, a gate, a live dashboard.",
			},
			|service|
				{
					service: service,
					canary: "",
					weight: 0,
					events: [],
					region: "unknown",
				},
		)
			|> withConfig(configDeclarations)
			|> withMount(dashboardMount)
			|> withTools(tools)
			|> withToolHandler(handleTool)
			|> register(startCanary, handleStart!)
			|> register(approveGate, handleApprove!)
			|> register(emergencyRollback, handleRollback!)
			|> registerReadOnly(statusMethod, handleStatus)
			|> build
}

## ---------------------------------------------------------------------------
## Everything below is private to this module: no other file can import it.
## ---------------------------------------------------------------------------

## Config the agent reads with `getConfig!`. The platform has no secret-handle
## surface yet (`docs/golem-1.6-parity.md`), and a `secret: True` declaration
## would require the manifest to supply the key as `secret<T>`, so this app
## declares only the plain key it reads.
configDeclarations = [
	{ name: "region", secret: False },
]

## The dashboard the agent writes into its own durable filesystem. The mount
## below serves it from the deployment's domain (`httpMount`): the `{id}` capture
## is the agent's constructor parameter, so the URL selects which instance's
## dashboard is read, and a matching GET never invokes the agent.
dashboardPath = "/dashboard.html"

dashboardMount = Mount({
	pathPrefix: ["canary", "{id}"],
	staticBindings: [],
	filesystemBindings: [
		Exact({ publicPath: ["dashboard.html"], filePath: dashboardPath }),
	],
	fileResponseHeaders: [],
	corsAllowedPatterns: [],
})

## The tool this agent publishes (`golem:tool/guest`). `readOnly: True` maps to
## the 1.6 read-only flag, so the runtime may cache its answer.
tools = [
	{
		name: "analyze_logs",
		description: "Scans log lines for panics, OOMs and latency spikes.",
		parameters: [],
		readOnly: True,
	},
]

## Declared methods. The name is what the CLI/RPC invokes; the description and
## prompt hint are what the tooling shows a caller.
startCanary = method({
	name: "start_canary",
	description: "Starts a canary rollout and returns the gate's approval URL.",
	promptHint: "Call this to begin rolling a version out to a percentage of traffic.",
})

approveGate = method({
	name: "approve_gate",
	description: "Records a reviewer's decision for a pending gate.",
	promptHint: "Call this once a human has approved or rejected the canary.",
})

emergencyRollback = method({
	name: "emergency_rollback",
	description: "Rolls the canary back to 0% of traffic.",
	promptHint: "Call this to take the canary out of rotation immediately.",
})

statusMethod = method({
	name: "get_status",
	description: "Reports the current rollout state, without writing anything.",
	promptHint: "",
})

## A blank line for `Err`-happy effects whose answer we do not need; the effect
## has already been recorded, so dropping the text costs nothing.
ignoreEffect : Try({}, Str) -> {}
ignoreEffect = |res|
	match res {
		Ok(_) => {}
		Err(_) => {}
	}

ignoreText : Try(Str, Str) -> {}
ignoreText = |res|
	match res {
		Ok(_) => {}
		Err(_) => {}
	}

## The live dashboard the agent writes into its own durable filesystem. Every
## write is recorded in the invocation's oplog and replayed, so the file
## survives restarts and snapshots. Its shapes are ordinary values — the
## platform's derived codecs carry them, never hand-written JSON.
renderDashboard : Str, Str, U64, List(Str) -> Str
renderDashboard = |service, canary, weight, events| {
	eventItems = List.map(events, |e| "<li>${e}</li>")
	"<!doctype html><html><body><h1>Release Guardian: ${service}</h1><p>Canary ${canary} at ${U64.to_str(weight)}%</p><ul>${Str.join_with(eventItems, "")}</ul></body></html>"
}

writeDashboard! : Str, Str, U64, List(Str) => {}
writeDashboard! = |service, canary, weight, events|
	ignoreEffect(writeFile!(dashboardPath, renderDashboard(service, canary, weight, events)))

## The tool handler: the blueprint's `LogAnalyzerTool`, reduced to what the
## string-in / string-out tool surface carries. `call.arguments` is the caller's
## text; `stdout` is written to the stream the caller attached. The parameter and
## result are left to inference from `withToolHandler`.
handleTool = |state, call|
	if call.name == "analyze_logs" {
		faults = countLogFaults(call.arguments)
		{
			state,
			result: {
				id: call.id,
				success: True,
				output: "{\"faults\": ${U64.to_str(faults)}}",
				stdout: "analyze-logs: ${U64.to_str(faults)} fault(s)\n",
				stderr: "",
			},
		}
	} else {
		{
			state,
			result: {
				id: call.id,
				success: False,
				output: "{\"error\": \"Unknown tool ${call.name}\"}",
				stdout: "",
				stderr: "",
			},
		}
	}

## Counts the lines mentioning a panic or an OOM, the way the blueprint's
## analyzer scans raw container logs. Roc has no `Str.split` in this nightly, so
## the count walks the text one occurrence at a time.
countLogFaults : Str -> U64
countLogFaults = |text|
	countOccurrences(text, "PANIC") + countOccurrences(text, "OOM")

countOccurrences : Str, Str -> U64
countOccurrences = |text, needle|
	match Str.split_first(text, needle) {
		Err(_) => 0
		Ok(split) => 1 + countOccurrences(split.after, needle)
	}

## The typed handlers. Each is a `state, input => ...` function annotated on its
## binding; `register` / `registerReadOnly` read the input and output types from
## it and derive the JSON codecs, so the handlers deal in values, never text.
handleStart! : State, StartInput => Try({ state : State, out : StartOutput }, Str)
handleStart! = |state, input| {
	region = match getConfig!("region") {
		Ok(value) => value
		Err(_) => "unknown"
	}
	logInfo!("canary ${input.version} at ${U64.to_str(input.weight)}% in ${region}")
	events = List.append(state.events, "canary ${input.version} at ${U64.to_str(input.weight)}%")
	writeDashboard!(state.service, input.version, input.weight, events)
	# The safety timer: a scheduled self-call that rolls back if the gate is never
	# answered. The invocation identity it answers with is a follow-up the app
	# would keep to cancel the timer on approval.
	ignoreText(
		rpcSchedule!(
			workerId!({}),
			"emergency_rollback",
			"{\"reason\":\"approval gate timed out\"}",
			300000,
		),
	)
	Ok({
		state: {
			..state,
			canary: input.version,
			weight: input.weight,
			events: events,
			region: region,
		},
		out: { version: input.version, weight: input.weight },
	})
}

handleApprove! : State, DecisionInput => Try({ state : State, out : Str }, Str)
handleApprove! = |state, input| {
	if input.approved {
		events = List.append(state.events, "approved by ${input.reviewer}")
		writeDashboard!(state.service, state.canary, 100, events)
		Ok({
			state: { ..state, weight: 100, events: events },
			out: "Promoted ${state.canary} to 100% (approved by ${input.reviewer})",
		})
	} else {
		events = List.append(state.events, "rejected by ${input.reviewer}")
		writeDashboard!(state.service, state.canary, 0, events)
		Ok({
			state: { ..state, weight: 0, events: events },
			out: "Canary ${state.canary} rejected by ${input.reviewer}",
		})
	}
}

handleRollback! : State, RollbackInput => Try({ state : State, out : Str }, Str)
handleRollback! = |state, input| {
	logInfo!("emergency rollback: ${input.reason}")
	events = List.append(state.events, "rolled back: ${input.reason}")
	writeDashboard!(state.service, state.canary, 0, events)
	Ok({
		state: { ..state, weight: 0, events: events },
		out: "Rolled ${state.canary} back to 0%: ${input.reason}",
	})
}

## Read-only: a pure `->` handler. It may not write the state or perform
## effects, which is exactly what the 1.6 read-only flag declares.
handleStatus : State, {} -> Status
handleStatus = |state, _input| {
	service: state.service,
	canary: state.canary,
	weight: state.weight,
	events: state.events,
	region: state.region,
}
```

### 2.5 `examples/canary_release/main.roc`

```roc
app [agent] { pf: platform "../../platform/main.roc" }

# Release Guardian — a canary-rollout orchestrator written against the typed
# authoring surface in `platform/Golem.roc`. Each method is declared once with
# `method`, bound to a typed handler with `register` / `registerReadOnly`, and
# the platform derives the metadata, the JSON dispatch and the state codecs.
#
# The blueprint's `ReleaseCoordinator` also used permission cards as values,
# secret handles, tool middleware and method streams. This platform does not
# expose those yet (see `docs/golem-1.6-parity.md`), so this app uses what it
# does expose: typed methods, config, a scheduling effect, an approval gate, the
# agent's durable filesystem, and a tool.
#
# The app is three files: this one names the value the platform's `requires`
# looks for (the header must *define* it, so a re-export is a plain binding),
# `Types.roc` carries the wire and state types, and `ReleaseCoordinator.roc`
# builds the agent itself.

import ReleaseCoordinator

agent = ReleaseCoordinator.agent
```

### 2.6 `golem.yaml`

The manifest is where the deployment-shaped half of the story lives: what is
deployed, where it is reachable, and which tools the runtime will route. It is
reproduced in full because the comment blocks in it are part of the
documentation.

Two rules are worth calling out before reading it, because both cost this
project a full debugging cycle:

- **Every tool needs two declarations, not one.** A tool the component publishes
  must be declared in the top-level `tools:` map (which names the component) **and**
  bound on the agent type under `agents:`. A binding whose key has no matching
  top-level entry fails the *deploy* — before anything is uploaded — with
  `Discovered tool has no matching top-level declaration`. This is the bug that
  made the canary's tool look impossible to ship.
- **A mount is only served for an agent type listed in the HTTP API deployment.**
  Declaring a mount in the component is not enough; the agent type has to appear
  under `httpApi.deployments.<env>[].agents`. That is why `canary-release` is
  listed there, and why the dashboard is reachable at all.

```yaml
# Schema for IDEA / VSCode:
# $schema: https://schema.golem.cloud/app/golem/1.6.0/golem.schema.json

manifestVersion: 1.6.0

app: roc-golem

environments:
  local:
    server: local
  local16:
    server:
      url: "http://localhost:9996"
      allowInsecure: true
      auth:
        staticToken: "local"
  cloud:
    server: cloud

# The file-site agent publishes its mount on the local server's HTTP API
# listener, and the CLI expands `subdomain` to `roc-golem.localhost:<port>` using
# this port, so a server started with a different `--custom-request-port` will
# not be reachable under that domain (the CLI warns when they differ).
#
# `effects-agent` is listed too: Golem only lets agent types deployed through an
# HTTP API create webhooks, and its mount is what gives the webhook callback
# route a place to live. Its callback URLs come out under `/webhooks/` (the
# deployment's default `webhookUrl` prefix) followed by the agent type name.
#
# `canary-release` publishes the dashboard its own filesystem holds
# (`/canary/<id>/dashboard.html`), which is why it needs a deployment entry: a
# mount is only served for an agent type deployed through the HTTP API.
localServer:
  customRequestPort: 9882

httpApi:
  deployments:
    local:
      - subdomain: roc-golem
        agents:
          file-site-agent: {}
          static-site-agent: {}
          effects-agent: {}
          canary-release: {}

tools:
  # The tool surface (`golem:tool/guest`) exposes each app tool under its
  # CLI-shaped name; 1.6 requires every discovered tool to be declared here.
  get-count:
    component: roc-golem:counter
  calculator:
    component: roc-golem:ai-tool
  echo:
    component: roc-golem:ai-tool
  stream-message:
    component: roc-golem:streaming-agent
  analyze-logs:
    component: roc-golem:canary-release

# The quota resource the counter's `quota` command reserves from, and the retry
# policy its `retry` command resolves. Both are environment-level objects the CLI
# creates during deployment, so the names here are the names the agent uses in
# `newQuotaToken!` and sees in `getRetryPolicies!` (see the README's "Quotas and
# retry" section).
#
# The resource holds ten requests and rejects rather than waits, so one `quota 6`
# fits and the next one is answered with `quota reservation failed`; the capacity
# is shared by the whole environment, which is why the e2e section that exercises
# it expects a fresh data directory.
resourceDefaults:
  local:
    example-requests:
      limit:
        type: Capacity
        value: 10
      enforcementAction: reject
      unit: request
      units: requests

# The counter resolves a policy for `send` operations; this rule matches that
# verb and retries twice with a fixed 100ms delay in between.
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
            periodic:
              secs: 0
              nanos: 100000000

agents:
  # Bind each tool to the agent type that exposes it, so the tool protocol
  # (and `golem tool invoke`) can route calls.
  counter-agent:
    config:
      greeting: "hello from the manifest"
    tools:
      get-count: {}
    # One initial file, so the counter's `fs` commands can read a file the
    # manifest provisioned — the guest-side counterpart of the site example's
    # served files. `golem deploy` uploads it and Golem puts it in the agent's
    # filesystem before the agent starts.
    files:
      - sourcePath: examples/counter/seed/hello.txt
        targetPath: /seed/hello.txt
  ai-tool-agent:
    tools:
      calculator: {}
      echo: {}
  streaming-agent:
    tools:
      stream-message: {}
  # The mount serves these files straight out of the agent's durable
  # filesystem: `golem deploy` uploads them, Golem provisions them when the
  # agent starts, and the runtime answers matching GET/HEAD requests without
  # invoking the agent.
  file-site-agent:
    files:
      - sourcePath: examples/site/static/hello.txt
        targetPath: /assets/hello.txt
      - sourcePath: examples/site/static/index.html
        targetPath: /site/index.html
  # A router's static bindings are served from these uploads, not from an agent
  # filesystem: the runtime answers matching GET/HEAD requests with the
  # content-addressed blob and an immutable ETag, without creating the agent.
  static-site-agent:
    files:
      - sourcePath: examples/static_site/static/assets/hello.txt
        targetPath: /assets/hello.txt
      - sourcePath: examples/static_site/static/index.html
        targetPath: /index.html
  # The one agent type in this manifest with an explicit `initialCard`. The card
  # *replaces* the broad default card every other type here gets, so it is
  # deliberately tiny: read the file this manifest provisions for the type, and
  # connect to one network destination. Everything else — a write anywhere, a
  # connect to any other host — is the runtime's refusal, which the e2e asserts.
  #
  # `files:` is the same provisioning the counter's `fs` checks use. Golem also
  # appends `read`/`stat`/`list` grants for each provisioned file's own path to
  # the type's card (see the CLI's `resolve_initial_permission`), so the declared
  # `read : /seed/**` grant and the provisioned file cover each other.
  permissions-agent:
    initialCard:
      lowerBound:
        positive:
          - "filesystem(?agent) @ ?agent : read : /seed/**"
          - "network() @ ?agent : connect : api.openai.com:443"
          # The default card carries `card(?account) @ ?agent : * : *`, which is
          # what lets an app inspect its own cards. A declared card replaces it,
          # so without this grant `permissionInspect!` answers
          # `not-permitted: card:inspect is not permitted for this agent` — and
          # then nothing can show the card's own lattice.
          - "card(?account) @ ?agent : inspect : *"
      # The other quadrant the docs' card example shows: an upper-bound exclusion
      # keeps `billing.*` secrets out of reach of any card the agent acquires
      # later. Nothing in this type reveals a secret, so this one is visible in
      # the card's lattice rather than exercised by a probe.
      upperBound:
        negative:
          - "secret(?env) @ ?agent : reveal : billing.*"
    files:
      - sourcePath: examples/permissions/seed/allow.txt
        targetPath: /seed/allow.txt
  # The typed-method example. Its `Agent.define(...).register(...)` surface
  # declares four methods (`start_canary`, `approve_gate`,
  # `emergency_rollback`, and the read-only `get_status`) and one tool. `region`
  # is the local config it reads with `getConfig!`.
  canary-release:
    config:
      region: "local"
    tools:
      analyze-logs: {}

components:
  roc-golem:counter:
    dir: "examples/counter"
    componentWasm: "../../build/counter_agent.wasm"
    build:
      - command: "../../tooling/build.sh counter"
  roc-golem:ai-tool:
    dir: "examples/ai_tool"
    componentWasm: "../../build/ai_tool_agent.wasm"
    build:
      - command: "../../tooling/build.sh ai_tool"
  roc-golem:streaming-agent:
    dir: "examples/streaming_agent"
    componentWasm: "../../build/streaming_agent_agent.wasm"
    build:
      - command: "../../tooling/build.sh streaming_agent"
  roc-golem:effects:
    dir: "examples/effects"
    componentWasm: "../../build/effects_agent.wasm"
    build:
      - command: "../../tooling/build.sh effects"
  roc-golem:site:
    dir: "examples/site"
    componentWasm: "../../build/site_agent.wasm"
    build:
      - command: "../../tooling/build.sh site"
  roc-golem:static-site:
    dir: "examples/static_site"
    componentWasm: "../../build/static_site_agent.wasm"
    build:
      - command: "../../tooling/build.sh static_site"
  roc-golem:permissions:
    dir: "examples/permissions"
    componentWasm: "../../build/permissions_agent.wasm"
    build:
      - command: "../../tooling/build.sh permissions"
  roc-golem:canary-release:
    dir: "examples/canary_release"
    componentWasm: "../../build/canary_release_agent.wasm"
    build:
      - command: "../../tooling/build.sh canary_release"
```

### 2.7 What Golem sees when this deploys

The ABI harness runs the built component outside Golem, against stubbed host
imports, and prints the agent type the host reports. This is real output for the
component in §2.3–§2.5:

```text
[get-definition] type-name = "canary-release"
[get-definition] kind      = AgentTypeKind::Regular
[get-definition] methods   = ["handle-message", "analyze_logs", "start_canary", "approve_gate", "emergency_rollback", "get_status"]
[get-definition] http-mount = "canary/{id}" (static bindings: 0, filesystem bindings: 1, file response headers: 0)
[discover-agent-types]     = ["canary-release"]
[initialize]               = ok
[invoke analyze_logs] = "{\"faults\": 0}"
```

Four things to notice, because they explain behaviour that is otherwise
surprising:

1. **`handle-message` is always present.** Every agent type gets it. An app that
   declares no methods answers everything through `handle-message`.
2. **`analyze_logs` appears in the method list.** The host turns each published
   *tool* into a *method* of the same name (in the tool's own snake_case
   spelling), which is why a tool is reachable two ways: through the tool surface
   (`golem tool invoke`, whose name is the kebab-case `analyze-logs`) and through
   the agent method surface (`golem agent invoke ... analyze_logs`).
3. **The mount is one filesystem binding on `/canary/{id}`.** `{id}` captures the
   agent's constructor parameter, so the URL selects the instance.
4. **`kind` is `Regular`** — this is a durable agent with state, not an
   ephemeral one. See gap #1 in §1.2.

### 2.8 What the blueprint's version of this app would have added

Mapping the gemini blueprint's four files onto this one, so the delta is
concrete rather than rhetorical:

| Blueprint file | This application | What the blueprint had that this does not |
| --- | --- | --- |
| `Contracts.roc` | `Types.roc` | A `ServiceId` nominal type, a typed-config record carrying a `Secret(Str)`, a `Card` in the release plan, a streamed `ReleaseEvent`, and inline codec-derivation blocks. Here the state and wire types are structural and the codecs are derived by ability (§4.3); there is no secret handle and no card in the input. |
| `ReleaseCoordinatorAgent.roc` | `ReleaseCoordinator.roc` | Permission-card verification and attenuation, a durable *promise* gate awaiting a human, runtime reflection to find a notifier agent, a streamed progress method, and a live-file artifact written through a router. This version has the gate as an ordinary method plus a scheduled timer instead of a promise, and the dashboard as a per-agent mount instead of a shared router. |
| `DiagnosticsAgent.roc` | — (folded into the coordinator) | A *second* agent: an ephemeral, stateless diagnostics worker, and a tool with security middleware. Here the log analysis is a plain function inside the same agent (`countLogFaults`, §2.4), with no middleware and no separate stateless agent to call it. |
| `main.roc` | `main.roc` | Component assembly: registering two agents, a tool, and an artifact router with front-end auth. Here the file is four lines, because a Roc component holds exactly one agent type (see §3.1) and the mount travels in the agent's own metadata. |

The pattern is consistent with §1.1: the blueprint's *shape* — a durable
orchestrator, a typed tool, a stateful gate — is what got built; its *breadth* —
secrets, cards, promises, reflection, streams, a second agent type — is the
PARTIAL and ABSENT column.

---

## 3. The platform source

### 3.1 The Roc half: five files

A Roc *platform* is a package an app compiles against. This one is the “golem”
platform, and the app supplies exactly one thing: a value named `agent`. The
five files divide as follows.

| File | Lines | What it is |
| --- | --- | --- |
| `platform/main.roc` | 558 | The platform declaration itself. It states the app contract (`requires { agent : _ }`), the six entry points the **host calls into** (`provides`), the 83 host functions the **platform calls out to** (`hosted`), and the glue that turns the host's JSON payloads into the app's values and back. |
| `platform/Golem.roc` | 1128 | The app-facing surface — everything an app imports: `Agent`, `define`/`defineAgent`, `method`, `register`/`registerReadOnly`, `build`, the spec combinators (`withConfig`, `withMount`, `withTools`, `withToolHandler`), the effect wrappers (`getConfig!`, `writeFile!`, `rpc!`, …), and the re-exported types. |
| `platform/Types.roc` | 136 | The metadata model: agent metadata, tool declarations, method declarations, config declarations, the mount shape, the HTTP request/response shape, `ToolCall`/`ToolResult`. |
| `platform/Effect.roc` | 440 | The thin wrappers that call the host functions and adapt their argument shape (for example, turning a `LogLevel` tag into the `U8` the host expects). |
| `platform/Host.roc` | 85 | The opaque list of host-effect signatures — the ABI the Rust host implements. 83 functions, from `rocFxLog!` to `rocFxPermissionValidateGrant!`. |

Two of these are worth a sentence of orientation before reading:

- **`main.roc` is the only file that knows the host's calling convention.**
  `provides` names six symbols, each a function the Rust host calls: `init`,
  `metadata`, `handle-message`, `handle-request`, `handle-multimodal`, and
  `handle-tool-call`. Every one of them takes and returns `Str` (JSON text) or
  `Try(Str, Str)`, so the display between Roc's typed values and the host's
  untyped payload is entirely in this file.
- **`Types := {}.{ ... }` is how a file groups its types.** There is no separate
  “type module” syntax in use; a nominal type with an empty record body and an
  inline block holds the file's definitions, and the names inside become
  importable. `Host :: [].{ ... }` is the same trick for an opaque set of
  function signatures.

```roc
platform "golem"
	requires {
		agent : _
	}
	exposes [
		Golem,
		Types,
	]
	packages {}
	provides {
		"main_handle_message_for_host_1_exposed_generic": main_handle_message_for_host!,
		"main_handle_multimodal_for_host_1_exposed_generic": main_handle_multimodal_for_host!,
		"main_handle_request_for_host_1_exposed_generic": main_handle_request_for_host!,
		"main_handle_tool_call_for_host_1_exposed_generic": main_handle_tool_call_for_host!,
		"main_init_for_host_1_exposed_generic": main_init_for_host!,
		"main_metadata_for_host_1_exposed_generic": main_metadata_for_host,
	}
	hosted {
		"rocFxAppendFile": Host.rocFxAppendFile!,
		"rocFxAwaitWebhook": Host.rocFxAwaitWebhook!,
		"rocFxBlobCreateContainer": Host.rocFxBlobCreateContainer!,
		"rocFxBlobDelete": Host.rocFxBlobDelete!,
		"rocFxBlobGet": Host.rocFxBlobGet!,
		"rocFxBlobHas": Host.rocFxBlobHas!,
		"rocFxBlobList": Host.rocFxBlobList!,
		"rocFxBlobPut": Host.rocFxBlobPut!,
		"rocFxCreateWebhook": Host.rocFxCreateWebhook!,
		"rocFxDbBegin": Host.rocFxDbBegin!,
		"rocFxDbCommit": Host.rocFxDbCommit!,
		"rocFxDbExecute": Host.rocFxDbExecute!,
		"rocFxDbOpen": Host.rocFxDbOpen!,
		"rocFxDbOpenIgnite": Host.rocFxDbOpenIgnite!,
		"rocFxDbOpenMysql": Host.rocFxDbOpenMysql!,
		"rocFxDbQuery": Host.rocFxDbQuery!,
		"rocFxDbRollback": Host.rocFxDbRollback!,
		"rocFxDeleteFile": Host.rocFxDeleteFile!,
		"rocFxDurableStreamAppend": Host.rocFxDurableStreamAppend!,
		"rocFxDurableStreamRead": Host.rocFxDurableStreamRead!,
		"rocFxDurableStreamReader": Host.rocFxDurableStreamReader!,
		"rocFxDurableStreamRelease": Host.rocFxDurableStreamRelease!,
		"rocFxDurableStreamWriter": Host.rocFxDurableStreamWriter!,
		"rocFxFileExists": Host.rocFxFileExists!,
		"rocFxFileSize": Host.rocFxFileSize!,
		"rocFxFork": Host.rocFxFork!,
		"rocFxGetConfig": Host.rocFxGetConfig!,
		"rocFxGetPrincipal": Host.rocFxGetPrincipal!,
		"rocFxGetRetryPolicies": Host.rocFxGetRetryPolicies!,
		"rocFxGetRetryPolicy": Host.rocFxGetRetryPolicy!,
		"rocFxGetWorkerId": Host.rocFxGetWorkerId!,
		"rocFxHttpRequest": Host.rocFxHttpRequest!,
		"rocFxIdempotencyKey": Host.rocFxIdempotencyKey!,
		"rocFxKvDelete": Host.rocFxKvDelete!,
		"rocFxKvExists": Host.rocFxKvExists!,
		"rocFxKvGet": Host.rocFxKvGet!,
		"rocFxKvSet": Host.rocFxKvSet!,
		"rocFxListDir": Host.rocFxListDir!,
		"rocFxLog": Host.rocFxLog!,
		"rocFxMakeAgentId": Host.rocFxMakeAgentId!,
		"rocFxMakeDir": Host.rocFxMakeDir!,
		"rocFxNewQuotaToken": Host.rocFxNewQuotaToken!,
		"rocFxNextRetryDelay": Host.rocFxNextRetryDelay!,
		"rocFxNowMillis": Host.rocFxNowMillis!,
		"rocFxOplogCommit": Host.rocFxOplogCommit!,
		"rocFxParseAgentId": Host.rocFxParseAgentId!,
		"rocFxPermissionDerive": Host.rocFxPermissionDerive!,
		"rocFxPermissionDeriveFromWallet": Host.rocFxPermissionDeriveFromWallet!,
		"rocFxPermissionDeriveScope": Host.rocFxPermissionDeriveScope!,
		"rocFxPermissionExpiresAt": Host.rocFxPermissionExpiresAt!,
		"rocFxPermissionId": Host.rocFxPermissionId!,
		"rocFxPermissionInspect": Host.rocFxPermissionInspect!,
		"rocFxPermissionInstall": Host.rocFxPermissionInstall!,
		"rocFxPermissionListModules": Host.rocFxPermissionListModules!,
		"rocFxPermissionParents": Host.rocFxPermissionParents!,
		"rocFxPermissionRelease": Host.rocFxPermissionRelease!,
		"rocFxPermissionRevoke": Host.rocFxPermissionRevoke!,
		"rocFxPermissionValidateGrant": Host.rocFxPermissionValidateGrant!,
		"rocFxPermissionsWallet": Host.rocFxPermissionsWallet!,
		"rocFxPermissionsWalletVersion": Host.rocFxPermissionsWalletVersion!,
		"rocFxQuotaCommit": Host.rocFxQuotaCommit!,
		"rocFxQuotaMerge": Host.rocFxQuotaMerge!,
		"rocFxQuotaReserve": Host.rocFxQuotaReserve!,
		"rocFxQuotaSplit": Host.rocFxQuotaSplit!,
		"rocFxReadFile": Host.rocFxReadFile!,
		"rocFxRemoveRetryPolicy": Host.rocFxRemoveRetryPolicy!,
		"rocFxResolveAgentId": Host.rocFxResolveAgentId!,
		"rocFxResolveComponentId": Host.rocFxResolveComponentId!,
		"rocFxResolveRetryPolicy": Host.rocFxResolveRetryPolicy!,
		"rocFxRpcCancel": Host.rocFxRpcCancel!,
		"rocFxRpcFireAndForget": Host.rocFxRpcFireAndForget!,
		"rocFxRpcInvoke": Host.rocFxRpcInvoke!,
		"rocFxRpcSchedule": Host.rocFxRpcSchedule!,
		"rocFxRpcScheduleCancelable": Host.rocFxRpcScheduleCancelable!,
		"rocFxSetPersistence": Host.rocFxSetPersistence!,
		"rocFxSetRetryPolicy": Host.rocFxSetRetryPolicy!,
		"rocFxSleepMillis": Host.rocFxSleepMillis!,
		"rocFxTrap": Host.rocFxTrap!,
		"rocFxWriteFile": Host.rocFxWriteFile!,
		"rocFxWsClose": Host.rocFxWsClose!,
		"rocFxWsConnect": Host.rocFxWsConnect!,
		"rocFxWsReceive": Host.rocFxWsReceive!,
		"rocFxWsSend": Host.rocFxWsSend!,
	}
	targets: {
		wasm32: {
			inputs: ["libhost.a", app],
			exports: ["[async-lift]golem:agent/guest@2.0.0#initialize", "[callback][async-lift]golem:agent/guest@2.0.0#initialize", "[async-lift]golem:agent/guest@2.0.0#invoke", "[callback][async-lift]golem:agent/guest@2.0.0#invoke", "golem:agent/guest@2.0.0#get-definition", "cabi_post_golem:agent/guest@2.0.0#get-definition", "golem:agent/guest@2.0.0#discover-agent-types", "cabi_post_golem:agent/guest@2.0.0#discover-agent-types", "[async-lift]golem:tool/guest@0.1.0#invoke", "[callback][async-lift]golem:tool/guest@0.1.0#invoke", "golem:tool/guest@0.1.0#discover-tools", "cabi_post_golem:tool/guest@0.1.0#discover-tools", "golem:tool/guest@0.1.0#get-tool", "cabi_post_golem:tool/guest@0.1.0#get-tool", "[async-lift]golem:api/save-snapshot@1.5.0#save", "[callback][async-lift]golem:api/save-snapshot@1.5.0#save", "[async-lift]golem:api/load-snapshot@1.5.0#load", "[callback][async-lift]golem:api/load-snapshot@1.5.0#load", "cabi_realloc"],
		},
	}

import Types exposing [
	AgentKind,
	ConfigDeclaration,
	FileBinding,
	FileResponseHeader,
	HttpHandlerOption,
	HttpHeader,
	HttpMountOption,
	HttpResponse,
	MethodDeclaration,
	MultimodalOption,
	OpenApiProvider,
	ToolCall,
	ToolDefinition,
	ToolParameter,
]
import Golem
import Host

main_init_for_host! : Str => Try(Str, Str)
main_init_for_host! = |config_str| {
	initial_model = (agent.init!)(config_str)
	Ok((agent.serializeState)(initial_model))
}

main_handle_message_for_host! : Str, Str => Try(Str, Str)
main_handle_message_for_host! = |state_json, message_str| {
	match (agent.deserializeState)(state_json) {
		Err(err) =>
			Err("Failed to deserialize state: ${err}")

		Ok(current_model) => {
			res = (agent.handleMessage!)(current_model, message_str)
			next_state_json = (agent.serializeState)(res.state)

			# We'll just take the first reply for now, or join them
			joined_replies = Str.join_with(res.replies, "\n")
			escaped_response = escape_json_str(joined_replies)
			Ok("{\"state\":${next_state_json},\"response\":\"${escaped_response}\"}")
		}
	}
}

main_handle_tool_call_for_host! : Str, Str => Try(Str, Str)
main_handle_tool_call_for_host! = |state_json, tool_call_json| {
	match (agent.deserializeState)(state_json) {
		Err(err) =>
			Err("Failed to deserialize state: ${err}")

		Ok(current_model) => {
			parsed_call = parse_tool_call(tool_call_json)
			res = (agent.handleToolCall!)(current_model, parsed_call)
			next_state_json = (agent.serializeState)(res.state)
			success_str = match res.result.success {
				True => "true"
				False => "false"
			}
			escaped_output = escape_json_str(res.result.output)
			# The tool's stdout/stderr travel next to the payload: the host writes
			# them through the streams the caller attached (`golem tool invoke
			# --stdout` / `--stderr`).
			escaped_stdout = escape_json_str(res.result.stdout)
			escaped_stderr = escape_json_str(res.result.stderr)
			Ok("{\"state\":${next_state_json},\"success\":${success_str},\"output\":\"${escaped_output}\",\"stdout\":\"${escaped_stdout}\",\"stderr\":\"${escaped_stderr}\"}")
		}
	}
}

main_handle_multimodal_for_host! : Str, Str => Try(Str, Str)
main_handle_multimodal_for_host! = |state_json, parts_json| {
	match (agent.deserializeState)(state_json) {
		Err(err) =>
			Err("Failed to deserialize state: ${err}")

		Ok(current_model) => {
			res = (agent.handleMultimodal!)(current_model, parts_json)
			next_state_json = (agent.serializeState)(res.state)
			Ok("{\"state\":${next_state_json},\"parts\":${format_json_string(res.parts)}}")
		}
	}
}

main_handle_request_for_host! : Str, Str => Try(Str, Str)
main_handle_request_for_host! = |state_json, request_json| {
	match (agent.deserializeState)(state_json) {
		Err(err) =>
			Err("Failed to deserialize state: ${err}")

		Ok(current_model) => {
			request = parse_http_request(request_json)
			res = (agent.handleRequest!)(current_model, request)
			next_state_json = (agent.serializeState)(res.state)
			headers_json = format_http_headers(res.response.headers)
			body_escaped = escape_json_str(res.response.body)
			Ok("{\"state\":${next_state_json},\"status\":${U16.to_str(res.response.status)},\"headers\":[${headers_json}],\"body\":\"${body_escaped}\"}")
		}
	}
}

# The host builds this from the request Golem handed it; the header list stays a
# string so the app can read it verbatim (or ignore it). The built-in `Json`
# codec parses the flat object of JSON strings: every field is optional
# (`Try(Str, [Missing])`), so an absent field — or a payload that is not that
# object at all — reads as `""`, the same fallback these callers have always used.
parse_http_request : Str -> Types.HttpRequest
parse_http_request = |raw_json| {
	parsed : Try(
		{
			method : Try(Str, [Missing]),
			scheme : Try(Str, [Missing]),
			authority : Try(Str, [Missing]),
			path : Try(Str, [Missing]),
			query : Try(Str, [Missing]),
			headersJson : Try(Str, [Missing]),
			body : Try(Str, [Missing]),
		},
		[InvalidJson(Str)],
	)
	parsed = Json.parse(raw_json)
	match parsed {
		Err(_) => {
			method: "",
			scheme: "",
			authority: "",
			path: "",
			query: "",
			headersJson: "",
			body: "",
		}
		Ok(fields) => {
			method: fields.method ?? "",
			scheme: fields.scheme ?? "",
			authority: fields.authority ?? "",
			path: fields.path ?? "",
			query: fields.query ?? "",
			headersJson: fields.headersJson ?? "",
			body: fields.body ?? "",
		}
	}
}

format_http_headers : List(HttpHeader) -> Str
format_http_headers = |headers| {
	formatted = List.map(headers, format_http_header)
	Str.join_with(formatted, ",")
}

format_http_header : HttpHeader -> Str
format_http_header = |header| "{\"name\":${format_json_string(header.name)},\"value\":${format_json_string(header.value)}}"

main_metadata_for_host : {} => Str
main_metadata_for_host = |_| {
	meta = agent.metadata

	formatted_tools = List.map(meta.tools, format_tool_definition)
	tools_json = Str.join_with(formatted_tools, ",")

	formatted_methods = List.map(meta.methods, format_method_declaration)
	methods_json = Str.join_with(formatted_methods, ",")

	name_escaped = escape_json_str(meta.name)
	version_escaped = escape_json_str(meta.version)
	desc_escaped = escape_json_str(meta.description)

	formatted_config = List.map(meta.config, format_config_declaration)
	config_json = Str.join_with(formatted_config, ",")

	mount_json = format_http_mount(meta.httpMount)
	kind_json = format_agent_kind(meta.agentKind)
	handler_json = format_http_handler(meta.httpHandler)
	multimodal_json = format_multimodal(meta.multimodal)
	openapi_json = format_openapi_provider(meta.openapiProvider)

	"{\"name\":\"${name_escaped}\",\"version\":\"${version_escaped}\",\"description\":\"${desc_escaped}\",\"tools\":[${tools_json}],\"methods\":[${methods_json}],\"config\":[${config_json}],\"agentKind\":${kind_json},\"httpMount\":${mount_json},\"httpHandler\":${handler_json},\"multimodal\":${multimodal_json},\"openapiProvider\":${openapi_json}}"
}

# One declared method's caller-visible fields, for the host to register as a
# Golem method (see `Types.MethodDeclaration`).
format_method_declaration : MethodDeclaration -> Str
format_method_declaration = |decl| {
	read_only_str = match decl.readOnly {
		True => "true"
		False => "false"
	}
	"{\"name\":${format_json_string(decl.name)},\"description\":${format_json_string(decl.description)},\"promptHint\":${format_json_string(decl.promptHint)},\"readOnly\":${read_only_str}}"
}

format_multimodal : MultimodalOption -> Str
format_multimodal = |multimodal|
	match multimodal {
		NoMultimodal => "null"
		HandlesMultimodal => "\"multimodal\""
	}

format_http_handler : HttpHandlerOption -> Str
format_http_handler = |handler|
	match handler {
		NoHandler => "null"
		HandlesRequests => "\"handler\""
	}

format_agent_kind : AgentKind -> Str
format_agent_kind = |kind|
	match kind {
		Regular => "\"regular\""
		HttpRouter => "\"http-router\""
	}

format_openapi_provider : OpenApiProvider -> Str
format_openapi_provider = |provider|
	match provider {
		NoOpenApi => "null"
		OpenApi(document) => format_json_string(document)
	}

format_http_mount : HttpMountOption -> Str
format_http_mount = |mount|
	match mount {
		None => "null"

		Mount(m) => {
			prefix_json = format_string_list(m.pathPrefix)
			static_json = format_file_bindings(m.staticBindings)
			bindings_json = format_file_bindings(m.filesystemBindings)
			headers_json = format_file_response_headers(m.fileResponseHeaders)
			cors_json = format_string_list(m.corsAllowedPatterns)
			"{\"pathPrefix\":${prefix_json},\"staticBindings\":${static_json},\"filesystemBindings\":${bindings_json},\"fileResponseHeaders\":${headers_json},\"corsAllowedPatterns\":${cors_json}}"
		}
	}

format_string_list : List(Str) -> Str
format_string_list = |items| {
	escaped = List.map(items, format_json_string)
	"[${Str.join_with(escaped, ",")}]"
}

format_json_string : Str -> Str
format_json_string = |item| "\"${escape_json_str(item)}\""

format_file_bindings : List(FileBinding) -> Str
format_file_bindings = |bindings| {
	formatted = List.map(bindings, format_file_binding)
	"[${Str.join_with(formatted, ",")}]"
}

format_file_binding : FileBinding -> Str
format_file_binding = |binding|
	match binding {
		Exact({ publicPath, filePath }) => "{\"exact\":{\"publicPath\":${format_string_list(publicPath)},\"filePath\":${format_json_string(filePath)}}}"

		Subtree({ publicPrefix, filesystemRoot }) => "{\"subtree\":{\"publicPrefix\":${format_string_list(publicPrefix)},\"filesystemRoot\":${format_json_string(filesystemRoot)}}}"
	}

format_file_response_headers : List(FileResponseHeader) -> Str
format_file_response_headers = |headers| {
	formatted = List.map(headers, format_file_response_header)
	"[${Str.join_with(formatted, ",")}]"
}

format_file_response_header : FileResponseHeader -> Str
format_file_response_header = |header| "{\"name\":${format_json_string(header.name)},\"value\":${format_json_string(header.value)}}"

format_config_declaration : ConfigDeclaration -> Str
format_config_declaration = |decl| {
	name_escaped = escape_json_str(decl.name)
	secret_str = match decl.secret {
		True => "true"
		False => "false"
	}
	"{\"name\":\"${name_escaped}\",\"secret\":${secret_str}}"
}

format_tool_definition : ToolDefinition -> Str
format_tool_definition = |tool| {
	formatted_params = List.map(tool.parameters, format_tool_parameter)
	params_json = Str.join_with(formatted_params, ",")

	name_escaped = escape_json_str(tool.name)
	desc_escaped = escape_json_str(tool.description)
	read_only_str = match tool.readOnly {
		True => "true"
		False => "false"
	}
	"{\"name\":\"${name_escaped}\",\"description\":\"${desc_escaped}\",\"parameters\":[${params_json}],\"readOnly\":${read_only_str}}"
}

format_tool_parameter : ToolParameter -> Str
format_tool_parameter = |param| {
	req_str = match param.required {
		True => "true"
		False => "false"
	}
	name_escaped = escape_json_str(param.name)
	desc_escaped = escape_json_str(param.description)
	type_escaped = escape_json_str(param.paramType)
	"{\"name\":\"${name_escaped}\",\"description\":\"${desc_escaped}\",\"type\":\"${type_escaped}\",\"required\":${req_str}}"
}

# The control characters below 0x20 with no short escape, as `(byte, escape)`
# pairs. `\n`, `\r` and `\t` are replaced directly above; `\b` and `\f` are here
# as their `\u` forms so that no control character has to appear in this source
# file. A byte 0x80 or above is never escaped: it is part of a UTF-8 sequence,
# which JSON carries as-is.
control_escapes : List((U8, Str))
control_escapes = [
	(0, "\\u0000"),
	(1, "\\u0001"),
	(2, "\\u0002"),
	(3, "\\u0003"),
	(4, "\\u0004"),
	(5, "\\u0005"),
	(6, "\\u0006"),
	(7, "\\u0007"),
	(8, "\\u0008"),
	(11, "\\u000b"),
	(12, "\\u000c"),
	(14, "\\u000e"),
	(15, "\\u000f"),
	(16, "\\u0010"),
	(17, "\\u0011"),
	(18, "\\u0012"),
	(19, "\\u0013"),
	(20, "\\u0014"),
	(21, "\\u0015"),
	(22, "\\u0016"),
	(23, "\\u0017"),
	(24, "\\u0018"),
	(25, "\\u0019"),
	(26, "\\u001a"),
	(27, "\\u001b"),
	(28, "\\u001c"),
	(29, "\\u001d"),
	(30, "\\u001e"),
	(31, "\\u001f"),
]

# Escapes a string for the JSON this platform hands the host. Every control
# character below 0x20 has to be escaped: the host parses these payloads with
# `serde_json`, which refuses a raw one, so an answer that carried one — a reply,
# a response body, a tool's stdout — was rejected as malformed JSON and the
# invocation's output was dropped. The built-in `Json` codec writes the same
# `\u00XX` form, so what this writes is what `Json.parse` reads back.
escape_json_str : Str -> Str
escape_json_str = |input| {
	s1 = Str.replace_each(input, "\\", "\\\\")
	s2 = Str.replace_each(s1, "\"", "\\\"")
	s3 = Str.replace_each(s2, "\n", "\\n")
	s4 = Str.replace_each(s3, "\r", "\\r")
	s5 = Str.replace_each(s4, "\t", "\\t")
	apply_escapes(s5, control_escapes)
}

# Applies the `(byte, escape)` pairs of `control_escapes` in turn.
apply_escapes : Str, List((U8, Str)) -> Str
apply_escapes = |text, pairs|
	apply_escapes_at(text, pairs, 0)

apply_escapes_at : Str, List((U8, Str)), U64 -> Str
apply_escapes_at = |text, pairs, index|
	match List.get(pairs, index) {
		Err(_) => text
		Ok((byte, escape)) =>
			apply_escapes_at(Str.replace_each(text, Str.from_utf8_lossy([byte]), escape), pairs, index + 1)
	}

# The emitter and the reader agree: what `escape_json_str` writes for a control
# character is what `Json.parse` reads back.
vertical_tab = Str.from_utf8_lossy([11])

backspace = Str.from_utf8_lossy([8])

expect escape_json_str(vertical_tab) == "\\u000b"

expect escape_json_str(backspace) == "\\u0008"

expect {
	round_trip : Try({ a : Str }, [InvalidJson(Str), MissingRequiredField(Str)])
	round_trip = Json.parse("{\"a\":\"${escape_json_str(vertical_tab)}\"}")
	round_trip == Ok({ a: vertical_tab })
}

expect {
	round_trip : Try({ a : Str }, [InvalidJson(Str), MissingRequiredField(Str)])
	round_trip = Json.parse("{\"a\":\"a${escape_json_str(backspace)}b\"}")
	round_trip == Ok({ a: Str.concat("a", Str.concat(backspace, "b")) })
}

# A UTF-8 sequence is carried as-is, not escaped byte by byte.
expect escape_json_str("é") == "é"

# The short escapes are what they were.
expect escape_json_str("a\"b\\c\nd\re\tf") == "a\\\"b\\\\c\\nd\\re\\tf"

# The host writes the tool call as a flat object of JSON strings. The built-in
# `Json` codec parses it; every field is optional, so an absent one — or a
# payload that is not that object — takes the same default as before.
parse_tool_call : Str -> ToolCall
parse_tool_call = |raw_json| {
	call : Try(
		{
			id : Try(Str, [Missing]),
			name : Try(Str, [Missing]),
			arguments : Try(Str, [Missing]),
			stdin : Try(Str, [Missing]),
		},
		[InvalidJson(Str)],
	)
	call = Json.parse(raw_json)
	match call {
		Err(_) => { id: "call-1", name: "default", arguments: "{}", stdin: "" }
		Ok(fields) => {
			id = fields.id ?? "call-1"
			name = fields.name ?? "default"
			arguments = fields.arguments ?? "{}"
			stdin = fields.stdin ?? ""
			{ id, name, arguments, stdin }
		}
	}
}

# The reader is the built-in `Json` codec. These pin the mapping from the host's
# payloads onto `parse_tool_call` / `parse_http_request`: a present field is
# decoded (including the escapes the host's emitter writes), an absent one takes
# the default, and a value that contains another field's text is still read as a
# value — `Json.parse` reads member names as names.

expect parse_tool_call("{\"id\":\"c1\",\"name\":\"echo\",\"arguments\":\"{}\",\"stdin\":\"s\"}") == { id: "c1", name: "echo", arguments: "{}", stdin: "s" }

expect parse_tool_call("{}") == { id: "call-1", name: "default", arguments: "{}", stdin: "" }

expect parse_tool_call("not json") == { id: "call-1", name: "default", arguments: "{}", stdin: "" }

# A value carrying the text of a later field is not mistaken for that field.
expect parse_tool_call("{\"name\":\"decoy \\\"arguments\\\":\\\"FAKE\\\"\",\"arguments\":\"real\"}") == { id: "call-1", name: "decoy \"arguments\":\"FAKE\"", arguments: "real", stdin: "" }

# The escapes the host writes are decoded.
expect parse_tool_call("{\"name\":\"he said \\\"hi\\\"\",\"stdin\":\"line1\\nline2\"}") == { id: "call-1", name: "he said \"hi\"", arguments: "{}", stdin: "line1\nline2" }

expect parse_tool_call("{\"stdin\":\"caf\\u00e9 \\ud83d\\ude00\"}") == { id: "call-1", name: "default", arguments: "{}", stdin: "café 😀" }

# A control character survives the emitter/reader round trip.
expect parse_tool_call("{\"stdin\":\"${escape_json_str(vertical_tab)}\"}") == { id: "call-1", name: "default", arguments: "{}", stdin: vertical_tab }

expect parse_http_request("{\"method\":\"GET\",\"scheme\":\"http\",\"authority\":\"h\",\"path\":\"/p\",\"query\":\"q=1\",\"headersJson\":\"[]\",\"body\":\"b\"}") == { method: "GET", scheme: "http", authority: "h", path: "/p", query: "q=1", headersJson: "[]", body: "b" }

# Only the fields the host sent are set; the rest take their default.
expect parse_http_request("{\"method\":\"GET\"}") == { method: "GET", scheme: "", authority: "", path: "", query: "", headersJson: "", body: "" }

expect parse_http_request("{}") == { method: "", scheme: "", authority: "", path: "", query: "", headersJson: "", body: "" }
```

```roc
# This module is imported by applications (`import pf.Golem`), so it still uses
# the legacy module-header syntax: in the pinned nightly (2026-10-19-601b54c)
# the new type-module format crashes the compiler when an application imports
# the module's type through the platform (`exposes`). The internal modules
# (Types, Effect) have been migrated; revisit Golem once the upstream compiler
# bug is fixed. The matching CI tolerance for the deprecation warning lives in
# .github/workflows/ci.yaml.
module [
	Agent,
	defineAgent,
	Method,
	method,
	Spec,
	define,
	withTools,
	withToolHandler,
	withConfig,
	withMount,
	register,
	registerReadOnly,
	build,
	logTrace!,
	logDebug!,
	logInfo!,
	logWarn!,
	logError!,
	workerId!,
	createWebhook!,
	awaitWebhook!,
	rpc!,
	rpcFireAndForget!,
	rpcSchedule!,
	rpcScheduleCancelable!,
	rpcCancel!,
	parseAgentId!,
	makeAgentId!,
	setPersistence!,
	http!,
	websocketConnect!,
	websocketSend!,
	websocketReceive!,
	websocketClose!,
	sleep!,
	now!,
	getConfig!,
	newQuotaToken!,
	quotaReserve!,
	quotaCommit!,
	quotaSplit!,
	quotaMerge!,
	getRetryPolicies!,
	getRetryPolicy!,
	resolveRetryPolicy!,
	nextRetryDelay!,
	setRetryPolicy!,
	removeRetryPolicy!,
	trap!,
	idempotencyKey!,
	fork!,
	oplogCommit!,
	resolveAgentId!,
	resolveComponentId!,
	kvGet!,
	kvSet!,
	kvDelete!,
	kvExists!,
	blobCreateContainer!,
	blobPut!,
	blobGet!,
	blobHas!,
	blobList!,
	blobDelete!,
	readFile!,
	writeFile!,
	appendFile!,
	fileExists!,
	fileSize!,
	deleteFile!,
	makeDir!,
	listDir!,
	durableStreamReader!,
	durableStreamRead!,
	durableStreamWriter!,
	durableStreamAppend!,
	durableStreamRelease!,
	dbOpen!,
	dbOpenMysql!,
	dbOpenIgnite!,
	dbQuery!,
	dbExecute!,
	dbBegin!,
	dbCommit!,
	dbRollback!,
	getPrincipal!,
	permissionsWallet!,
	permissionsWalletVersion!,
	permissionId!,
	permissionParents!,
	permissionExpiresAt!,
	permissionInspect!,
	permissionDerive!,
	permissionDeriveFromWallet!,
	permissionDeriveScope!,
	permissionInstall!,
	permissionRevoke!,
	permissionRelease!,
	permissionListModules!,
	permissionValidateGrant!,
]

import Effect
import Types exposing [ConfigDeclaration, HttpMountOption, LogLevel, MethodDeclaration, PersistenceMode, ToolDefinition, ToolCall, ToolResult, Metadata, HttpRequest, HttpResponse]

Agent(state) : {
	init! : Str => state,
	handleMessage! : state, Str => { state : state, replies : List(Str) },
	# A router's request handler. Only called when the app declares
	# `httpHandler: HandlesRequests`; every other agent type can answer with a
	# plain `Err`-style response.
	handleRequest! : state, HttpRequest => { state : state, response : HttpResponse },
	# The multimodal method's handler. Only called when the app declares
	# `multimodal: HandlesMultimodal`; every other agent type can answer with the
	# same kind of envelope it was handed. `parts` is the JSON envelope of the
	# caller's parts — `[{"tag":"text","value":{"text":…,"language":…}},
	# {"tag":"binary","value":{"bytes":[…],"mimeType":…}}]` — and the answer is
	# the same envelope, which is what the caller's parts become (see the README's
	# "Multimodal values").
	handleMultimodal! : state, Str => { state : state, parts : Str },
	handleToolCall! : state, ToolCall => { state : state, result : ToolResult },
	metadata : Metadata,
	serializeState : state -> Str,
	deserializeState : Str -> Try(state, Str),
}

defineAgent : Agent(state) -> Agent(state)
defineAgent = |agent|
	agent

logTrace! : Str => {}
logTrace! = |msg|
	Effect.log!(Trace, msg)

logDebug! : Str => {}
logDebug! = |msg|
	Effect.log!(Debug, msg)

logInfo! : Str => {}
logInfo! = |msg|
	Effect.log!(Info, msg)

logWarn! : Str => {}
logWarn! = |msg|
	Effect.log!(Warn, msg)

logError! : Str => {}
logError! = |msg|
	Effect.log!(Error, msg)

workerId! : {} => Str
workerId! = |_|
	Effect.getWorkerId!({})

## Creates a webhook for this agent and returns its callback URL: a POST to the
## URL completes the promise this effect created. The URL is the handle the app
## stores in its own state; `awaitWebhook!` resolves it back to the promise.
createWebhook! : {} => Try(Str, Str)
createWebhook! = |_|
	Effect.createWebhook!({})

## Waits for the POST to `url` (from `createWebhook!`) and returns its body as
## text.
awaitWebhook! : Str => Try(Str, Str)
awaitWebhook! = |url|
	Effect.awaitWebhook!(url)

rpc! : Str, Str, Str => Try(Str, Str)
rpc! = |targetWorker, fnName, payload|
	Effect.rpcInvoke!(targetWorker, fnName, payload)

## Triggers a remote method and returns its invocation identity right away
## (`{"agentId": …, "idempotencyKey": …}`) instead of waiting for the reply.
rpcFireAndForget! : Str, Str, Str => Try(Str, Str)
rpcFireAndForget! = |targetWorker, fnName, payload|
	Effect.rpcFireAndForget!(targetWorker, fnName, payload)

## Schedules a remote method for `delayMillis` from now and returns the
## scheduled invocation's identity, in the same shape as `rpcFireAndForget!`.
rpcSchedule! : Str, Str, Str, U64 => Try(Str, Str)
rpcSchedule! = |targetWorker, fnName, payload, delayMillis|
	Effect.rpcSchedule!(targetWorker, fnName, payload, delayMillis)

## The same as `rpcSchedule!`, but returns a handle for `rpcCancel!`, which
## cancels the invocation as long as it has not run yet.
rpcScheduleCancelable! : Str, Str, Str, U64 => Try(U32, Str)
rpcScheduleCancelable! = |targetWorker, fnName, payload, delayMillis|
	Effect.rpcScheduleCancelable!(targetWorker, fnName, payload, delayMillis)

## Cancels a scheduled invocation that has not run yet. The handle comes from
## `rpcScheduleCancelable!` and can be used once.
rpcCancel! : U32 => Try({}, Str)
rpcCancel! = |handle|
	Effect.rpcCancel!(handle)

## Splits an agent id into `{"agentType": …, "constructor": …,
## "phantomId": …}` — the same resolution `rpc!` does on its target.
parseAgentId! : Str => Try(Str, Str)
parseAgentId! = |agentId|
	Effect.parseAgentId!(agentId)

## Builds an agent id from an agent type name and its constructor parameter,
## the inverse of `parseAgentId!`.
makeAgentId! : Str, Str => Try(Str, Str)
makeAgentId! = |agentTypeName, constructorParam|
	Effect.makeAgentId!(agentTypeName, constructorParam)

setPersistence! : PersistenceMode => {}
setPersistence! = |mode|
	Effect.setPersistenceMode!(mode)

http! : Str => Try(Str, Str)
http! = |reqJson|
	Effect.httpRequest!(reqJson)

websocketConnect! : Str => Try(U32, Str)
websocketConnect! = |url|
	Effect.wsConnect!(url)

websocketSend! : U32, Str => Try({}, Str)
websocketSend! = |handle, msg|
	Effect.wsSend!(handle, msg)

websocketReceive! : U32 => Try(Str, Str)
websocketReceive! = |handle|
	Effect.wsReceive!(handle)

websocketClose! : U32 => Try({}, Str)
websocketClose! = |handle|
	Effect.wsClose!(handle)

sleep! : U64 => {}
sleep! = |millis|
	Effect.sleepMillis!(millis)

now! : {} => U64
now! = |_|
	Effect.nowMillis!({})

getConfig! : Str => Try(Str, Str)
getConfig! = |key|
	Effect.getConfig!(key)

## Asks the host for a quota token on the resource `resourceName`, which the
## manifest declares under `resourceDefaults.<environment>.<resourceName>`.
## `expectedUse` is the amount this token's reservations are sized for: it tells
## the host how much credit to schedule, and consumes nothing by itself. The
## token lives host-side; the number returned is the handle the other quota
## effects work with.
newQuotaToken! : Str, U64 => U32
newQuotaToken! = |resourceName, expectedUse|
	Effect.newQuotaToken!(resourceName, expectedUse)

## Reserves `amount` from the token's allocation. `Ok` is the handle of a
## reservation for `quotaCommit!`; `Err` is the rejection a `reject`ing resource
## answered with, including its estimate of the wait for capacity when it has
## one. A `throttle`ing resource waits here instead, and a `terminate`ing one
## fails the invocation.
quotaReserve! : U32, U64 => Try(U32, Str)
quotaReserve! = |token, amount|
	Effect.quotaReserve!(token, amount)

## Commits what the reserved work actually used: less than was reserved returns
## the unused capacity, more charges the excess to the token as debt, and a
## reservation that is never committed counts as zero. A handle is spent once.
quotaCommit! : U32, U64 => Try({}, Str)
quotaCommit! = |reservation, used|
	Effect.quotaCommit!(reservation, used)

## Splits `childExpectedUse` off the token into a second one. Both stay in this
## agent instance — quota tokens cannot travel between agents in this platform.
quotaSplit! : U32, U64 => Try(U32, Str)
quotaSplit! = |token, childExpectedUse|
	Effect.quotaSplit!(token, childExpectedUse)

## Merges `other` back into the token, consuming it. The two tokens have to name
## the same resource.
quotaMerge! : U32, U32 => Try({}, Str)
quotaMerge! = |token, other|
	Effect.quotaMerge!(token, other)

## Every retry policy active for this agent, as a JSON array of named policies
## (`[{"name": …, "priority": …, "predicate": {"nodes": […]},
## "policy": {"nodes": […]}}, …]`). The trees are the host's flattened form —
## the root is `nodes[0]` and children are indices into the same list — with
## durations as nanosecond counts; the README's "Quotas and retry" section spells
## the shape out.
getRetryPolicies! : {} => Str
getRetryPolicies! = |_|
	Effect.getRetryPolicies!({})

## One policy by name, the same JSON as above — or `""` when no policy has that
## name.
getRetryPolicy! : Str => Try(Str, Str)
getRetryPolicy! = |name|
	Effect.getRetryPolicy!(name)

## The first policy whose predicate matches `verb` / `nounUri` with the given
## properties (a JSON object of strings, integers and booleans, `{}` for none),
## as policy JSON — or `""` when none matches. A predicate the host cannot
## evaluate comes back as `Err`, without failing the invocation.
resolveRetryPolicy! : Str, Str, Str => Try(Str, Str)
resolveRetryPolicy! = |verb, nounUri, propertiesJson|
	Effect.resolveRetryPolicy!(verb, nounUri, propertiesJson)

## Advances the host's retry evaluator once, after a failed attempt: `policyJson`
## is a policy from above (`""` takes the agent's configured default),
## `stateJson` the `state` the previous decision returned (`[]` starts a
## sequence), `elapsedNanos` the time since the first attempt, and `contextJson`
## `{"verb": …, "nounUri": …, "properties": {…}}`. The answer is
## `{"state": […], "delayNanos": …}`, with `null` for a delay when the policy is
## exhausted (a zero delay still allows another attempt).
nextRetryDelay! : Str, Str, U64, Str => Try(Str, Str)
nextRetryDelay! = |policyJson, stateJson, elapsedNanos, contextJson|
	Effect.nextRetryDelay!(policyJson, stateJson, elapsedNanos, contextJson)

## Adds or overwrites a named policy, taking the JSON `getRetryPolicies!`
## prints. The host records it in the agent's oplog, so it outlives a restart.
setRetryPolicy! : Str => Try({}, Str)
setRetryPolicy! = |policyJson|
	Effect.setRetryPolicy!(policyJson)

## Removes a named policy (also recorded in the oplog). A policy the manifest
## declared comes back on the next deployment; this removes the agent's own copy.
removeRetryPolicy! : Str => {}
removeRetryPolicy! = |name|
	Effect.removeRetryPolicy!(name)

## Fails the current invocation with `reason` and never returns. The host turns
## the call into a trap, so the failure is permanent and uncaught: Golem does not
## retry it, and the app has no result to branch on. A condition the app should
## handle belongs in its own error handling instead (`Try`, a `Err` reply, …).
trap! : Str => {}
trap! = |reason|
	Effect.trap!(reason)

## A durable key for an external write, as a UUID string. Golem derives it from
## the position of this call in the invocation's oplog, so a replay of the same
## invocation (a retry, or recovery after a crash) hands the same call the key the
## earlier attempt used — which is what makes a repeated write duplicate-safe at
## the peer. Pass it to whatever the peer treats as an idempotency key; a header
## alone is not a guarantee. Two calls in one invocation are two positions, so
## each gets its own key.
idempotencyKey! : {} => Str
idempotencyKey! = |_|
	Effect.idempotencyKey!({})

## Forks the agent at this point in the invocation: both copies continue from
## here, and each gets its own result — `Ok("original:<phantom-id>")` on the copy
## the caller is invoking, `Ok("forked:<phantom-id>")` on the new one. The phantom
## id is the same on both sides and is what the forked copy's agent id carries;
## the fork is durable, so neither copy replays the work that led to it twice.
## The two copies are separate agents after this point — they can rejoin through a
## promise or talk over `rpc!`.
fork! : {} => Try(Str, Str)
fork! = |_|
	Effect.fork!({})

## Blocks until the oplog is durable on `replicas` replicas (or on all of them,
## when the deployment has fewer). It is a replication barrier for this agent's
## own history: not a transaction and no guarantee about an external system.
oplogCommit! : U8 => {}
oplogCommit! = |replicas|
	Effect.oplogCommit!(replicas)

## The agent id for `agentName` in the `componentReference` component, as
## `{"componentId": …, "agentId": …}`. The reference is the component's name as
## the manifest declares it (`"roc-golem:counter"`); the name is an agent id
## string such as `counter-agent("c-1")`, and it is not checked for existence. A
## reference that names no deployed component is reported as `Err`.
resolveAgentId! : Str, Str => Try(Str, Str)
resolveAgentId! = |componentReference, agentName|
	Effect.resolveAgentId!(componentReference, agentName)

## The component id of a deployed component, as a UUID string. The reference is
## the component's name as the manifest declares it (`"roc-golem:counter"`); a
## reference that names no deployed component is reported as `Err`.
resolveComponentId! : Str => Try(Str, Str)
resolveComponentId! = |componentReference|
	Effect.resolveComponentId!(componentReference)

## The key-value store (`wasi:keyvalue/eventual`): buckets of key-value pairs,
## shared by the whole environment rather than owned by one agent. Values go in
## and come back as lossy-UTF-8 text — bytes are the store's wire type and this
## platform carries them the way `http!` carries a body — and a bucket is made
## by the first write to it, so there is nothing to open first.
##
## `kvGet!` answers with the value's text, and with the JSON literal `null`
## when the bucket holds no such key. That is how a missing key is told apart
## from an empty value (an empty value reads as `""`); the one value it cannot
## tell apart is stored text that is itself `null`, which `kvExists!` resolves.
kvGet! : Str, Str => Try(Str, Str)
kvGet! = |bucket, key|
	Effect.kvGet!(bucket, key)

## Stores `value` (as UTF-8 bytes) at `key` in `bucket`, replacing whatever was
## there. The bucket comes into existence with the first write.
kvSet! : Str, Str, Str => Try({}, Str)
kvSet! = |bucket, key, value|
	Effect.kvSet!(bucket, key, value)

## Removes `key` from `bucket`. Deleting a key that is not there is not an
## error.
kvDelete! : Str, Str => Try({}, Str)
kvDelete! = |bucket, key|
	Effect.kvDelete!(bucket, key)

## Whether `bucket` holds `key` — the direct check, without reading the value.
kvExists! : Str, Str => Try(Bool, Str)
kvExists! = |bucket, key|
	Effect.kvExists!(bucket, key)

## The blobstore (`wasi:blobstore`): containers of named objects, shared by the
## whole environment. Object bodies go in and come back as lossy-UTF-8 text,
## the convention above; container and object names are plain strings, so no
## handle reaches the app.

## Creates `name` if the container does not exist yet. Calling it on one that
## is already there leaves the container and its objects alone, so an app can
## ensure its container with this one call.
blobCreateContainer! : Str => Try({}, Str)
blobCreateContainer! = |name|
	Effect.blobCreateContainer!(name)

## Stores `value` as the object `object` in `container`, replacing an object
## with that name. The container has to exist (`blobCreateContainer!`).
blobPut! : Str, Str, Str => Try({}, Str)
blobPut! = |container, object, value|
	Effect.blobPut!(container, object, value)

## The whole object as lossy-UTF-8 text. A container or object that is not
## there is reported as `Err`.
blobGet! : Str, Str => Try(Str, Str)
blobGet! = |container, object|
	Effect.blobGet!(container, object)

## Whether `container` holds an object with that name.
blobHas! : Str, Str => Try(Bool, Str)
blobHas! = |container, object|
	Effect.blobHas!(container, object)

## The object names in `container` as a JSON array of strings (`[]` when the
## container is empty). The order is the store's own — the interface promises
## none — so an app that needs a stable one sorts the array.
blobList! : Str => Try(Str, Str)
blobList! = |container|
	Effect.blobList!(container)

## Deletes the object. Deleting one that is not there is not an error.
blobDelete! : Str, Str => Try({}, Str)
blobDelete! = |container, object|
	Effect.blobDelete!(container, object)

## The agent's own filesystem (`wasi:filesystem`): files the agent reads and
## writes live in its sandbox, one filesystem per agent, and every write is
## recorded in the agent's oplog, so it is replayed with the invocation that made
## it and the files survive restarts, snapshots and agent updates. The
## manifest's per-agent `files:` list provisions the initial files; HTTP mounts
## *serve* files out of the same filesystem, which is a different thing (a mount
## answers requests without invoking the agent).
##
## Paths are resolved against the agent's root: a leading `/` names the root (the
## spelling a manifest `files:` target uses), so `/seed/a.txt` and `seed/a.txt`
## are the same file; `.` and `..` normalize inside the root; and a path that
## would step above the root fails with `escapes the agent's filesystem root`.
## File contents travel as lossy-UTF-8 text, the convention `http!` uses for
## bytes, and no file handle reaches the app — every effect opens the file it
## names and closes it again.

## The whole file as text. A file that is not there (or cannot be read) is
## reported in the `Err` channel with the host's error code, such as
## `no-entry`.
readFile! : Str => Try(Str, Str)
readFile! = |path|
	Effect.readFile!(path)

## Creates the file (truncating what was there) and writes the text's UTF-8
## bytes. Missing parent directories are an error; `makeDir!` creates them one
## level at a time.
writeFile! : Str, Str => Try({}, Str)
writeFile! = |path, value|
	Effect.writeFile!(path, value)

## Writes the text's bytes at the end of the file, creating it when it is not
## there yet.
appendFile! : Str, Str => Try({}, Str)
appendFile! = |path, value|
	Effect.appendFile!(path, value)

## Whether anything is there under the path — a directory or a file, a symlink
## followed to what it points at. A path whose parent does not exist answers
## `False`, not `Err`.
fileExists! : Str => Try(Bool, Str)
fileExists! = |path|
	Effect.fileExists!(path)

## The size of the file in bytes (for a directory, the size of the directory's
## own record; for a symlink, the length of the path it holds).
fileSize! : Str => Try(U64, Str)
fileSize! = |path|
	Effect.fileSize!(path)

## Removes the file. A path that is not a file at all — a directory, or nothing
## — is reported (`is-directory`, `no-entry`) rather than silently ignored.
deleteFile! : Str => Try({}, Str)
deleteFile! = |path|
	Effect.deleteFile!(path)

## Creates one directory. Its parent has to exist already, so this is `mkdir`,
## not `mkdir -p`; creating one that exists is reported as `exist`.
makeDir! : Str => Try({}, Str)
makeDir! = |path|
	Effect.makeDir!(path)

## The entries of a directory as a JSON array of `{"name": …,
## "type": …}`, sorted by name so two listings of an unchanged directory read
## the same. `type` is `"file"`, `"dir"`, `"symlink"`, or `"other"` for the
## kinds the interface leaves unclassified (devices, fifos, sockets). `[]` is an
## empty directory.
listDir! : Str => Try(Str, Str)
listDir! = |path|
	Effect.listDir!(path)

## External durable streams (`golem:agent/durable-streams`): a reader and a
## writer for a service that speaks the Durable Streams protocol — a stream
## hosted by another service, addressed by URL. The host owns the HTTP, the
## framing and (for an authenticated stream) the credential; each effect is one
## protocol attempt, recorded in the agent's oplog like every other host call,
## so a replay is answered from the recording instead of reaching the service
## again.
##
## **The app owns the state.** A checkpoint (a server-generated `offset` plus an
## optional `cursor`), a producer identity (`producerId` + `producerEpoch`), the
## next `sequence` and the retry policy are the app's to carry, exactly as the
## official SDKs document; nothing here retries, advances a checkpoint or
## remembers a pending append for the app. `-1` starts a read from the service's
## beginning and `now` asks it to resolve the current tail once during catch-up
## (the checkpoint tokens are opaque: pass them back unchanged).
##
## Both resources are host-side; the app carries them as `U32` handles, the way
## it carries a WebSocket or a permission card. `durableStreamRead!` and
## `durableStreamAppend!` borrow the handle — the reader or writer stays usable
## for the next call — and `durableStreamRelease!` gives it up. Roc holds a
## plain number, so the host cannot see when an app is done; release what you do
## not keep. Releasing a writer never rolls back an append.
##
## The structured values travel as JSON in the WIT shape with camelCase names
## (`retry_json`'s convention); the README's "Durable streams" section spells
## every shape out. A batch's `payload` is bytes, and bytes travel as
## lossy-UTF-8 text (the `http!` convention), so `payloadBytes` — the byte
## length the host received — is how an app notices that the text lost
## something. An error arrives as `"<kind>: <message>"` (`not-found: …`,
## `rate-limited: …`), with `(retryAfterMs=…)`, `(producerEpoch=…)` and
## `(expectedSequence=…)` appended when the error carries them.
##
## Authentication is the one gap: the constructors take the WIT's optional
## `secret` and this platform passes none, because it has no secret-handle
## surface yet (`getConfig!` hands an app a secret's *value*, not a handle). An
## authenticated stream is a follow-up (see the README).

## Builds a reader for `url`: `{"url": …, "mode": "json"|"bytes",
## "timeoutMs": …}`, where the deadline is the whole attempt's and must be 1
## through 300000 ms. `Ok` is the handle `durableStreamRead!` and
## `durableStreamRelease!` work with; `Err` is a text this effect could not read
## (its shape, its mode, or a deadline outside the range). Construction records
## no HTTP call, so it does not reach the service — and it does not create the
## stream: a URL that does not exist yet is reported by the first read.
## `json` mode reads the service's JSON messages as the raw batch text; `bytes`
## mode reads a byte stream (whose HTTP batch boundaries are not preserved).
durableStreamReader! : Str => Try(U32, Str)
durableStreamReader! = |optionsJson|
	Effect.durableStreamReader!(optionsJson)

## One read attempt on the reader behind `handle`:
## `{"checkpoint": {"offset": …, "cursor": …|null},
## "transport": "catch-up"|"long-poll"|"sse", "contentType": …|null}`. The
## checkpoint is where to continue from (`{"offset": "-1", "cursor": null}`
## for the beginning); `contentType` pins the stream's media type, which SSE
## requires, and must agree with the reader's mode. `Ok` is the batch:
## `{"payload": …, "payloadBytes": N, "contentType": …, "next": {"offset":
## …, "cursor": …|null}, "upToDate": …, "closed": …}` — the payload as text
## plus the byte count, and `next` the checkpoint the app advances to once it
## has consumed the batch. `upToDate` says the read reached the service's tail
## and `closed` that the stream ended; an empty payload is not the end unless
## `closed` is true. An error is never the end of the stream.
durableStreamRead! : U32, Str => Try(Str, Str)
durableStreamRead! = |handle, requestJson|
	Effect.durableStreamRead!(handle, requestJson)

## Builds a writer for `url`: `{"url": …, "contentType": …, "producerId":
## …, "producerEpoch": …, "timeoutMs": …}`. The producer identity is the app's
## to choose and to keep (the epoch is the ownership generation: a higher epoch
## at sequence 0 fences older ones for the same id); the deadline rule is the
## reader's, and the epoch stays at or below 2^53-1. The writer only carries
## the identity — retaining it advances no sequence by itself.
durableStreamWriter! : Str => Try(U32, Str)
durableStreamWriter! = |optionsJson|
	Effect.durableStreamWriter!(optionsJson)

## One append attempt on the writer behind `handle`:
## `{"payload": {"tag": "json", "value": ["…", …]},
## "sequence": N, "close": …}` for JSON values — each element is one complete
## encoded value, and the host frames the outer array — or
## `{"payload": {"tag": "bytes", "value": "…"}, …}`, which appends the
## text's UTF-8 bytes. `sequence` counts *requests* for this producer, not
## messages or bytes, and it is the app's to advance: the same sequence,
## payload and `close` flag sent again is the protocol's idempotent retry of an
## uncertain append, and new data takes the next sequence. Empty data is only
## valid with `close` true, which commits the data and the closure together.
## `Ok` is the receipt `{"nextOffset": …|null, "epoch": …, "sequence": …,
## "closed": …}` — a duplicate acknowledgement may omit the offset — and the
## writer stays usable for the next sequence or for the retry.
durableStreamAppend! : U32, Str => Try(Str, Str)
durableStreamAppend! = |handle, requestJson|
	Effect.durableStreamAppend!(handle, requestJson)

## Gives a reader or writer handle up: the resource is dropped and the handle is
## freed for reuse. A handle that was never allocated, or was already released,
## is reported (`unknown durable stream handle N`), never a use-after-free.
durableStreamRelease! : U32 => Try({}, Str)
durableStreamRelease! = |handle|
	Effect.durableStreamRelease!(handle)

## Databases (`golem:rdbms`): three engines — postgres, MySQL and Apache Ignite
## 2.x — with one `open` per engine and a shared statement surface. A connection
## and a transaction are host-side resources the app carries as `U32` handles:
## `dbQuery!` and `dbExecute!` take either a connection's or a transaction's
## handle, and `dbCommit!` / `dbRollback!` consume the transaction's.
##
## Statements and values travel as JSON. The params are a JSON array of
## `db-value` encodings in the opened engine's own vocabulary, and `dbQuery!`
## answers `{"columns": […], "rows": […]}` with each column's ordinal, name,
## WIT type and the database's own type name. The README's database section
## spells the encoding out, hand-mapped cases included (a UUID is its canonical
## text; a case that carries no payload has just its tag, `{"tag": "null"}`).
##
## The three interfaces are synchronous host calls, and Golem records them in
## the agent's oplog like every other host call, so a replay is answered from
## the recorded calls rather than reaching the database again. `query-stream` is
## not exposed: the effects hand an app text.

dbOpen! : Str => Try(U32, Str)
dbOpen! = |address|
	Effect.dbOpen!(address)

## The same for MySQL. The address is `mysql://user[:password]@host[:port]/db`.
dbOpenMysql! : Str => Try(U32, Str)
dbOpenMysql! = |address|
	Effect.dbOpenMysql!(address)

## The same for Apache Ignite 2.x. The address is
## `ignite://[user:password@]host[:port][?pool_size=N&tls=true]`; the default
## port is 10800.
dbOpenIgnite! : Str => Try(U32, Str)
dbOpenIgnite! = |address|
	Effect.dbOpenIgnite!(address)

## Runs `statement` and returns its result as JSON:
## `{"columns": [{"ordinal": 0, "name": "id", "dbType": {"tag": "int4"},
## "dbTypeName": "INT4"}, …], "rows": [[{"tag": "int4", "value": 1}, …], …]}`.
## A row is the JSON array of its values in column order (the WIT `db-row`'s
## single `values` field is that array), and Ignite's columns carry no type
## because its `db-column` does not.
##
## `params` is a JSON array of `db-value` encodings — `[]` for a statement
## without parameters — in the vocabulary of the engine the handle was opened
## on: `[{"tag": "int4", "value": 1}, {"tag": "text", "value": "a"}]` for
## postgres, `[{"tag": "int", "value": 1}]` for MySQL,
## `[{"tag": "dbInt", "value": 1}]` for Ignite. A params text the encoding
## cannot read is reported in the `Err` channel, like a host error.
##
## A statement that is not a query (a `CREATE`, an `INSERT`, …) belongs in
## `dbExecute!`; a `query` runs whatever the database accepts and hands back
## whatever columns it answered with.
dbQuery! : U32, Str, Str => Try(Str, Str)
dbQuery! = |handle, statement, params|
	Effect.dbQuery!(handle, statement, params)

## Runs `statement` and returns the number of affected rows. The params are the
## same JSON array `dbQuery!` takes.
dbExecute! : U32, Str, Str => Try(U64, Str)
dbExecute! = |handle, statement, params|
	Effect.dbExecute!(handle, statement, params)

## Starts a transaction on the connection and returns its handle: statements
## run through that handle until `dbCommit!` or `dbRollback!` consumes it. A
## handle is spent once — using it again is reported, not replayed.
dbBegin! : U32 => Try(U32, Str)
dbBegin! = |handle|
	Effect.dbBegin!(handle)

## Commits the transaction and consumes its handle.
dbCommit! : U32 => Try({}, Str)
dbCommit! = |handle|
	Effect.dbCommit!(handle)

## Rolls the transaction back and consumes its handle.
dbRollback! : U32 => Try({}, Str)
dbRollback! = |handle|
	Effect.dbRollback!(handle)

## Permissions (`golem:permissions`) and the principal
##
## Golem decides what an agent may do with *permission cards*: each card is an
## unforgeable capability listing grants (`class(owner) @ recipient : verb :
## resource-id`), the set of cards an agent holds is its *wallet*, and the
## runtime checks the wallet before an operation that needs one. These effects
## expose the wallet an app already holds, the inspection/derivation/
## installation/revocation operations the official SDKs expose, and the
## kernel's own introspection — they do not create authority: a card can only
## be narrowed, and an operation the cards do not allow is the runtime's
## refusal, reported in the effect's `Err`.
##
## A `permission-card` is host-side; the app carries it as a `U32` handle, the
## way it carries a WebSocket or database handle. Ownership is explicit:
##
## * `permissionId!` / `permissionParents!` / `permissionExpiresAt!` /
##   `permissionInspect!` / `permissionDerive!` / `permissionDeriveScope!`
##   **borrow** the handle — the card stays and can be used again,
## * `permissionRevoke!` and `permissionInstall!` **consume** it (the interface
##   takes ownership), so using the same handle twice is an `Err`
##   (`unknown permission card handle N`), never a use-after-free,
## * `permissionRelease!` gives a card up without using it as authority: the
##   card is dropped and the handle is freed. Roc holds a plain number, so the
##   host cannot see when an app is done with a card — a handle that is neither
##   consumed nor released stays in the instance for as long as it lives, so
##   release what you do not keep.
##
## The structured values travel as JSON in the WIT shape with camelCase names
## (`retry_json`'s convention). The README's "Permissions and the principal"
## section spells every shape out; the short version: a grant is
## `{"class":…,"owner":…,"recipient":…,"verb":…,"resourceId":…}`, a
## bounds object is `{"lowerPositive":[…],"lowerNegative":[…],
## "upperPositive":[…],"upperNegative":[…],"lifespan":{"iso8601":"P30D"}}`
## (or `null`/absent), a `deriveScope!` request adds `"parents":[<handle>,…]`
## and takes no lifespan, a holder is
## `{"tag":"account"|"app"|"agent","val":…}`, and a `permission-error`
## arrives as `"<case-name>: <message>"` (`not-permitted: …`,
## `lower-bound-too-broad: …`).

## The principal the runtime handed this invocation, as JSON:
## `{"tag":"anonymous"}`, `{"tag":"golem-user","val":{"accountId":…}}`,
## `{"tag":"agent","val":{"componentId":…,"agentId":…}}` (an RPC caller),
## or `{"tag":"oidc","val":{…}}`. The answer is the JSON literal `null` when
## the runtime handed none: a tool invocation, which the tool ABI carries no
## principal for.
getPrincipal! : {} => Str
getPrincipal! = |_|
	Effect.getPrincipal!({})

## Every card installed in the caller's wallet, as a JSON array of
## `{"tag":"concrete"|"template"|"scope","handle":N}`. A `template` card's
## grants carry slot variables (`?agent`, `?env`) that resolve for whoever ends
## up holding it; `scope` is the card the runtime lent this invocation, when it
## lent one. Each entry's handle is a card this instance now holds, so it is the
## app's to use — and to release.
permissionsWallet! : {} => Try(Str, Str)
permissionsWallet! = |_|
	Effect.permissionsWallet!({})

## The wallet's version, as `{"walletIdHash":[<bytes>],"generation":N}`. It
## changes whenever the set of installed cards does, which is how an app can
## tell that its wallet moved without listing it.
permissionsWalletVersion! : {} => Try(Str, Str)
permissionsWalletVersion! = |_|
	Effect.permissionsWalletVersion!({})

## The card's stable identifier, as a JSON string (its canonical UUID text).
permissionId! : U32 => Try(Str, Str)
permissionId! = |handle|
	Effect.permissionId!(handle)

## The card's parents in the derivation DAG, as a JSON array of UUID strings
## (`[]` for a card derived from nothing).
permissionParents! : U32 => Try(Str, Str)
permissionParents! = |handle|
	Effect.permissionParents!(handle)

## When the card stops being valid, as
## `{"seconds":…,"nanoseconds":…}` — or `null` for a card that never expires
## (for a scope card, the earliest expiry of its root cards).
permissionExpiresAt! : U32 => Try(Str, Str)
permissionExpiresAt! = |handle|
	Effect.permissionExpiresAt!(handle)

## The card's whole grant lattice, as
## `{"lowerPositive":[…],"lowerNegative":[…],"upperPositive":[…],
## "upperNegative":[…]}`. Inspection needs the `card:inspect` permission, so a
## card the caller may hold but not read is reported rather than opened.
permissionInspect! : U32 => Try(Str, Str)
permissionInspect! = |handle|
	Effect.permissionInspect!(handle)

## Derives a narrower child card from the card behind `handle`. The bounds JSON
## says which lower- and upper-positive grants to retain, which negatives to
## add, and how long the child lives:
## `{"lowerPositive":[…],"lowerNegative":[…],"upperPositive":[…],
## "upperNegative":[…],"lifespan":{"iso8601":"P30D"}}` (`lifespan` may be
## absent or `null`). A child can only narrow its parent: a grant the parent
## does not cover is refused (`lower-bound-too-broad` /
## `upper-bound-too-broad`). `Ok` is the handle of the derived card — it is *not*
## installed anywhere, and the parent is only borrowed.
permissionDerive! : U32, Str => Try(U32, Str)
permissionDerive! = |handle, boundsJson|
	Effect.permissionDerive!(handle, boundsJson)

## The same, derived from a card the caller's own wallet already holds instead
## of one named parent. The runtime picks the source card; a request no card in
## the wallet covers is refused.
permissionDeriveFromWallet! : Str => Try(U32, Str)
permissionDeriveFromWallet! = |boundsJson|
	Effect.permissionDeriveFromWallet!(boundsJson)

## Derives an invocation-local card from the cards whose handles the request
## names: `{"parents":[<handle>,…],"lowerPositive":[…],"lowerNegative":[…],
## "upperPositive":[…],"upperNegative":[…]}`, with no lifespan (the card lives
## exactly as long as the invocation that derived it, and cannot be installed or
## revoked). The requested grants must be concrete — a slot variable in a
## scope request is `invalid-pattern` — and the parents are only borrowed.
permissionDeriveScope! : Str => Try(U32, Str)
permissionDeriveScope! = |requestJson|
	Effect.permissionDeriveScope!(requestJson)

## Installs the card behind `handle` onto `holder`, a JSON
## `{"tag":"agent","val":{"componentId":…,"agentId":…}}` (the shape
## `resolveAgentId!` and `getPrincipal!` print). The card is consumed by the
## call, so the handle is spent whether or not the installation succeeded. Only
## agent targets are supported today: an `account` or `app` holder is refused.
permissionInstall! : U32, Str => Try({}, Str)
permissionInstall! = |handle, holderJson|
	Effect.permissionInstall!(handle, holderJson)

## Revokes the card behind `handle` and every card derived from it, consuming
## the handle. `Ok` is how many cards the revocation cut, the card itself
## included. Revoking a card that is already revoked is `card-revoked`, not a
## success.
permissionRevoke! : U32 => Try(U32, Str)
permissionRevoke! = |handle|
	Effect.permissionRevoke!(handle)

## Gives a card handle up: the card is dropped (which releases the runtime's own
## entry for it) and the handle is freed for reuse. A handle that was never
## allocated, or was already consumed by `permissionRevoke!` /
## `permissionInstall!` / an earlier `permissionRelease!`, is reported.
permissionRelease! : U32 => Try({}, Str)
permissionRelease! = |handle|
	Effect.permissionRelease!(handle)

## Every resource class the permission kernel knows about, as a JSON array of
## `{"className":…,"verbs":[…]}`, in the order the kernel listed them. This is
## what tells an app which classes and verbs a grant may name.
permissionListModules! : {} => Try(Str, Str)
permissionListModules! = |_|
	Effect.permissionListModules!({})

## Checks one grant — a JSON `pattern-grant` — against the kernel's schema: its
## class, owner, recipient, verb and resource-id pattern. `Ok` means the kernel
## accepts it; anything it rejects comes back in the `Err` channel
## (`unknown-resource-class: …`, `invalid-verb: …`, `invalid-pattern: …`).
permissionValidateGrant! : Str => Try({}, Str)
permissionValidateGrant! = |grantJson|
	Effect.permissionValidateGrant!(grantJson)

# ============================================================================
# The typed agent-authoring surface (`Agent.method`, `Agent.define`,
# `Agent.register`, `Agent.build`). It lives in this module — rather than a
# module of its own — because a module named `Agent` collides with the `Agent`
# type below when an application imports both, and because this file already
# keeps the legacy header an application can import types through.
#
# `define` starts a spec, the `with*` builders add the config, the tools and
# the mount, `register` / `registerReadOnly` bind a typed handler to each
# method, and `build` turns the spec into the `Agent(state)` record above with
# a dispatcher, the derived metadata and the derived state codecs.
# ============================================================================

## One method an agent declares. `method` builds it; `register` and
## `registerReadOnly` bind a handler whose input and output types are the ones
## it carries. Whether the method is *read-only* is the registration's to say:
## a handler bound with `registerReadOnly` is pure and its declaration carries
## the 1.6 read-only flag, one bound with `register` is effectful and is not.
Method : {
	name : Str,
	description : Str,
	promptHint : Str,
}

## Declares a method. `description` and `promptHint` are what the tooling (the
## CLI, MCP, the registry) shows a caller; both may be `""`.
method : { name : Str, description : Str, promptHint : Str } -> Method
method = |cfg| {
	name: cfg.name,
	description: cfg.description,
	promptHint: cfg.promptHint,
}

## What a bound handler is stored as: the declaration the metadata carries, plus
## an erased call. `invoke!` takes the input as its JSON text and answers with
## the output as JSON text; the erasure is what lets one agent hold methods
## whose input and output types differ in a single list.
Handler(state) : {
	declaration : MethodDeclaration,
	invoke! : state, Str => Try({ state : state, out : Str }, Str),
}

## The app-facing tool handler: what `handleToolCall!` answers with. The default
## reports every tool as unknown.
ToolHandler(state) : state, ToolCall => { state : state, result : ToolResult }

defaultToolHandler : state, ToolCall => { state : state, result : ToolResult }
defaultToolHandler = |state, call| {
	{
		state,
		result: {
			id: call.id,
			success: False,
			output: "{\"error\": \"Unknown tool ${call.name}\"}",
			stdout: "",
			stderr: "",
		},
	}
}

## The specification an app builds up. `build` turns it into the `Agent(state)`
## record the platform's `requires` expects.
Spec(state) : {
	name : Str,
	version : Str,
	description : Str,
	init! : Str => state,
	handlers : List(Handler(state)),
	tools : List(ToolDefinition),
	toolHandler : ToolHandler(state),
	config : List(ConfigDeclaration),
	mount : HttpMountOption,
}

## Starts a spec. `init!` is the agent's constructor: it receives the
## constructor parameter as text (the ID) and answers with the initial state,
## which pins the agent's `state` type for every handler registered after.
define : { name : Str, version : Str, description : Str }, (Str => state) -> Spec(state)
define = |cfg, init!| {
	name: cfg.name,
	version: cfg.version,
	description: cfg.description,
	init!: init!,
	handlers: [],
	tools: [],
	toolHandler: defaultToolHandler,
	config: [],
	mount: None,
}

## Declares the tools this agent publishes (`golem:tool/guest`); the app answers
## them through `handleToolCall!` or through the handler `withToolHandler` binds.
withTools : Spec(state), List(ToolDefinition) -> Spec(state)
withTools = |spec, tools| {
	..spec,
	tools: tools,
}

## Binds the app's tool handler. `call.arguments` is the tool's single string
## input and `call.stdin` the text the caller piped in; the answer's
## `stdout`/`stderr` are what the caller's attached streams receive.
withToolHandler : Spec(state), ToolHandler(state) -> Spec(state)
withToolHandler = |spec, handler| {
	..spec,
	toolHandler: handler,
}

## Declares the config keys the agent reads with `getConfig!`; a key marked
## `secret: True` comes from the environment's secret store.
withConfig : Spec(state), List(ConfigDeclaration) -> Spec(state)
withConfig = |spec, config| {
	..spec,
	config: config,
}

## Publishes the agent type on the deployment's domain (`HttpMountOption`), the
## way a filesystem-binding mount serves the agent's own files.
withMount : Spec(state), HttpMountOption -> Spec(state)
withMount = |spec, mount| {
	..spec,
	mount: mount,
}

## Binds an effectful handler to `m`. The handler is a `state, input => ...`
## function (write its type on a named binding to pin the input and output); it
## receives the current state and the decoded input and answers with the next
## state and the output, which the platform decodes and encodes through the
## derived JSON codecs.
register : Spec(state), Method, (state, input => Try({ state : state, out : output }, Str)) -> Spec(state) where [
	input.Json.Parseable([InvalidJson(Str), ..]),
	output.Json.Encodable(_),
]
register = |spec, m, fn!| {
	invoke! = |state, inputJson|
		match Json.parse(inputJson) {
			Err(_) => Err("invalid input for ${m.name}")
			Ok(inp) =>
				match fn!(state, inp) {
					Err(e) => Err(e)
					Ok(res) => Ok({ state: res.state, out: Json.to_str(res.out) })
				}
		}
	addHandler(spec, {
		declaration: { name: m.name, description: m.description, promptHint: m.promptHint, readOnly: False },
		invoke!: invoke!,
	})
}

## Binds a pure handler to `m`: a `state, input -> output` function that may not
## perform effects or change the state, which is what the 1.6 read-only flag
## declares, so the runtime may cache the answer and refuses a write made from
## the call.
registerReadOnly : Spec(state), Method, (state, input -> output) -> Spec(state) where [
	input.Json.Parseable([InvalidJson(Str), ..]),
	output.Json.Encodable(_),
]
registerReadOnly = |spec, m, pure| {
	invoke! = |state, inputJson|
		match Json.parse(inputJson) {
			Err(_) => Err("invalid input for ${m.name}")
			Ok(inp) => Ok({ state: state, out: Json.to_str(pure(state, inp)) })
		}
	addHandler(spec, {
		declaration: { name: m.name, description: m.description, promptHint: m.promptHint, readOnly: True },
		invoke!: invoke!,
	})
}

addHandler : Spec(state), Handler(state) -> Spec(state)
addHandler = |spec, handler| {
	..spec,
	handlers: List.append(spec.handlers, handler),
}

## One call as the host sends it: the method's name and its input as JSON text.
## The host wraps a declared method's parameter in this envelope so the
## dispatcher knows which handler to run (see `host/src/lib.rs`).
Call : { method : Str, input : Str }

decodeCall : Str -> Try(Call, Str)
decodeCall = |json| {
	parsed : Try({ method : Try(Str, [Missing]), input : Try(Str, [Missing]) }, [InvalidJson(Str)])
	parsed = Json.parse(json)
	match parsed {
		Err(_) => Err("this agent only answers calls through its declared methods")
		Ok(fields) => Ok({ method: fields.method ?? "", input: fields.input ?? "{}" })
	}
}

findHandler : List(Handler(state)), Str, U64 -> Try(Handler(state), Str)
findHandler = |handlers, name, index| {
	match List.get(handlers, index) {
		Err(_) => Err("no method named ${name}")
		Ok(handler) =>
			if handler.declaration.name == name {
				Ok(handler)
			} else {
				findHandler(handlers, name, index + 1)
			}
	}
}

declarationOf : Handler(state) -> MethodDeclaration
declarationOf = |handler| handler.declaration

## Turns the spec into the `Agent(state)` record the platform's `requires`
## expects: the dispatcher answers `handle-message` (the host routes every
## declared method here, in the envelope above), the metadata carries the
## declarations, and the state codecs are the derived JSON ones.
build : Spec(state) -> Agent(state) where [
	state.Json.Encodable(_),
	state.Json.Parseable([InvalidJson(Str), ..]),
]
build = |spec| {
	dispatch! = |state, messageJson|
		match decodeCall(messageJson) {
			Err(err) => { state, replies: [err] }
			Ok(call) =>
				match findHandler(spec.handlers, call.method, 0) {
					Err(err) => { state, replies: [err] }
					Ok(handler) =>
						match (handler.invoke!)(state, call.input) {
							Err(err) => { state, replies: [err] }
							Ok(res) => { state: res.state, replies: [res.out] }
						}
				}
		}
	{
		init!: spec.init!,
		handleMessage!: dispatch!,
		handleRequest!: |state, _request| {
			state,
			response: { status: 404, headers: [], body: "this agent is not an HTTP router" },
		},
		handleMultimodal!: |state, _parts| { state, parts: "[]" },
		handleToolCall!: spec.toolHandler,
		metadata: {
			name: spec.name,
			version: spec.version,
			description: spec.description,
			tools: spec.tools,
			methods: List.map(spec.handlers, declarationOf),
			config: spec.config,
			agentKind: Regular,
			httpMount: spec.mount,
			httpHandler: NoHandler,
			multimodal: NoMultimodal,
			openapiProvider: NoOpenApi,
		},
		serializeState: |state| Json.to_str(state),
		deserializeState: |json|
			match Json.parse(json) {
				Ok(state) => Ok(state)
				Err(_) => Err("failed to parse state: ${json}")
			},
	}
}
```

```roc
## Types representing Golem Agent Metadata and Tool specifications

Types := {}.{
	ToolParameter : { name : Str, description : Str, paramType : Str, required : Bool }
	# `readOnly` maps to the 1.6 read-only method flag: the runtime may cache the
	# result and traps writes made from the call.
	ToolDefinition : { name : Str, description : Str, parameters : List(ToolParameter), readOnly : Bool }
	Metadata : {
		name : Str,
		version : Str,
		description : Str,
		tools : List(ToolDefinition),
		# The methods this agent type declares beyond `handle-message`. Empty for
		# an app that answers everything through `handleMessage!`. The host
		# registers each as a Golem method whose single parameter is the JSON text
		# of its input, and routes an invocation of that name back to the app's
		# own dispatcher (see `platform/Agent.roc`).
		methods : List(MethodDeclaration),
		config : List(ConfigDeclaration),
		agentKind : AgentKind,
		httpMount : HttpMountOption,
		httpHandler : HttpHandlerOption,
		multimodal : MultimodalOption,
		openapiProvider : OpenApiProvider,
	}
	# One method an app declares through `Agent.method` / `Agent.register`. Only
	# the fields a caller can see travel: `name` is what the CLI/RPC invokes,
	# `description` and `promptHint` are the tooling's, and `readOnly` is the 1.6
	# read-only method flag (the runtime may cache the result and traps writes
	# made from the call).
	MethodDeclaration : {
		name : Str,
		description : Str,
		promptHint : Str,
		readOnly : Bool,
	}
	# `HttpRouter` agent types are served by Golem itself: they are parameterless,
	# ephemeral, have no durable filesystem and register only their OpenAPI
	# provider method (and their request handler, when declared). Use `Regular`
	# for an agent with methods, state and (optionally) a filesystem mount.
	AgentKind : [Regular, HttpRouter]
	# `HandlesRequests` registers the router's `handle` method, so every request
	# its static bindings do not match is handed to `handleRequest!`. `NoHandler`
	# keeps a router purely static (and lets unmatched paths answer 404 without
	# waking anything).
	HttpHandlerOption : [NoHandler, HandlesRequests]
	# `HandlesMultimodal` declares the `handle-multimodal` method, whose input and
	# output are a multimodal list of parts, and reaches the app through
	# `handleMultimodal!`. `NoMultimodal` (what an app with nothing to do with
	# multimodal content writes) registers no such method, so the hook is
	# unreachable — the same way `handleRequest!` is unreachable without
	# `HandlesRequests`. A router cannot declare it: routers register only their
	# request handler and their OpenAPI provider.
	MultimodalOption : [NoMultimodal, HandlesMultimodal]
	# One part of a multimodal envelope. The envelope itself travels as JSON text
	# (the app reads it out of `handleMultimodal!`'s argument and answers with the
	# same shape — see the README's "Multimodal values"); these types are the
	# shape that text carries, for an app that wants to model the parts it reads
	# or builds.
	MultimodalPart : [
		Text(MultimodalText),
		Binary(MultimodalBinary),
	]
	# A text part's text and its BCP-47 language code (`""` when the caller's
	# value carried none).
	MultimodalText : { text : Str, language : Str }
	# A binary part's byte values, as the JSON array text the envelope carries
	# (`[1,2,3]`), and its MIME type (`""` when the caller's value carried none).
	MultimodalBinary : { bytes : Str, mimeType : Str }
	# The parts a multimodal method carries, in order: what `handle-multimodal`
	# receives and what it answers with.
	MultimodalEnvelope : List(MultimodalPart)
	# One request routed to a router's mount. `headersJson` is the raw header list
	# (`[{"name":...,"value":...}]`, in request order) and `query` is empty when
	# the URL had none.
	HttpRequest : {
		method : Str,
		scheme : Str,
		authority : Str,
		path : Str,
		query : Str,
		headersJson : Str,
		body : Str,
	}
	HttpHeader : { name : Str, value : Str }
	# What `handleRequest!` answers with: a status, response headers and a body.
	HttpResponse : { status : U16, headers : List(HttpHeader), body : Str }
	# The OpenAPI document a router publishes at the deployment's
	# `/openapi.yaml`. `NoOpenApi` leaves the route to Golem's generated spec.
	OpenApiProvider : [NoOpenApi, OpenApi(Str)]
	# An HTTP mount publishes the agent type on its deployment's domain. `None`
	# keeps the agent reachable only through the CLI/RPC surfaces.
	HttpMountOption : [None, Mount(HttpMount)]
	# `pathPrefix` segments are URL literals; a segment written as `{name}`
	# captures a value, and that name is the agent's constructor (identity)
	# parameter, so the URL selects which instance serves the request. Golem
	# requires every constructor parameter to be captured exactly once, and
	# router mounts must be literal-only (they have no constructor).
	#
	# `filesystemBindings` (regular agents) map public paths onto files in the
	# agent's own durable filesystem. `staticBindings` (routers) map them onto
	# files uploaded at deploy time from the manifest's `files:` list, which the
	# runtime serves itself with an immutable ETag. Either way matching GET/HEAD
	# requests never invoke the agent.
	HttpMount : {
		pathPrefix : List(Str),
		staticBindings : List(FileBinding),
		filesystemBindings : List(FileBinding),
		fileResponseHeaders : List(FileResponseHeader),
		corsAllowedPatterns : List(Str),
	}
	# `Exact` serves one public path from one file. `Subtree` serves every path
	# under a public prefix from a filesystem root, preserving the rest of the
	# path; the root is a directory inside the agent's filesystem.
	FileBinding : [
		Exact({ publicPath : List(Str), filePath : Str }),
		Subtree({ publicPrefix : List(Str), filesystemRoot : Str }),
	]
	# Extra headers added to responses served from a file binding. Golem
	# rejects headers it owns (content-length, cache-control, etag, ...).
	FileResponseHeader : { name : Str, value : Str }
	# A tool invocation. `arguments` is the single string input the platform
	# declares for every tool; `stdin` is the text the caller piped into the tool
	# (`golem tool invoke --stdin …`), empty when the caller attached nothing.
	ToolCall : { id : Str, name : Str, arguments : Str, stdin : Str }
	# The app's answer. `stdout` and `stderr` are the text the tool writes on the
	# streams the caller attached (`golem tool invoke --stdout` / `--stderr`); an
	# empty string writes nothing. Both are written after `handleToolCall!`
	# returns, so a tool cannot emit output while it runs yet (see the README).
	ToolResult : { id : Str, success : Bool, output : Str, stdout : Str, stderr : Str }
	# A config key the agent reads with `getConfig!`. `secret` marks it as coming
	# from the environment's secret store instead of the manifest's local config.
	ConfigDeclaration : { name : Str, secret : Bool }
	PersistenceMode : [PersistNothing, PersistStateOnly, PersistEverything]
	LogLevel : [Trace, Debug, Info, Warn, Error]
}
```

```roc
import Types exposing [LogLevel, PersistenceMode]
import Host

## Low-level effect implementations that call the Rust host functions.
Effect := {}.{
	log! : LogLevel, Str => {}
	log! = |level, msg| {
		levelU8 : U8
		levelU8 =
			match level {
				Trace => 0
				Debug => 1
				Info => 2
				Warn => 3
				Error => 4
			}
		Host.rocFxLog!(levelU8, msg)
	}

	getWorkerId! : {} => Str
	getWorkerId! = |_| Host.rocFxGetWorkerId!({})

	createWebhook! : {} => Try(Str, Str)
	createWebhook! = |_| Host.rocFxCreateWebhook!({})

	awaitWebhook! : Str => Try(Str, Str)
	awaitWebhook! = |url| Host.rocFxAwaitWebhook!(url)

	rpcInvoke! : Str, Str, Str => Try(Str, Str)
	rpcInvoke! = |targetWorker, fnName, payload| {
		Host.rocFxRpcInvoke!(targetWorker, fnName, payload)
	}

	rpcFireAndForget! : Str, Str, Str => Try(Str, Str)
	rpcFireAndForget! = |targetWorker, fnName, payload| {
		Host.rocFxRpcFireAndForget!(targetWorker, fnName, payload)
	}

	rpcSchedule! : Str, Str, Str, U64 => Try(Str, Str)
	rpcSchedule! = |targetWorker, fnName, payload, delayMillis| {
		Host.rocFxRpcSchedule!(targetWorker, fnName, payload, delayMillis)
	}

	rpcScheduleCancelable! : Str, Str, Str, U64 => Try(U32, Str)
	rpcScheduleCancelable! = |targetWorker, fnName, payload, delayMillis| {
		Host.rocFxRpcScheduleCancelable!(targetWorker, fnName, payload, delayMillis)
	}

	rpcCancel! : U32 => Try({}, Str)
	rpcCancel! = |handle| {
		Host.rocFxRpcCancel!(handle)
	}

	parseAgentId! : Str => Try(Str, Str)
	parseAgentId! = |agentId| {
		Host.rocFxParseAgentId!(agentId)
	}

	makeAgentId! : Str, Str => Try(Str, Str)
	makeAgentId! = |agentTypeName, constructorParam| {
		Host.rocFxMakeAgentId!(agentTypeName, constructorParam)
	}

	setPersistenceMode! : PersistenceMode => {}
	setPersistenceMode! = |mode| {
		modeU8 : U8
		modeU8 =
			match mode {
				PersistNothing => 0
				PersistStateOnly => 1
				PersistEverything => 2
			}
		Host.rocFxSetPersistence!(modeU8)
	}

	httpRequest! : Str => Try(Str, Str)
	httpRequest! = |reqJson| {
		Host.rocFxHttpRequest!(reqJson)
	}

	wsConnect! : Str => Try(U32, Str)
	wsConnect! = |url| {
		Host.rocFxWsConnect!(url)
	}

	wsSend! : U32, Str => Try({}, Str)
	wsSend! = |handle, msg| {
		Host.rocFxWsSend!(handle, msg)
	}

	wsReceive! : U32 => Try(Str, Str)
	wsReceive! = |handle| {
		Host.rocFxWsReceive!(handle)
	}

	wsClose! : U32 => Try({}, Str)
	wsClose! = |handle| {
		Host.rocFxWsClose!(handle)
	}

	sleepMillis! : U64 => {}
	sleepMillis! = |millis| {
		Host.rocFxSleepMillis!(millis)
	}

	nowMillis! : {} => U64
	nowMillis! = |_| {
		Host.rocFxNowMillis!({})
	}

	getConfig! : Str => Try(Str, Str)
	getConfig! = |key| {
		Host.rocFxGetConfig!(key)
	}

	## Quotas: a token, a reservation and the commit are three host calls, so
	## the app carries the two handles between them.
	newQuotaToken! : Str, U64 => U32
	newQuotaToken! = |resourceName, expectedUse| {
		Host.rocFxNewQuotaToken!(resourceName, expectedUse)
	}

	quotaReserve! : U32, U64 => Try(U32, Str)
	quotaReserve! = |token, amount| {
		Host.rocFxQuotaReserve!(token, amount)
	}

	quotaCommit! : U32, U64 => Try({}, Str)
	quotaCommit! = |reservation, used| {
		Host.rocFxQuotaCommit!(reservation, used)
	}

	quotaSplit! : U32, U64 => Try(U32, Str)
	quotaSplit! = |token, childExpectedUse| {
		Host.rocFxQuotaSplit!(token, childExpectedUse)
	}

	quotaMerge! : U32, U32 => Try({}, Str)
	quotaMerge! = |token, other| {
		Host.rocFxQuotaMerge!(token, other)
	}

	## Retry policies: the trees travel as the JSON `getRetryPolicies!`
	## prints, so nothing is parsed on the Roc side.
	getRetryPolicies! : {} => Str
	getRetryPolicies! = |_| {
		Host.rocFxGetRetryPolicies!({})
	}

	getRetryPolicy! : Str => Try(Str, Str)
	getRetryPolicy! = |name| {
		Host.rocFxGetRetryPolicy!(name)
	}

	resolveRetryPolicy! : Str, Str, Str => Try(Str, Str)
	resolveRetryPolicy! = |verb, nounUri, propertiesJson| {
		Host.rocFxResolveRetryPolicy!(verb, nounUri, propertiesJson)
	}

	nextRetryDelay! : Str, Str, U64, Str => Try(Str, Str)
	nextRetryDelay! = |policyJson, stateJson, elapsedNanos, contextJson| {
		Host.rocFxNextRetryDelay!(policyJson, stateJson, elapsedNanos, contextJson)
	}

	setRetryPolicy! : Str => Try({}, Str)
	setRetryPolicy! = |policyJson| {
		Host.rocFxSetRetryPolicy!(policyJson)
	}

	removeRetryPolicy! : Str => {}
	removeRetryPolicy! = |name| {
		Host.rocFxRemoveRetryPolicy!(name)
	}

	## Oplog and execution control (`golem:api/host`), the part of it an app can
	## use: no oplog cursor and no idempotence switch survive in Golem 1.6, so
	## what is left is failing an invocation, naming an external write, forking
	## the agent, committing the oplog, and resolving component and agent ids.
	trap! : Str => {}
	trap! = |reason| {
		Host.rocFxTrap!(reason)
	}

	idempotencyKey! : {} => Str
	idempotencyKey! = |_| {
		Host.rocFxIdempotencyKey!({})
	}

	fork! : {} => Try(Str, Str)
	fork! = |_| {
		Host.rocFxFork!({})
	}

	oplogCommit! : U8 => {}
	oplogCommit! = |replicas| {
		Host.rocFxOplogCommit!(replicas)
	}

	resolveAgentId! : Str, Str => Try(Str, Str)
	resolveAgentId! = |componentReference, agentName| {
		Host.rocFxResolveAgentId!(componentReference, agentName)
	}

	resolveComponentId! : Str => Try(Str, Str)
	resolveComponentId! = |componentReference| {
		Host.rocFxResolveComponentId!(componentReference)
	}

	## The two shared stores (`wasi:keyvalue`, `wasi:blobstore`): one effect per
	## operation. Values are bytes and travel as lossy-UTF-8 text, the convention
	## `http!` uses for bodies; buckets, keys, containers and objects are named by
	## strings, so no handle has to be carried between calls. A missing key is
	## `"null"` (`kvExists!` is the direct check).
	kvGet! : Str, Str => Try(Str, Str)
	kvGet! = |bucket, key| {
		Host.rocFxKvGet!(bucket, key)
	}

	kvSet! : Str, Str, Str => Try({}, Str)
	kvSet! = |bucket, key, value| {
		Host.rocFxKvSet!(bucket, key, value)
	}

	kvDelete! : Str, Str => Try({}, Str)
	kvDelete! = |bucket, key| {
		Host.rocFxKvDelete!(bucket, key)
	}

	kvExists! : Str, Str => Try(Bool, Str)
	kvExists! = |bucket, key| {
		Host.rocFxKvExists!(bucket, key)
	}

	blobCreateContainer! : Str => Try({}, Str)
	blobCreateContainer! = |name| {
		Host.rocFxBlobCreateContainer!(name)
	}

	blobPut! : Str, Str, Str => Try({}, Str)
	blobPut! = |container, object, value| {
		Host.rocFxBlobPut!(container, object, value)
	}

	blobGet! : Str, Str => Try(Str, Str)
	blobGet! = |container, object| {
		Host.rocFxBlobGet!(container, object)
	}

	blobHas! : Str, Str => Try(Bool, Str)
	blobHas! = |container, object| {
		Host.rocFxBlobHas!(container, object)
	}

	blobList! : Str => Try(Str, Str)
	blobList! = |container| {
		Host.rocFxBlobList!(container)
	}

	blobDelete! : Str, Str => Try({}, Str)
	blobDelete! = |container, object| {
		Host.rocFxBlobDelete!(container, object)
	}

	## The agent's own filesystem (`wasi:filesystem@0.2.6`): one effect per
	## operation, files named by path and bytes carried as lossy-UTF-8 text. The
	## host resolves the path against the agent's root and opens the file itself,
	## so no descriptor reaches the app and nothing has to be closed.
	readFile! : Str => Try(Str, Str)
	readFile! = |path| {
		Host.rocFxReadFile!(path)
	}

	writeFile! : Str, Str => Try({}, Str)
	writeFile! = |path, value| {
		Host.rocFxWriteFile!(path, value)
	}

	appendFile! : Str, Str => Try({}, Str)
	appendFile! = |path, value| {
		Host.rocFxAppendFile!(path, value)
	}

	fileExists! : Str => Try(Bool, Str)
	fileExists! = |path| {
		Host.rocFxFileExists!(path)
	}

	fileSize! : Str => Try(U64, Str)
	fileSize! = |path| {
		Host.rocFxFileSize!(path)
	}

	deleteFile! : Str => Try({}, Str)
	deleteFile! = |path| {
		Host.rocFxDeleteFile!(path)
	}

	makeDir! : Str => Try({}, Str)
	makeDir! = |path| {
		Host.rocFxMakeDir!(path)
	}

	listDir! : Str => Try(Str, Str)
	listDir! = |path| {
		Host.rocFxListDir!(path)
	}

	## External durable streams (`golem:agent/durable-streams`): a reader and a
	## writer for a service that speaks the Durable Streams protocol. Both are
	## host-side resources the app carries as `U32` handles, and every structured
	## value travels as JSON in the WIT shape (see `Golem.roc` for the shapes and
	## the README for what the app owns).
	durableStreamReader! : Str => Try(U32, Str)
	durableStreamReader! = |optionsJson| {
		Host.rocFxDurableStreamReader!(optionsJson)
	}

	durableStreamRead! : U32, Str => Try(Str, Str)
	durableStreamRead! = |handle, requestJson| {
		Host.rocFxDurableStreamRead!(handle, requestJson)
	}

	durableStreamWriter! : Str => Try(U32, Str)
	durableStreamWriter! = |optionsJson| {
		Host.rocFxDurableStreamWriter!(optionsJson)
	}

	durableStreamAppend! : U32, Str => Try(Str, Str)
	durableStreamAppend! = |handle, requestJson| {
		Host.rocFxDurableStreamAppend!(handle, requestJson)
	}

	durableStreamRelease! : U32 => Try({}, Str)
	durableStreamRelease! = |handle| {
		Host.rocFxDurableStreamRelease!(handle)
	}

	## Databases (`golem:rdbms`): three engines, one `open` per interface, and a
	## shared query/execute/transaction surface. Connections and transactions are
	## `U32` handles in the host's handle table; a statement's params are a JSON
	## array of `db-value` encodings and a result is `{"columns": …, "rows": …}`
	## (the encoding is spelled out in the README's database section).
	dbOpen! : Str => Try(U32, Str)
	dbOpen! = |address| {
		Host.rocFxDbOpen!(address)
	}

	dbOpenMysql! : Str => Try(U32, Str)
	dbOpenMysql! = |address| {
		Host.rocFxDbOpenMysql!(address)
	}

	dbOpenIgnite! : Str => Try(U32, Str)
	dbOpenIgnite! = |address| {
		Host.rocFxDbOpenIgnite!(address)
	}

	dbQuery! : U32, Str, Str => Try(Str, Str)
	dbQuery! = |handle, statement, params|
		Host.rocFxDbQuery!(handle, statement, params)

	dbExecute! : U32, Str, Str => Try(U64, Str)
	dbExecute! = |handle, statement, params|
		Host.rocFxDbExecute!(handle, statement, params)

	dbBegin! : U32 => Try(U32, Str)
	dbBegin! = |handle|
		Host.rocFxDbBegin!(handle)

	dbCommit! : U32 => Try({}, Str)
	dbCommit! = |handle|
		Host.rocFxDbCommit!(handle)

	dbRollback! : U32 => Try({}, Str)
	dbRollback! = |handle|
		Host.rocFxDbRollback!(handle)

	## Permissions (`golem:permissions`) and the principal: every one of these is a
	## synchronous host call, so nothing is awaited here. Cards are host-side
	## capabilities the app carries as `U32` handles.
	getPrincipal! : {} => Str
	getPrincipal! = |_|
		Host.rocFxGetPrincipal!({})

	permissionsWallet! : {} => Try(Str, Str)
	permissionsWallet! = |_|
		Host.rocFxPermissionsWallet!({})

	permissionsWalletVersion! : {} => Try(Str, Str)
	permissionsWalletVersion! = |_|
		Host.rocFxPermissionsWalletVersion!({})

	permissionId! : U32 => Try(Str, Str)
	permissionId! = |handle|
		Host.rocFxPermissionId!(handle)

	permissionParents! : U32 => Try(Str, Str)
	permissionParents! = |handle|
		Host.rocFxPermissionParents!(handle)

	permissionExpiresAt! : U32 => Try(Str, Str)
	permissionExpiresAt! = |handle|
		Host.rocFxPermissionExpiresAt!(handle)

	permissionInspect! : U32 => Try(Str, Str)
	permissionInspect! = |handle|
		Host.rocFxPermissionInspect!(handle)

	permissionDerive! : U32, Str => Try(U32, Str)
	permissionDerive! = |handle, boundsJson|
		Host.rocFxPermissionDerive!(handle, boundsJson)

	permissionDeriveFromWallet! : Str => Try(U32, Str)
	permissionDeriveFromWallet! = |boundsJson|
		Host.rocFxPermissionDeriveFromWallet!(boundsJson)

	permissionDeriveScope! : Str => Try(U32, Str)
	permissionDeriveScope! = |requestJson|
		Host.rocFxPermissionDeriveScope!(requestJson)

	permissionInstall! : U32, Str => Try({}, Str)
	permissionInstall! = |handle, holderJson|
		Host.rocFxPermissionInstall!(handle, holderJson)

	permissionRevoke! : U32 => Try(U32, Str)
	permissionRevoke! = |handle|
		Host.rocFxPermissionRevoke!(handle)

	permissionRelease! : U32 => Try({}, Str)
	permissionRelease! = |handle|
		Host.rocFxPermissionRelease!(handle)

	permissionListModules! : {} => Try(Str, Str)
	permissionListModules! = |_|
		Host.rocFxPermissionListModules!({})

	permissionValidateGrant! : Str => Try({}, Str)
	permissionValidateGrant! = |grantJson|
		Host.rocFxPermissionValidateGrant!(grantJson)
}
```

```roc
Host :: [].{
	rocFxLog! : U8, Str => {}
	rocFxGetWorkerId! : {} => Str
	rocFxCreateWebhook! : {} => Try(Str, Str)
	rocFxAwaitWebhook! : Str => Try(Str, Str)
	rocFxRpcInvoke! : Str, Str, Str => Try(Str, Str)
	rocFxRpcFireAndForget! : Str, Str, Str => Try(Str, Str)
	rocFxRpcSchedule! : Str, Str, Str, U64 => Try(Str, Str)
	rocFxRpcScheduleCancelable! : Str, Str, Str, U64 => Try(U32, Str)
	rocFxRpcCancel! : U32 => Try({}, Str)
	rocFxParseAgentId! : Str => Try(Str, Str)
	rocFxMakeAgentId! : Str, Str => Try(Str, Str)
	rocFxSetPersistence! : U8 => {}
	rocFxHttpRequest! : Str => Try(Str, Str)
	rocFxWsConnect! : Str => Try(U32, Str)
	rocFxWsSend! : U32, Str => Try({}, Str)
	rocFxWsReceive! : U32 => Try(Str, Str)
	rocFxWsClose! : U32 => Try({}, Str)
	rocFxSleepMillis! : U64 => {}
	rocFxNowMillis! : {} => U64
	rocFxGetConfig! : Str => Try(Str, Str)
	rocFxNewQuotaToken! : Str, U64 => U32
	rocFxQuotaReserve! : U32, U64 => Try(U32, Str)
	rocFxQuotaCommit! : U32, U64 => Try({}, Str)
	rocFxQuotaSplit! : U32, U64 => Try(U32, Str)
	rocFxQuotaMerge! : U32, U32 => Try({}, Str)
	rocFxGetRetryPolicies! : {} => Str
	rocFxGetRetryPolicy! : Str => Try(Str, Str)
	rocFxResolveRetryPolicy! : Str, Str, Str => Try(Str, Str)
	rocFxNextRetryDelay! : Str, Str, U64, Str => Try(Str, Str)
	rocFxSetRetryPolicy! : Str => Try({}, Str)
	rocFxRemoveRetryPolicy! : Str => {}
	rocFxTrap! : Str => {}
	rocFxIdempotencyKey! : {} => Str
	rocFxFork! : {} => Try(Str, Str)
	rocFxOplogCommit! : U8 => {}
	rocFxResolveAgentId! : Str, Str => Try(Str, Str)
	rocFxResolveComponentId! : Str => Try(Str, Str)
	rocFxKvGet! : Str, Str => Try(Str, Str)
	rocFxKvSet! : Str, Str, Str => Try({}, Str)
	rocFxKvDelete! : Str, Str => Try({}, Str)
	rocFxKvExists! : Str, Str => Try(Bool, Str)
	rocFxBlobCreateContainer! : Str => Try({}, Str)
	rocFxBlobPut! : Str, Str, Str => Try({}, Str)
	rocFxBlobGet! : Str, Str => Try(Str, Str)
	rocFxBlobHas! : Str, Str => Try(Bool, Str)
	rocFxBlobList! : Str => Try(Str, Str)
	rocFxBlobDelete! : Str, Str => Try({}, Str)
	rocFxReadFile! : Str => Try(Str, Str)
	rocFxWriteFile! : Str, Str => Try({}, Str)
	rocFxAppendFile! : Str, Str => Try({}, Str)
	rocFxFileExists! : Str => Try(Bool, Str)
	rocFxFileSize! : Str => Try(U64, Str)
	rocFxDeleteFile! : Str => Try({}, Str)
	rocFxMakeDir! : Str => Try({}, Str)
	rocFxListDir! : Str => Try(Str, Str)
	rocFxDurableStreamReader! : Str => Try(U32, Str)
	rocFxDurableStreamRead! : U32, Str => Try(Str, Str)
	rocFxDurableStreamWriter! : Str => Try(U32, Str)
	rocFxDurableStreamAppend! : U32, Str => Try(Str, Str)
	rocFxDurableStreamRelease! : U32 => Try({}, Str)
	rocFxDbOpen! : Str => Try(U32, Str)
	rocFxDbOpenMysql! : Str => Try(U32, Str)
	rocFxDbOpenIgnite! : Str => Try(U32, Str)
	rocFxDbQuery! : U32, Str, Str => Try(Str, Str)
	rocFxDbExecute! : U32, Str, Str => Try(U64, Str)
	rocFxDbBegin! : U32 => Try(U32, Str)
	rocFxDbCommit! : U32 => Try({}, Str)
	rocFxDbRollback! : U32 => Try({}, Str)
	rocFxGetPrincipal! : {} => Str
	rocFxPermissionsWallet! : {} => Try(Str, Str)
	rocFxPermissionsWalletVersion! : {} => Try(Str, Str)
	rocFxPermissionId! : U32 => Try(Str, Str)
	rocFxPermissionParents! : U32 => Try(Str, Str)
	rocFxPermissionExpiresAt! : U32 => Try(Str, Str)
	rocFxPermissionInspect! : U32 => Try(Str, Str)
	rocFxPermissionDerive! : U32, Str => Try(U32, Str)
	rocFxPermissionDeriveFromWallet! : Str => Try(U32, Str)
	rocFxPermissionDeriveScope! : Str => Try(U32, Str)
	rocFxPermissionInstall! : U32, Str => Try({}, Str)
	rocFxPermissionRevoke! : U32 => Try(U32, Str)
	rocFxPermissionRelease! : U32 => Try({}, Str)
	rocFxPermissionListModules! : {} => Try(Str, Str)
	rocFxPermissionValidateGrant! : Str => Try({}, Str)
}
```

### 3.2 The Rust half: the host

The platform's other half is a Rust library compiled to `wasm32-unknown-unknown`
and linked into the component as a static archive. It implements two things: the
Golem-facing exports (the agent protocol, the tool surface, the HTTP mount
handlers) and every host function the Roc code calls.

It is ~13,000 lines and is **not reproduced here** (see §0.4). This is the
inventory, so nothing is hidden:

| File | Lines | Role |
| --- | --- | --- |
| `host/src/lib.rs` | 7136 | The whole implementation: the `golem:agent/guest` handler (initialize, invoke, agent-type metadata, the mount), the `golem:tool/guest` surface (discover, get, invoke, streams), the router request handler, the durable-state holder, and each effect's host function. |
| `host/src/roc_platform_abi.rs` | 3905 | The Roc↔Rust ABI: the memory-ownership rules for Roc values crossing the boundary, and the shims for the platform's hosted functions. Generated from the platform definition, then hand-audited. |
| `host/src/roc_std.rs` | 724 | The Roc value representation as the host sees it — `RocStr`, `RocList`, `RocResult` — including how each is allocated and deallocated. Pinned to one Roc nightly; a version mismatch is refused at build time because a `Str` dropped through the wrong deallocator **traps at runtime**. |
| `host/src/rdbms_json.rs` | 1261 | The JSON shapes for the database interface (`golem:rdbms`). |
| `host/src/retry_json.rs` | 883 | The JSON shapes for retry policies. |
| `host/src/durable_stream_json.rs` | 719 | The JSON shapes for external durable streams. |
| `host/src/permissions_json.rs` | 715 | The JSON shapes for the permission-card surface. |
| `host/src/guest_bridge.rs` | 293 | The test bridge: in-process stand-ins so host logic can be unit-tested without a Golem server. |

The four `*_json.rs` files exist because Golem's structured effects — a
database row, a retry policy tree, a stream batch, a permission card — travel as
JSON text rather than as WIT records. Roc parses them with its built-in `Json`
codec, so an app sees ordinary values while the wire carries text.

**The ABI contract, in full.** Six symbols the host calls, and 83 it can call
back:

```text
provided (host → guest)    6   main_init_for_host, main_metadata_for_host,
                               main_handle_message_for_host,
                               main_handle_request_for_host,
                               main_handle_multimodal_for_host,
                               main_handle_tool_call_for_host

hosted  (guest → host)    83   rocFxLog! … rocFxPermissionValidateGrant!
```

### 3.3 How one call flows

Taking `start_canary` as the example, because it is the one the reference app
leans on hardest:

1. A caller runs `golem agent invoke 'canary-release("prod-1")' start_canary
   '{"version":"1.1.0","weight":10}'`. Golem routes it to the agent instance
   for that identity.
2. Golem calls the component's `golem:agent/guest.invoke` with the method name
   and the parameter.
3. The host (`lib.rs`) sees a name that is not `handle-message` and **is** in the
   app's declared method list, so it wraps the parameter in the envelope
   `{"method": "start_canary", "input": "..."}` and calls
   `main_handle_message_for_host`.
4. The platform (`main.roc`) parses that envelope, finds the handler whose
   declared name matches, and — for `register` — decodes the input JSON into the
   handler's annotated input type before calling it. The app's `handleStart!`
   therefore receives a real `StartInput` record, not text.
5. `handleStart!` reads config, logs, appends an event, writes the dashboard file,
   and schedules the safety timer. Every one of those is a `!` effect that
   crosses back into the host (step 3's 83 functions, in reverse).
6. It returns `{ state, out }`. The platform encodes `out` to JSON, encodes the
   state, and hands both back up; the host commits them to the invocation log and
   answers the caller with the typed output.

Two consequences of this path are worth a reviewer's attention. Because the
**state is a JSON string on every call**, an app's state must be
encode-and-decode-safe, and the platform derives both directions from the
state type (or asks the app for them, via `serializeState`/`deserializeState`).
And because **the method envelope is re-derived on every call**, a method added
to the app is reachable by the CLI and by RPC with no manifest change at all —
only for *tools* does the manifest need the two declarations of §2.6.

---

## 4. Plain-language explanations

### 4.1 What a Roc platform is, and what this one is

In Roc, an application does not link a library of functions. It declares a
**platform** — a package that the compiler treats as the host environment — and
the platform declares, in turn, what it needs from the app. This platform's
contract is one line (`platform/main.roc`):

```roc
platform "golem"
    requires {
        agent : _
    }
```

An app must therefore provide a value called `agent`. In exchange, the platform
provides everything else: the type of that value, the effects the app may call,
and the JSON codecs.

So the platform is the *entire* environment for a Roc wasm component here. The
interesting part is that the boundary between the two halves is **text, not
types**: every one of the six host entry points takes and returns JSON strings.
Roc's compiler cannot see across that boundary, so the platform's job is to make
the boundary invisible to the app — and the type annotations on an app's handlers
are what it uses to do so.

### 4.2 Why `->` and `=>` are not decoration

Roc has two function arrows, and this platform gives them different jobs.
Compare the two signatures an app binds a method with:

```roc
register         : Spec(state), Method, (state, input => Try({ state : state, out : output }, Str)) -> Spec(state)
registerReadOnly : Spec(state), Method, (state, input ->  output)                             -> Spec(state)
```

The *handler* in the first line is `=>` — effectful. It may log, write files,
call other agents, and it returns a new state alongside the output.

The handler in the second line is `->` — pure. It may not perform a single
effect, and it cannot return a new state at all. The type system does not let it.

This is not a style rule; it is the enforcement of Golem's read-only flag. In a
Golem agent, a read-only method promises the runtime it will not write, so the
runtime is free to cache the answer and to reject a write made from the call.
In most languages that promise is a comment. Here, a read-only method that tries
to write does not compile — see `handleStatus` in §2.4, which is spelled
`State, {} -> Status` and returns the state's fields rather than the state.

### 4.3 Abilities: where the JSON codecs come from

Roc derives codecs through **abilities** — a constraint on a type variable. The
signature of `register` continues:

```roc
where [
    input.Json.Parseable([InvalidJson(Str), ..]),
    output.Json.Encodable(_),
]
```

Read it as: “the input type must be parseable from JSON, the output type must be
encodable to JSON, and the compiler must be able to find out how.” It can,
because the app wrote the concrete types down — the handler's binding carries an
annotation:

```roc
handleStart! : State, StartInput => Try({ state : State, out : StartOutput }, Str)
```

That one line is the whole codec story. The platform decodes the caller's JSON
into `StartInput` and encodes the result out of `StartOutput`, so the handler
body deals in values. If the annotation were missing, the types would be
unresolved and the app would not compile.

The same mechanism encodes the durable state: `build` requires
`state.Json.Encodable(_)` and `state.Json.Parseable([InvalidJson(Str), ..])`.
This is the honest answer to the blueprint's `.{ encoder_for : _, parser_for : _ }`
slots: the compiler does derive the codecs, but the place it is told so is an
*ability bound*, not a placeholder field.

### 4.4 The tool surface, and the rule that surprises everyone

An app publishes tools by listing them in its metadata. The component then
exports `golem:tool/guest`, Golem discovers the tools, and they appear in
`golem tool list` and are invokable by `golem tool invoke`.

Three rules apply, and each one of them has cost someone a debugging cycle:

- **The published name is the CLI-shaped one.** An app writes
  `name: "analyze_logs"`; the tool surface publishes `analyze-logs`. The host
derives the second from the first by lowercasing letters and digits and turning
every other character into `-`.
- **The manifest must declare the tool twice.** Once at the top level (naming the
  component) and once bound on the agent type. Half of that pair is an error the
  *manifest validator* raises, before any upload: “Discovered tool has no
  matching top-level declaration”. §2.6 covers it.
- **A tool runs in a fresh instance, without the agent's state.** This is the one
  worth understanding. A tool call does not wake the caller's agent instance: it
  gets a new, stateless one. An app whose state is a plain JSON shape tolerant of
  `{}` works fine; an app whose state decodes strictly gets an honest failure
  rather than a fabricated answer. A stateful agent should be driven through the
  **agent method surface** instead — `golem agent invoke <id> <tool_name>`, which
  reaches the same handler *with* the state. The reference app in §2 publishes a
  tool and the suite pins both paths, because the difference is invisible until
  it bites.

### 4.5 Three ways to answer HTTP

An agent can be reachable over HTTP in three shapes, and the reference app uses
one of them:

| Shape | Declared as | Served by | Use it when |
| --- | --- | --- | --- |
| Filesystem mount | `httpMount` with `filesystemBindings` | The runtime, from the agent's durable filesystem | You want to serve a file the agent owns — provisioned from the manifest's `files:`, or, as in §2, written by the agent itself. A matching `GET` never invokes the agent. |
| Static router | `agentKind: HttpRouter` with `staticBindings` | The runtime, from deploy-time uploads | You want to serve a website whose content ships with the component. The agent type is ephemeral and parameterless; nothing of the app runs. |
| Dynamic router | `HttpHandlerOption: HandlesRequests` | The app, via `handleRequest!` | Your paths are computed — you need to inspect the request and answer with a status, headers and a body. Every request the static mappings do not match reaches the app. |

All three need the agent type listed in the manifest's HTTP API deployment
(§2.6). The reference app uses the first: its dashboard is written into its own
filesystem by the rollout methods and served from there, so what a reader sees
is the last state the release reached.

### 4.6 The distance from here to the peak state

One capability at a time, with the honest cost. “Mechanical” means the project
has done this class of change before and the pattern is documented; anything else
gets a warning.

| # | Gap | What closing it costs |
| --- | --- | --- |
| 1 | General ephemeral agents | **Medium.** A third agent kind beside `Regular` and `HttpRouter`, plus the host's agent-type construction for a stateless kind. The type-level work is the easy half; the part to think about is that an ephemeral agent has no state, so its method set differs and the app-facing `Agent` record needs a shape that says so. |
| 2 | Opaque secret handles | **Medium, and it is the one to do first.** Golem already exposes the `secret` resource; what is missing is surfacing it as a Roc handle so a host function can *use* a secret without the app holding its text. Closing it also closes the authenticated half of gap #5. |
| 5 | Method streams | **Medium.** Each of (b) and (c) is one more declared method plus one more hook — the same shape as `handleRequest!` and `handleMultimodal!`, both of which are done and documented — plus exposing the durable-session route options the platform does not build today. |
| 11 | Tool middleware | **Large, low value.** It needs an export and a component shape of its own; it cannot be reached from an agent-shaped component at all. Worth doing only if an app needs to police *another* component's tools. |
| 14 | Atomic regions | **Small, and currently declined on purpose.** It is the library-author half of `golem:durability`; everything this platform does is already recorded in the invocation log, so the wrapper adds ceremony without an app-level benefit. Re-open with a concrete use case, not speculatively. |
| 9 | Runtime reflection | **Out of reach from here.** A guest-side surface that enumerates an agent type's methods is not exposed to this platform; what exists is id resolution (`resolveComponentId!`, `resolveAgentId!`). If Golem exposes discovery, this becomes mechanical. **Flagged as an open question in §6, not as work to schedule.** |
| 12 | Calling another component's tools | **Medium-large.** Import `golem:tool/host` — the client half — plus the stream machinery the tool surface already uses for its writers. Note that calling another *agent* already works, so this is specifically about invoking tools rather than agents. |

Read together: two of the seven are deliberate decisions, one is outside this
platform's reach, and four are the ordinary “add an interface, a host function, an
effect, and a test” cycle that the platform's own documentation says is its
standard move. Nothing on this list is a rewrite.

---

## 5. What is verified, and how

Everything below runs on every change to the branch, and was observed for the
commit this document describes (`5e08694`). None of it is aspirational.

| Gate | What it is | Result |
| --- | --- | --- |
| End-to-end suite (`tooling/e2e.sh`) | Deploys every example to a **real local Golem 1.6 server**, invokes agents, drives tools, mounts, routers, webhooks, RPC, key-value and blobstore, the database surfaces, permissions, quota, retries, multimodal methods, external durable streams, snapshots across an *incompatible* revision, and the canary checks in §2 | **179 pass, 0 fail, 1 skip** (the skip is a real Postgres round trip, enabled by setting `E2E_DATABASE_URL`) |
| Host unit tests (`cargo test --lib`) | The Rust host's own logic: the ABI value codec, the state holder, the tool-stream plumbing, the router body chunks, the webhook table | **87 passed, 0 failed** |
| Platform tests (`roc test platform/main.roc`) | How the host's flat JSON payloads map onto `ToolCall` and `HttpRequest`, through the built-in `Json` codec | **16 passed** |
| Type check (`roc check`) | The platform and all nine examples | **0 errors** everywhere |
| Whole build (`./tooling/build.sh all`) | Compiles the platform and every example, and packages the release assets | **exit 0** |
| ABI harness smoke battery | Runs each built component **outside** Golem against stubbed host imports — metadata, initialize, message round-trips, tool calls, and every recorded effect call | Runs in CI and in the release workflow |
| CI (GitHub Actions) | Two jobs: the suites above, and the end-to-end job against a real server | green |
| Release workflow | Tagged build: unit tests, e2e, package, ABI battery, publish | green for `v0.6.0` |

**What is not covered, stated plainly:** every check targets a *local* Golem
server. Deployment to a hosted environment — its auth, its storage backends, its
network — is unverified here. Two example components (those importing the p3
`wasi:http` interface) cannot be instantiated by the ABI harness at all, so their
coverage is the end-to-end job alone. And the Rust host's 13,000 lines are not
quoted in this document, per §0.4.

**Checking this document rather than trusting it.** Every code listing carries its
repository path as a heading and was expanded mechanically from that file, so a
reader holding the repository can diff the two directly. The counts in §1.1 and
§1.3 are plain tallies of the tables above them. The two claims that are neither
a listing nor a suite check were each settled by direct experiment rather than
assertion: that a type module's associated items are public while its top-level
definitions are private (§2.2 — both directions probed), and that a tool call
runs on a fresh instance without the agent's state (§4.4 — a live invocation
against a local server, with the same behaviour pinned by the canary's
tool-surface check).

---

## 6. Questions for the reviewer

These are the decisions where an outside opinion is worth more than more code.
They are ordered by how much would change if the answer went the other way.

1. **Is the text boundary acceptable?** Structured effects — database rows,
   retry-policy trees, stream batches, permission cards — travel between Roc and
the host as JSON text, because they are not WIT records. It keeps the ABI flat
   and the Roc side typed, at the cost of parsing on both sides. Is that the right
   trade, or should these become typed WIT records?
2. **Should secret handles come next?** They are the one gap that blocks another
   gap (authenticated durable streams), and they are the difference between “an
   app can read a secret” and “an app can use a secret without holding it”. A
   reviewer's view on whether that is the right priority is the most useful thing
   this document can ask for.
3. **Is one agent type per component a real limit?** The reference blueprint
   wanted three components in one story. Here, one component holds one agent type,
   so that story needs three components. Is that a constraint an app can design
   around, or a wall?
4. **Is the state-as-a-JSON-string model sound at scale?** An invocation decodes
   the whole state, and a method parameter arrives as one buffered string. Durable
   streams fix transport, not app-side buffering. Where does that start to hurt?
5. **Should the atomic-region wrapper stay unexposed?** §4.6 records the current
   answer (yes, leave it out) and the reason (the invocation log already records
   everything). A reviewer who has built on Golem can say whether that reasoning
   holds.
6. **Is the original blueprint worth a correction?** It is confidently written and
   about a third of its Roc spelling is invented (§1.3). If others are working
   from it, correcting it may be more valuable than any single feature here.

Two standing requests, so a reviewer knows what to ignore: the platform is
pre-1.0 and its app-facing records have broken repeatedly (`agentKind`,
`httpMount`, `ToolResult`'s fields, `handleRequest!`, `handleMultimodal!`), with a
migration note each time; and the feature-by-feature inventory behind §1.1 is
kept in `docs/golem-1.6-parity.md`, checked against the WIT bundle this project
vendors — which is the authoritative list, not the SDKs' documentation.

