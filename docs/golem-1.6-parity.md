# Golem 1.6 parity: what the other SDKs expose, and where this platform stands

Status: inventory, 2026-10-20. "Parity" here means the surface the official
SDKs (Rust/TS/Scala/MoonBit) can reach on Golem 1.6, checked against the WIT
bundle this repo vendors (`wit/deps/`), which is the authoritative list.

## What this platform provides today

| Surface | Via | Notes |
| --- | --- | --- |
| agent protocol (`golem:agent/guest@2.0.0`) | platform exports | async exports, schema-value trees, agent types, tools |
| outgoing HTTP | `wasi:http/client@0.3.0` (p3) | buffered request/response |
| WebSockets | `golem:websocket/client` | connect/send/receive/close |
| config + secrets | `golem:agent/host.get-config-value` | `getConfig!`, `secret: true` declarations |
| agent-to-agent RPC | `golem:agent/host` wasm-rpc | `rpc!` (invoke-and-await), `rpcFireAndForget!`, `rpcSchedule!`, `rpcScheduleCancelable!` / `rpcCancel!`, `parseAgentId!` / `makeAgentId!` |
| webhooks | `golem:agent/host` + `golem:api/host` promises | `createWebhook!` / `awaitWebhook!`; the POST callback completes a durable promise |
| user-defined snapshots | `golem:api/save-snapshot`/`load-snapshot` | state JSON; e2e-covered across an incompatible revision |
| quotas + retry policies | `golem:quota/types`, `golem:api/retry` | `newQuotaToken!` / `quotaReserve!` / `quotaCommit!` / `quotaSplit!` / `quotaMerge!`, and `getRetryPolicies!` / `resolveRetryPolicy!` / `nextRetryDelay!` / … with the policy trees as JSON |
| key-value + blobstore | `wasi:keyvalue/eventual`, `wasi:blobstore` | `kvGet!` / `kvSet!` / `kvDelete!` / `kvExists!` and `blobCreateContainer!` / `blobPut!` / `blobGet!` / `blobHas!` / `blobList!` / `blobDelete!`; both stores are environment-scoped, and values travel as text |
| databases | `golem:rdbms/{postgres,mysql,ignite2}` | `dbOpen!` / `dbOpenMysql!` / `dbOpenIgnite!` / `dbQuery!` / `dbExecute!` / `dbBegin!` / `dbCommit!` / `dbRollback!`; connections and transactions are `U32` handles, statements' values travel as the JSON `db-value` encoding |
| oplog + execution control | `golem:api/host` | `trap!`, `idempotencyKey!`, `fork!`, `oplogCommit!`, `resolveComponentId!` / `resolveAgentId!` — the part of the 1.5 surface 1.6 kept (see item 10) |
| tools | `golem:tool/guest` | CLI/MCP surface; stateless (fresh instance); stdin/stdout/stderr as optional streams, with the app's output buffered (see item 11) |
| read-only methods | agent metadata | declared for tools; until-write cache |
| HTTP mounts | agent metadata | filesystem bindings (regular agents), static bindings + OpenAPI provider (routers) |
| the agent's own filesystem | `wasi:filesystem@0.2.6` (p2) | `readFile!` / `writeFile!` / `appendFile!` / `fileExists!` / `fileSize!` / `deleteFile!` / `makeDir!` / `listDir!`; per-agent and durable, provisioned from the manifest's `files:` list |
| logging, clocks, random | `wasi:logging`, `wasi:clocks`, … | `logInfo!` … `sleep!`, `now!` |

## Gaps, in rough order of how much a real app misses them

1. **Dynamic HTTP router handler** (`handle(request) -> response`) — **shipped,
   with the response body blocked upstream.** The handler is invoked for every
   request a router mount's file mappings do not match: the request is decoded
   from the canonical envelope (method, scheme, authority, path, query, ordered
   byte-valued headers, buffered body) and the app answers with a status,
   headers and a body. The head reaches the client and a bodyless answer is
   complete; **a non-empty body does not arrive on Golem 1.6.0**.

   The blocker is precise, and it is not the export machinery, not the head and
   not timing. A controlled comparison on one local server (this platform's
   router and the official Rust SDK probe side by side) measured:

   - the SDK probe delivers a body (35 bytes, clean framing); this platform
     answers 200 with 0 bytes and curl reports a truncated transfer;
   - this platform's oplog has **no `StreamItems` entry** — the chunk never
     reaches the invocation session — while the probe's has one between its
     `StreamSession` and `StreamEnd`;
   - queuing the write with `spawn_local`, a `yield_async` before it, and a
     300 ms wait before it (during which the oplog shows the output stream
     registering) all leave the body empty, and an inline write in the SDK
     probe deadlocks (504) while this platform's is accepted immediately — so
     the two "wrapped" streams are not the same thing on the host side;
   - the value-tree slot itself is fine: returning the **request's** stream (a
     host-created endpoint) as the response body delivers the client's own
     upload back (`POST /static/echo` → 200, `echo me`).

   So the transfer of a **guest-created** stream (`schema-value-stream.wrap`)
   into a returned value tree is what fails; the executor pipes that reader into
   an `accounted_output_stream_pair` live stream whose items never surface.
   `docs/router-handler-status.md` is the full record, and
   `docs/upstream-router-body.md` is the report, filed upstream as
   [golemcloud/golem#4291](https://github.com/golemcloud/golem/issues/4291). Two ways
   forward: a host-created body endpoint (the shape
   `golem:tool/host.create-output` already has for tools, and the one this
   platform has demonstrated it can deliver), or executor-side instrumentation
   of the `wrap`/pipe path. The platform logs a warning when an app's body is
   dropped, so nothing changes for an app once either lands.
2. **Durable streams** (`golem:agent/durable-streams`): append/read into host
   storage, streaming method inputs/outputs, and the HTTP streaming endpoints the
   SDKs derive from them. Our platform's methods are string-in/string-out.
   Cost: large — needs stream-typed schema values on both sides plus a Roc-facing
   API.
3. ~~**Agent-to-agent calls beyond invoke-and-await**~~ — **done**: effects
   `rpcFireAndForget!` (`invoke`), `rpcSchedule!` (`schedule-invocation`),
   `rpcScheduleCancelable!` + `rpcCancel!` (cancelable scheduling), and
   `parseAgentId!` / `makeAgentId!`, all in `golem:agent/host` as expected. Two
   details worth knowing: the scheduling effects take a delay in milliseconds
   rather than an absolute `instant`, so the host reads the wall clock
   (`wasi:clocks/system-clock@0.3.0`, added to the world) and converts the delay
   into the absolute time the host API takes; and the cancelable receipt's
   `metadata` is not surfaced — the
   effect returns the cancellation handle only, the way the WebSocket effects
   return handles. All of them are e2e-covered against a counter in another
   component. Still not exposed: `async-invoke-and-await` (`future-invoke-result`)
   and `invoke-and-await`'s `invocation-result-with-metadata` (our `rpc!` drops
   the metadata and returns the reply text).
4. ~~**Webhooks** (`golem:agent/host.create-webhook`)~~ — **done**: effects
   `createWebhook!` / `awaitWebhook!`. `createWebhook!` creates the promise
   (`golem:api/host.create-promise`) and the webhook for it, and returns the
   callback URL; the promise id stays host-side, in a per-instance map keyed by
   the URL, so the app stores the URL in its own state between invocations.
   `awaitWebhook!` awaits the POST (`get-promise-result.get`, an async import
   driven with `wit_bindgen::block_on`) and returns the body as lossy UTF-8.
   The URL is `<scheme>://<domain>/<webhookUrl>/<agent-type-name>/<id>` (the
   mount's `webhook-suffix` is still empty, so the agent type name is the
   suffix); the route is POST only, unauthenticated, answers 204, and 404s an
   id it cannot verify. Golem only allows it for agent types deployed through
   an HTTP API, so the effects agent joined `golem.yaml`'s deployment with a
   binding-less mount, and the e2e covers create → POST → await plus the 404.
   Not exposed: the mount's `webhook-suffix` (adding a field to `HttpMount`
   would break every existing `Mount({…})` literal).
5. **Multimodal values** (`golem:core` text/binary/…): agent methods that take
   or return multimodal content, as the TS/Rust SDKs do. Cost: medium — schema
   graph construction (the router work already builds a graph, so the pattern
   exists). **Design decided, not implemented** — the plan below is what an
   implementer should follow; it was checked against the 1.6.0 sources.

   - **The surface.** This platform declares one string-in/string-out method
     (`handle-message`), so multimodal content has nowhere to travel: the
     runtime builds a caller's input from the *declared* schema, and a string
     schema can only ever produce a string node. The smallest honest fix is a
     **second declared method** whose input and output schemas are multimodal —
     `handle-multimodal` — leaving `handle-message` (and every existing caller)
     untouched. The app reaches it through a new `handleMultimodal!` hook, the
     same kind of documented break `handleRequest!` was for routers.
   - **The schema.** A `list<variant{text: text-type, binary: binary-type}>`
     whose *list* node carries `metadata.role = multimodal` (the SDKs set the
     role on the list node, not the variant — `MultimodalAdvanced` in the Rust
     SDK, `multimodal.rs` in `golem-rust`). `text-type` takes
     `text-restrictions` (`languages`, length, regex) and `binary-type` takes
     `binary-restrictions` (`mime-types`, byte bounds); both may be empty. The
     existing `router_schema_graph()` in `host/src/lib.rs` is the pattern for
     building the graph by hand.
   - **The caller's JSON** is Golem's public/external schema encoding
     (`golem-common/src/schema/external.rs`, tag `v1.6.0`): every value is a
     tagged object, e.g. `[{"tag":"text","value":{"text":"hi",
     "language":null}},{"tag":"binary","value":{"bytes":[1,2,3],
     "mimeType":"image/png"}}]`. `union` values carry their tag explicitly at
     this layer, so no discriminator inference is needed here.
   - **The conversion** the host owes the app is between that tree and a
     documented JSON envelope on the Roc side (the same envelope the caller
     used is the obvious choice: a list of `{tag, value}` objects), in both
     directions, plus the failure cases (unknown tag, payload shape mismatch,
     a stream or capability node inside the list — which this method must
     reject).
   - **Open questions for the implementer to settle first**: whether the CLI's
     `golem agent invoke` and the HTTP API validate a caller's JSON against a
     multimodal schema (they do for other schemas, so the expectation is yes,
     but confirm it against the pinned CLI), and whether the runtime requires
     `role: multimodal` to accept the value or merely uses it for tooling.
   - **Test plan.** The conversion is synchronous, so the ABI harness covers it
     with a recording stub (send a multimodal tree in, assert the app's JSON
     envelope; answer with an envelope, assert the tree out; one rejected
     case). The e2e adds a real `golem agent invoke … handle-multimodal
     '[…]'` round trip against the server. No new WIT import is needed:
     `golem:core/types@2.0.0` is already imported and carries every node type.
6. **Permissions** (`golem:permissions/{types,inspect,derive,revoke,wallet,kernel-introspection}`):
   cards, grants, and the principal the runtime passes to methods (we accept
   `Principal` and ignore it). Cost: medium-large.
7. ~~**Resource quotas** (`golem:quota/types`) and **retry policies**
   (`golem:api/retry`)~~ — **done**: the five quota effects (`newQuotaToken!`,
   `quotaReserve!`, `quotaCommit!`, `quotaSplit!`, `quotaMerge!`) and six retry
   effects (`getRetryPolicies!`, `getRetryPolicy!`, `resolveRetryPolicy!`,
   `nextRetryDelay!`, `setRetryPolicy!`, `removeRetryPolicy!`), with both
   interfaces added to `wit/world.wit`. Tokens and reservations travel as `U32`
   handles in the host's handle table, the way WebSocket connections do. Policies,
   predicates and decisions are flattened trees, so they travel as JSON text
   (`host/src/retry_json.rs` mirrors them in both directions: camelCase WIT names,
   durations as nanosecond numbers, predicate values as plain JSON
   strings/numbers/booleans, the root at `nodes[0]`); the shape is spelled out in
   the README. Both interfaces are synchronous, so the ABI harness covers the
   quota path and a two-step retry walk with recording stubs — that check runs on
   the counter component in CI and the release workflow — and the e2e covers them
   against a real server: a resource that commits one reservation and rejects the
   next past its capacity, and a manifest-declared policy that is listed, resolved
   and walked twice. Two details about a live environment worth knowing: besides
   the policies a manifest declares it carries a `default` rule (`predTrue`, the
   lowest priority), so `resolveRetryPolicy!` only answers `""` when the
   environment has no rules at all; and `quotaCommit!` returns `Try({}, Str)`
   rather than `{}`, so a handle that was already spent is reported the way
   `websocketClose!` reports one (`getRetryPolicy!` reports a missing name as
   `""`, the same way `resolveRetryPolicy!` reports no match). The
   `resourceDefaults` / `retryPolicyDefaults` in `golem.yaml` are declared for the
   `local` environment only, which is the one this repo's e2e and `golem -L
   deploy` use, and a resource's capacity is environment-level and durable rather
   than per agent — the e2e reports that one check as `SKIP` when a rerun finds
   the data directory's capacity already spent. Not exposed: passing a quota token
   to another agent. The SDKs send tokens through schema values, and
   `schema-value-tree` in this platform carries no resources.
8. ~~**Blobstore** (`wasi:blobstore`), **key-value** (`wasi:keyvalue`) and
   **databases** (`golem:rdbms/{postgres,mysql,ignite2}`)~~ — **done**: the two
   stores are imports of `wit/world.wit` now, and the effects are the data plane
   per store: four key-value effects (`kvGet!` / `kvSet!` / `kvDelete!` /
   `kvExists!` on `wasi:keyvalue/eventual`) and six blobstore ones
   (`blobCreateContainer!` / `blobPut!` / `blobGet!` / `blobHas!` / `blobList!`
   / `blobDelete!`), with values as bytes on the wire travelling as lossy-UTF-8
   text, the convention `http!` uses; buckets, keys, containers and objects are
   named by strings, so — unlike the SDKs' typed clients — no handle travels
   through the app. A missing key is the JSON literal `null` (`kvExists!` is
   the direct check), which is what tells it from an empty value; the one value
   it cannot be told from is stored text that is itself `null`, documented in
   `platform/Golem.roc` and the README. Both interfaces are synchronous, so the
   ABI harness drives a full round trip of each against recording stubs that
   keep their own small store (the counter example carries `kv` and `blob`
   commands for it, and CI/release run them); the blobstore's `list-objects` is
   the one call whose `stream<object-name>` the host drains itself, into the
   JSON array `blobList!` returns. The e2e covers both round trips against a
   real server. Not exposed: `wasi:keyvalue/cache` (async results) and
   `eventual-batch`, and on the blobstore side `delete-container`,
   `container-exists`, `copy-object`, `move-object`, `delete-objects`, `clear`
   and the `info` readers.

   The **databases** half landed with eight effects on the three interfaces:
   `dbOpen!` (postgres), `dbOpenMysql!`, `dbOpenIgnite!`, `dbQuery!`,
   `dbExecute!`, `dbBegin!`, `dbCommit!`, `dbRollback!`. Details worth knowing:

   - **One `open` per engine, one shared statement surface.** Each interface has
     its own `open` and its own `db-value` / `db-column-type` vocabulary, so the
     platform names the three opens after the interface each calls and shares
     one query/execute/transaction family across them. A single `dbOpen!`
     taking a kind plus an address would have invented a discriminator the WIT
     does not have (and a new Roc type to carry it); the three effects mirror
     the WIT one-to-one, the way every other effect on this platform does.
   - **Connections and transactions are `U32` handles in one table**, tagged by
     engine and kind: `dbQuery!` / `dbExecute!` take either, a handle of the
     wrong kind is reported rather than misread, and `dbCommit!` /
     `dbRollback!` consume the transaction's handle, so using one twice is
     `unknown database handle N` (the shape `wsClose!` and `quotaCommit!` use).
   - **Values travel as the WIT structure in JSON** (`host/src/rdbms_json.rs`
     mirrors all three vocabularies in both directions, the way `retry_json`
     mirrors the retry trees): a value is `{"tag": …, "value": …}` with the WIT
     case names in camelCase, a record is an object with camelCase fields, a
     nested variant (`ip-address`, a range bound) is a tagged object, a
     `lazy-db-value` is what it wraps; a `uuid` (and Ignite's `db-uuid`) is the
     canonical UUID string and a non-finite float is `"NaN"` / `"Infinity"` /
     `"-Infinity"`, the two hand-mapped cases. `dbQuery!` answers
     `{"columns": …, "rows": …}` — a row is the JSON array of its values in
     column order — with each column's ordinal, name, WIT type and the
     database's own type name (Ignite's columns carry no type: its `db-column`
     has none). The README spells the encoding out with examples.
   - **Opening is parse-only and the pool is lazy**, which is what makes the
     honest no-database checks possible: a well-formed address opens with no
     server behind it, a malformed one is the `connection-failure` variant of
     the effect's `Err` (not a trap), and the first statement is where the pool
     is created — the e2e asserts all three, and the real round trip
     (create/insert/select/count, then a rolled-back and a committed
     transaction) is opt-in through `E2E_DATABASE_URL` because it writes to the
     database it names. One ABI detail the round trip caught: `dbBegin!` has to
     look a handle up *and* store another under one borrow of the handle table,
     or the second `borrow_mut` panics inside wasm.
   - Both the harness and the e2e cover it: the harness drives the whole round
     trip on the counter against a stub that keeps its own little relational
     store — the params and the rows really go in and out of it, and the
     recorded calls show which host functions ran — plus the three `open`s and
     the bad-address path, and CI/release run those; the e2e runs the three
     opens and the no-server failure path against a real deployment.
   - Not exposed: `query-stream` (a result stream is a resource an app reads
     chunk by chunk, and the effects hand an app text). The old item also named
     `golem:durability`, which is now its own item below.
9. ~~**The agent's own filesystem**~~ — **done**: eight effects
   (`readFile!` / `writeFile!` / `appendFile!` / `fileExists!` / `fileSize!` /
   `deleteFile!` / `makeDir!` / `listDir!`) over `wasi:filesystem/types@0.2.6` +
   `preopens@0.2.6`, which `wit/world.wit` now imports. **p2, deliberately**:
   the p2 descriptor calls are synchronous — POSIX-shaped `read` / `write` /
   `open-at` / `stat-at` / `create-directory-at` / `unlink-file-at`, plus a
   `directory-entry-stream` resource read entry by entry — so the host makes
   plain calls (no `block_on`, no streams) and the ABI harness can drive the
   effects against an in-memory stub, the same reason quota/retry,
   execution-control and the two stores landed with harness coverage. The p3
   revision (`wasi:filesystem@0.3.0`, `async func`s over component-model
   streams, the shape the MoonBit SDK uses) stays open for whoever needs the
   async/stream shapes; the two revisions can coexist in the world, so nothing
   here forecloses it. Details worth knowing:

   - **Paths are resolved against the preopened root before the host is asked**:
     a leading `/` names the root (the spelling the manifest's `targetPath`
     uses), `.` and empty segments are dropped, `..` steps back inside the root,
     and a path that would step above it fails with `escapes the agent's
     filesystem root`. That is what makes `/seed/a.txt` and `seed/a.txt` the
     same file — the p2 interface itself refuses a leading `/`.
   - Writes are recorded in the invocation's oplog and replayed by the runtime,
     so the filesystem is durable across invocations and participates in
     snapshots; the manifest's per-agent `files:` list provisions the initial
     files. HTTP mounts *serve* files out of the same filesystem — a different,
     host-side surface, covered by the README's mount section.
   - `listDir!` reports each entry as `{name, type}` (`file` / `dir` /
     `symlink` / `other`), sorted by name; the host drains the p2 entry stream
     itself. The path resolution, the listing JSON and the read loop (a p2
     `read` answers one chunk plus an end-of-file flag, and Golem caps a chunk
     at 64 KiB) are unit-tested in `host/src/lib.rs`.
   - `fileSize!` returns `Try(U64, Str)`, the one result shape the platform did
     not use before, and the hand-written ABI spells it out itself
     (`RocU64Result`): the compiler puts a tag union's tag directly after the
     payload region, which on a 32-bit target is 12 bytes for `Try(U64, Str)`
     where Rust's own union would pad the payload to 16 and move the tag. The
     harness and the e2e both check the number, which is how the first (wrong)
     layout was caught.
   - Covered by the ABI harness (against the counter: write in one invocation,
     read it back in the next, `fs size`, `fs mkdir` + `fs list`, with the
     recorded calls checked) and by the e2e (the manifest-provisioned file read
     back, the cross-invocation write → read, append, exists/size, listings,
     mkdir/delete and the path rule both ways).

   Not exposed: `rename-at`, `symlink-at`, `readlink-at`, `remove-directory-at`,
   the metadata-hash readers, timestamps, and the stream-shaped descriptor
   operations (`read-via-stream` / `write-via-stream` / `append-via-stream`).
10. ~~**Oplog/execution control**~~ — **done**: six effects on `golem:api/host`,
    which was already imported: `trap!` (`trap`, never returns — the invocation
    fails with the reason), `idempotencyKey!` (`generate-idempotency-key`, as a
    UUID string), `fork!` (`fork`, as `Try(Str, Str)` with
    `"original:<phantom-id>"` / `"forked:<phantom-id>"`), `oplogCommit!`
    (`oplog-commit`), `resolveComponentId!` and `resolveAgentId!` (the two
    resolvers; the second returns `{"componentId": …, "agentId": …}`). They are
    synchronous host calls, so no `block_on` — and the ABI harness drives them:
    the counter example carries `trap`, `oplog-commit`,
    `resolve-agent-id` and `resolve-component-id` commands, whose replies (and
    recorded host calls) the harness checks against stubs, while the e2e runs the
    same commands against a real deployment plus `fork!` and `idempotencyKey!` on
    the effects agent.

    Three things worth knowing about the surface itself:

    - **Most of the 1.5 execution-control interface is gone in 1.6, on purpose**
      (`wit/deps/golem-1.x/golem-host.wit` has none of them, and
      `docs/src/content/next/develop/durability.mdx` in
      [golem-verify](https://github.com/golemcloud/golem), tag `v1.6.0`, says
      why): `get-oplog-index` and `set-oplog-index` (guests no longer read or
      move the execution cursor), `mark-begin-operation` and
      `mark-end-operation` (no guest-defined oplog markers; the runtime repairs
      an incomplete call at its own `Start` instead of rerunning a group of
      completed effects), and `get-idempotence-mode` / `set-idempotence-mode`
      (no guest switch that declares host calls idempotent; a remote write has to
      be made duplicate-safe at the peer, and retry predicates must not use
      `is-idempotent`). Do not try to expose them.
    - `idempotencyKey!` is **per call position**, not per invocation: Golem
      derives the key from the call's oplog index and records it with the
      invocation's outcome, so two calls in one invocation return two different
      keys, a later invocation is a new sequence, and a replay of the invocation
      hands the recorded keys back. The e2e asserts exactly that (`agree=false`
      for the two keys of one invocation, the recorded answer for a re-invocation
      under the same key, and a different `previous-first` after a new one).
    - `update-agent`, `revert-agent` and `get-agent-metadata` are still in the
      interface but stay operator-facing (the CLI does them; a trapped agent's
      failed invocation is reverted with `golem agent revert`), so the platform
      does not expose them.
11. **Tool streams** (`golem:tool/streams`, stdin/stdout/stderr for
    CLI-shaped tools) — **done**; **tool middleware**
    (`golem:tool/middleware`) — **not attempted, on purpose** (see below).

    The streams half: every command `build_tools` emits now declares stdin,
    stdout and stderr as `stream-spec`s with `required: false`, which is what
    makes the runtime attach one — it refuses an attachment a command does not
    declare, and requires one it declares `required`. `golem:tool/guest.invoke`
    drains the stdin stream into text *before* calling the app (`collect`, the
    export is async, so the await is direct; a failed item is reported as an
    invalid input rather than handed to the app as a truncated pipe), and writes
    the app's answer through the writers afterwards. The app-facing records
    carry the text: `ToolCall.stdin` in, `ToolResult.stdout` / `.stderr` out.
    Four details worth knowing:

    - **The app's output is buffered, not streamed.** The writers are written
      once, after `handleToolCall!` returns; true incremental output needs an
      output channel the app can write to while the call is in flight, which the
      `Agent` interface cannot express yet. That is the follow-up this item
      leaves open (see the README).
    - **`InvocationResult.stdout`/`stderr` stay absent** and the bytes go
      through the `tool-output-writer`s only, because the executor rejects a
      guest that returns attachment streams instead of using them
      (`materialize_guest_tool_response` in `golem-worker-executor`,
      `durable_host/tool/mod.rs`, tag `v1.6.0`). Each writer is `finish`ed even
      when the app wrote nothing, so a reader is never left on a writer the
      guest abandoned; a `ConsumerCancelled` write failure is not an invocation
      failure (the reader went away), and any other write/terminal failure is
      logged rather than failing a call whose result is still valid.
    - **It is an API break**, handled explicitly: `ToolResult` gained `stdout`
      and `stderr`, so every existing `handleToolCall!` that constructs one needs
      the two fields (`""` is the old behaviour). `ToolCall` gaining `stdin`
      breaks nothing — apps read it or ignore it. The README carries the
      migration note. There is no cheaper way to do it: the app has to be able
      to *answer* with output text, and a record type that does not name the
      fields cannot carry it.
    - **Stdin is read verbatim up to the next double quote**, because it travels
      through the same minimal JSON field reader `arguments` uses; documented as
      a caveat rather than fixed here.

    Covered by the harness (the `golem:tool/streams` stubs now supply stdin bytes
    and record what the component writes through both writers, with the writer's
    terminal — a writer left open reads `abandoned`; `--tool-stream` on the
    ai_tool example drives it, and CI/release run it) and by the e2e (piping
    `piped stdin bytes` through `golem tool invoke --stdin - --stdout --output …
    --stderr --stderr-output …` and checking both files).

    The middleware half is **not** attempted, and cannot be reached from this
    platform's shape: middleware is a *separate component shape*. The executor
    binds the world `golem:tool/tool-middleware-runtime` (not `tool-guest`) for
    it, and the registry recognises a middleware component by the interface it
    exports — `tool-middleware-guest`
    (`discover-tool-middlewares`, `get-tool-middleware`,
    `invoke-tool-middleware`), which an agent component does not export. Adding
    it means a second export on every component built from this platform, plus
    the surface that export needs: the runtime-owned `underlying-tool`
    capability (an async `invoke` returning a result resource plus two output
    streams, i.e. output that *is* incremental — the very thing the buffered
    writer above cannot express), `typed-schema-value` in and out, and the
    `tool-middleware` metadata (`parameter-schema`, monomorphic scope with a
    presented and an expected tool). Cost: large, and it buys nothing for an app
    that just wants its own tools to read stdin and write stdout.

    Adjacent, and also not exposed: the *client* half of the tool surface,
    `golem:tool/host` (`get-all-tools` / `get-tool`, and `tool-rpc`'s
    `invoke-and-await` / `invoke` / `async-invoke-and-await` with
    `create-stdin` / `create-output`). An agent can publish and answer tools,
    but cannot call another component's tool from Roc — a new interface to
    import plus the same stream machinery the writers above use. That is a gap
    of its own, not one of item 11's two halves.
12. ~~**RPC across components**~~ — **works on 1.6**: `parse-agent-id` falls back
to the environment's registered agent types, so `rpc!` resolves an agent in
another component, and item 3's e2e pins it (the effects agent calls a counter
in a different component and the reply crosses back). The README's older
"cross-component calls fail with `Agent type not found`" note was wrong for
1.6 and is gone.
13. **Durability** (`golem:durability`): the interface that carries
    `persist-remote-write`-style markers — the other half of the old item 8,
    which is now its own gap. **Decision: leave unexposed.** It is the
    library-author API for wrapping a group of effects in one typed durable
    call, and everything this platform does is already oplog-recorded for free;
    exposing it would mean schema-value request/response construction plus the
    flattened resource lifecycle for no app-level benefit. Recorded here so the
    next inventory does not re-open it without a concrete use case.

## How to close them

The pattern is established and cheap to repeat: add the interface to
`wit/world.wit`, write a host function that calls it (synchronous effects use
`wit_bindgen::block_on`, as `websocketReceive!` and `http!` do), declare the
effect in `platform/Effect.roc`, wrap it in `platform/Golem.roc`, then cover it
in the ABI harness (recording stubs) or the e2e (real server) — the suites and
the patterns for both already exist.

Items 1 and 2 are the two that are *not* mechanical: they need the stream/export
machinery discussed above. Item 11's middleware half is in the same class — it
needs a second export and a component shape of its own (see item 11).
