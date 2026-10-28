# Production readiness

A one-page state of play for using this platform for real applications: what is
verified, what to decide before launching, and what is known to be missing. The
feature-by-feature inventory is `docs/golem-1.6-parity.md`; this page is the
summary for someone deciding whether to build on it.

Pinned: **Golem 1.6.0 GA**, Roc nightly **`nightly-2026-10-27-de79a0f`** (the ABI
is tied to it; `tooling/build.sh` refuses a mismatched `roc`), and the platform
release you deploy against.

## Verified against a real Golem server, on every change

167 end-to-end checks, 87 host unit tests, two ABI-harness blocks, and two CI jobs
(platform verification plus a real-server e2e) run on every landing. What those
cover, end to end rather than in unit tests:

- durable agent state across invocations, and snapshots across an *incompatible*
  revision (the `counter_v2` update test);
- the tool surface (`golem:tool/guest`, what `golem tool invoke` and MCP drive),
  including stdin/stdout/stderr streams;
- agent-to-agent calls: RPC, fire-and-forget, scheduled, cancelable, across
  components, plus the agent-id helpers;
- webhooks (create, POST, await, the 404 for an unverifiable id);
- HTTP: file mounts from an agent's own filesystem, static bindings with an
  immutable ETag, the OpenAPI provider, and the **dynamic router handler** with
  the statuses, headers and bodies an app computes (bodies of any size);
- key-value and blobstore round trips through the environment's own stores;
- PostgreSQL, MySQL and Ignite opens and queries, plus transactions and a
  connection-failure path;
- the agent's own filesystem (provisioned files, writes read back later,
  listings, and the path rule);
- execution control: `trap!`, idempotency keys, `fork!`, `oplogCommit!`, and both
  resolvers;
- permissions: the wallet, inspection, derivations (accepted and refused),
  revocation, installation, and — on a component with its own narrow
  `initialCard` — the runtime *enforcing* it (a granted read works; a write
  outside the granted root and a connect to an unnamed host are refused as
  effect errors, not traps);
- quota reservations, retry-policy resolution and walking;
- multimodal method inputs (a declared list-of-parts method);
- external durable streams against the protocol's reference server;
- configuration and secrets (as values).

## Decide before you launch

1. **Cut a release and pin it.** Applications consume a platform by pinned
   release asset; the whole Golem 1.6 line is currently unreleased, so nothing in
   it can be pinned yet. This is a `main` landing plus a tag.
2. **Settle the update story.** The manifest's `tools:` declarations pin
   component revisions through *tool releases*, so deploying a **changed**
   tool-bearing component into an environment that already has a deployment is
   refused (`409 Conflict ... COMPONENT_IN_USE`). Either each release goes to a
   fresh environment or Golem's intended tool-release update path has to be used.
   Unresolved, and the reason a platform change currently needs a new environment
   to verify.
3. **Test against a real cloud environment.** Every check in this repo targets a
   *local* Golem server. Deployment to a hosted environment, its auth, and its
   storage backends are unverified here. Add a staging deploy job (Golem
   credentials as repository secrets) rather than discovering it in [REDACTED].
4. **Declare `initialCard`s** for anything that handles untrusted input. An agent
   type that declares none gets Golem's broad default card by design;
   `examples/permissions` shows the shape and the e2e proves enforcement.
5. **Keep the platform's suites green in your fork.** They are what stands
   between a Golem or Roc bump and a silently broken ABI; they caught a
   `Try(U64, Str)` layout error and a malformed value-tree chunk that produced an
   empty body with no error anywhere.

## Known limitations, and what each would take

- **Tool output is buffered**: written after the handler returns, not while it
  runs. Incremental output needs an output channel the `Agent` interface cannot
  express.
- **Streaming method inputs/outputs** and the SDKs' derived HTTP streaming
  endpoints are not exposed. The machinery works (a router response body is a
  guest-created stream written from the handler); the surface — a declared method
  with a stream parameter or result, plus the durable-session route options — is
  what is missing.
- **Authenticated external durable streams** need a secret-*handle* surface,
  which does not exist yet (`getConfig!` returns a secret's value, not a
  capability). In flight.
- **Tool middleware** is a separate component shape; not supported, deliberately.
- **One agent type per component** — several types means several components.
- **Secrets reach the app as values.** Prefer host-mediated paths, and treat any
  secret an app reads as being in app space.
- **Whole-input buffering**: a method input is one JSON string in memory, so very
  large inputs cost memory (durable streams fix transport, not app buffering).
- **Pre-1.0 churn.** The app-facing records have broken repeatedly
  (`agentKind`/`httpMount`, `ToolResult` fields, `handleRequest!`,
  `handleMultimodal!`). Each change carries a migration note, but budget a code
  pass per platform upgrade.
- **Components importing p3 `wasi:http`** (the effects agent, the permissions
  example) cannot be instantiated by the ABI harness, so their coverage is the
  e2e job.

## Operating it

- **Storage**: Golem retains durable-stream history and forks until the agent's
  data is deleted; there is no retention horizon. Plan capacity if you use
  durable streams or forks heavily. Gateways and executors take their own limits
  (stream sizes, live readers per stream, per-node ceilings) — see Golem's
  operations docs.
- **Local verification** of a platform change needs a fresh data directory (see
  the update story above). Seed `<dir>/data/builtin-artifacts` from a warmed data
  dir to skip the ~290 MB built-in-tool download, which also occasionally fails
  mid-download.
- **Observability**: the platform logs through `wasi:logging` under the
  `roc-agent` target; everything else is Golem's own (oplog, metrics, tracing).
  There is no platform-side metrics surface.
