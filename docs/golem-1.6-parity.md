# Golem 1.6 parity: what the other SDKs expose, and where this platform stands

Status: inventory, 2026-10-20. "Parity" here means the surface the official
SDKs (Rust/TS/Scala/MoonBit) can reach on Golem 1.6, checked against the WIT
bundle this repo vendors (`wit/deps/`), which is the authoritative list.

## What this platform provides today

| Surface | Via | Notes |
| --- | --- | --- |
| agent protocol (`golem:agent/guest@2.0.0`) | platform exports | async exports, schema-value trees, agent types, tools, multimodal methods (item 5) |
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
| permissions + the principal | `golem:permissions/{types,inspect,derive,revoke,wallet,kernel-introspection}` | `getPrincipal!` and the whole card surface: `permissionsWallet!` / `permissionsWalletVersion!`, `permissionId!` / `permissionParents!` / `permissionExpiresAt!` / `permissionInspect!`, `permissionDerive!` / `permissionDeriveFromWallet!` / `permissionDeriveScope!`, `permissionInstall!` / `permissionRevoke!` / `permissionRelease!`, `permissionListModules!` / `permissionValidateGrant!`; grants and views as JSON, cards as `U32` handles (see item 6) |
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
5. ~~**Multimodal values** (`golem:core` text/binary/…): agent methods that take
   or return multimodal content, as the TS/Rust SDKs do~~ — **done**: an app that
   writes `multimodal: HandlesMultimodal` in its metadata registers a second
   declared method, `handle-multimodal`, whose input and result are a
   `list<variant{text: text-type, binary: binary-type}>` with the list node
   carrying `metadata.role = multimodal`, and answers it through the new
   `handleMultimodal!` hook. `handle-message` and every existing caller are
   untouched: an app that does not opt in registers the agent type it had. The
   hook is a breaking addition to the `Agent` record, like `handleRequest!` was.
   Details worth knowing:

   - **The envelope is the app's side of the conversion**, in both directions:
     `[{"tag":"text","value":{"text":…,"language":…}},
     {"tag":"binary","value":{"bytes":[…],"mimeType":…}}]`. It is Golem's
     public encoding (`golem-common/src/schema/external.rs`) *flattened*: that
     encoding wraps every value in `{"kind":…,"value":…}` and names a variant's
     case by index, while the envelope puts the modality in `tag` — the word this
     platform's other JSON vocabularies already use (`db-value`, the principal) —
     and drops the variant wrapper, so a hand-written Roc reader can walk it
     (there is no JSON parser in the language; the platform's convention is
     `Str.split_first`). The plan's example below said `tag` where `external.rs`
     says `kind`; the implementation follows the plan's spelling and the README
     documents the difference.
   - **What the runtime does with the schema was measured, not inferred.**
     `role: multimodal` is *not* required for a value to round-trip: with the
     role removed from the list node, the same `golem agent invoke …
     handle-multimodal` answers identically on a local 1.6.0 server, because the
     value encoding is the generic list/variant/text/binary one. The role is what
     the *tooling* layers read (the CLI's bridge generators, MCP's tool-input
     extraction, the registry's route compilation), and the runtime's placement
     validation uses it to refuse the shape in constructor scope. The route layer
     also **refuses a multimodal response in an HTTP API** (`Multimodal responses
     are not supported in http apis`), which is why the method declares no HTTP
     endpoint: a multimodal result cannot be served from a mount.
   - **The CLI's caller surface is a literal, not JSON.** `golem agent invoke`
     accepts a value for the method — `agent-type get`/the parse error render the
     declared parameter as `({ tag: "text"; value: text } | { tag: "binary";
     value: binary })[]` — but as the component's source-language literal, which
     for `source_language: "roc"` means the CLI's TypeScript dialect:
     `'[{tag: "text", value: Text("hi")}, {tag: "binary",
     value: Binary("data:image/png;base64,AQID")}]'`. The external JSON is the
     *wire* form the CLI builds from that literal, not something a caller types;
     the HTTP API's own JSON body codec (`from_untrusted_json_value`) is a third
     vocabulary and is not reachable here (no endpoint). The plan's expectation —
     that the CLI validates a caller's value against a multimodal schema — holds;
     the encoding it validates *into* is just not the caller-facing one.
   - **The refusals are app-visible errors, never traps**: on the way in a value
     tree that is not the parameter record and list, a part that is not a variant,
     a case the method does not declare, a payload the case does not carry, and a
     capability or `stream` node inside the list; on the way out a non-array
     answer, a missing or unknown `tag`, a missing or wrongly shaped payload, a
     byte that is not a `u8`, and an optional field that is neither a string nor
     `null`. Each message names the part it happened in.
   - **A method can opt out, and the combination the host cannot serve is
     refused.** `NoMultimodal` registers nothing, and a direct call to the hook is
     refused (`this agent type declares no multimodal method`) the way the OpenAPI
     provider method is refused on a router without a document;
     `HandlesMultimodal` on `agentKind: HttpRouter` is refused at registration
     (`a router cannot declare a multimodal method: routers register only their
     request handler and OpenAPI provider`) — the same shape as the
     `staticBindings`-on-a-regular-agent refusal.
   - **Coverage**: host unit tests for the declared graph, both conversion
     directions and every rejection; the ABI harness builds a caller's value tree
     from an envelope and reads the answer's tree back (`--multimodal`,
     `--expect-multimodal`, `--multimodal-shape-mismatch`,
     `--expect-multimodal-error`), wired into CI and the release workflow; and the
     e2e sends one text and one binary part through a real `golem agent invoke …
     handle-multimodal` and checks what the app read and what came back. The
     `counter` example is the worked case, and no new WIT import was needed
     (`golem:core/types@2.0.0` already carries every node type).
   - **Not exposed**: per-part restrictions (languages, length bounds, regexes,
     MIME allow-lists, byte bounds) — one platform-wide declaration cannot express
     an app's own — streaming parts (the method carries content, not streams), and
     any HTTP route for the method (see the refusal above).
6. ~~**Permissions** (`golem:permissions/{types,inspect,derive,revoke,wallet,kernel-introspection}`):
   cards, grants, and the principal the runtime passes to methods (we accept
   `Principal` and ignore it)~~ — **done**: all six interfaces are imports of
   `wit/world.wit` now, and the principal is rendered instead of ignored. The
   effects are `getPrincipal!` (the invocation's principal as JSON),
   `permissionsWallet!` / `permissionsWalletVersion!`, `permissionId!` /
   `permissionParents!` / `permissionExpiresAt!` / `permissionInspect!`, the three
   derivations, `permissionInstall!` / `permissionRevoke!` /
   `permissionRelease!`, and `permissionListModules!` /
   `permissionValidateGrant!`. Every function in the six interfaces is
   synchronous, so nothing here is async and the ABI harness covers the surface
   (see below). Details worth knowing:

   - **The structured values travel as JSON** in the WIT shape with camelCase
     names (`host/src/permissions_json.rs`, the `retry_json` / `rdbms_json`
     convention): a grant is
     `{"class":…,"owner":…,"recipient":…,"verb":…,"resourceId":…}`, a
     card view and a derivation's bounds are the four quadrants
     (`lowerPositive` / `lowerNegative` / `upperPositive` / `upperNegative`,
     plus `lifespan:{"iso8601":"P30D"}` or `null` for a persistent
     derivation, and `parents:[<handle>,…]` for `deriveScope!`, which takes no
     lifespan), a holder is `{"tag":"account"|"app"|"agent","val":…}`, and
     a `permission-error` becomes the text `<case-name>: <message>`
     (`not-permitted: …`, `lower-bound-too-broad: …`) the way the database and
     webhook errors are rendered. The README's "Permissions and the principal"
     section spells every shape out.
   - **The principal is JSON** in the shape the Rust SDK's principal serde uses:
     `{"tag":"anonymous"}`, `{"tag":"golem-user","val":{"accountId":…}}`,
     `{"tag":"agent","val":{"componentId":…,"agentId":…}}`,
     `{"tag":"oidc","val":{…}}` with every field the runtime supplies. The
     `agent` case flattens the WIT's nested `agent-id` record, which is the same
     object `resolveAgentId!` prints and a `{"tag":"agent"}` holder takes — one
     agent-id shape everywhere. A tool invocation has no principal (the tool ABI
     carries none), so `getPrincipal!` answers the JSON literal `null` there
     rather than inventing an identity.
   - **Cards are `U32` handles in the host's table**, like WebSocket, quota and
     database handles. Ownership is explicit: `id` / `parents` / `expires-at` /
     `inspect` / `derive` / `derive-scope` borrow a handle, `revoke` and
     `install` consume it (a second use is `unknown permission card handle N`,
     never a trap), and `permissionRelease!` — the one effect the interfaces have
     no equivalent for — drops the card, releasing the runtime's own entry, and
     frees the handle. Roc holds a plain number, so the host cannot see when an
     app is done with a card: an unreleased handle keeps its card alive for the
     instance's lifetime, which is documented in `Golem.roc` and the README.
   - **The runtime, not the platform, decides.** The e2e's permission checks are
     the runtime's own answers: the default card (this repo declares no
     `initialCard`, so every agent type gets the broad template card) in the
     wallet, a narrowing derivation accepted, one refused, a revocation, and an
     installation the runtime only allows onto agent targets. Two runtime
     behaviours worth recording: a `derive` request is checked against the
     parent's lattice and can be *conservatively* refused (a request that retains
     a grant the card carries still fails when the runtime cannot prove the child
     stays inside the parent — on the default card that is every `?agent`-owned
     class, `filesystem` included, while `?env` / `?env/*/*` / `?component` /
     `?account`-owned ones derive); and `validate-grant` checks *concrete*
     grants, so a slot (`?agent`) is an `invalid-pattern` there.
   - **Coverage**: the harness drives `permission roundtrip` on the counter
     against stubs that keep their own card table — wallet, inspect, id, parents,
     expires-at, a narrowing derivation, a refused one, a scope card, a revoke,
     a second revoke, a release and a second release, `validate-grant` both ways
     — and CI/release run it; the e2e runs the same commands against a real
     server, plus the principal from a CLI invocation (`golem-user`) and from an
     RPC one (`agent`, naming the caller). The counter's `permission` commands
     build their derivation requests from the card's own inspection, so they work
     against whatever card the runtime hands out.
   - **Not done, on purpose**: declaring an `initialCard` for one agent type in
     `golem.yaml` to assert a *denied* action. The counter needs its own
     filesystem, its tools, agent calls, its quota resource and its retry
     policies, so a narrow card there would have to grant all of that back and a
     mistake would surface as an unrelated check failing; a separate agent type
     that only reads its own filesystem would make the check clean. Recorded here
     as the follow-up; the enforcement check the e2e does make is the runtime
     refusing a derivation the default card cannot cover. Also not exposed:
     passing a card to another agent as a value (`schema-value-tree` carries no
     permission-card handle in this platform, the same gap as quota tokens) and
     lending a scope card to an awaited call.
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
