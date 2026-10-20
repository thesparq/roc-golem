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
| oplog + execution control | `golem:api/host` | `trap!`, `idempotencyKey!`, `fork!`, `oplogCommit!`, `resolveComponentId!` / `resolveAgentId!` — the part of the 1.5 surface 1.6 kept (see item 10) |
| tools | `golem:tool/guest` | CLI/MCP surface; stateless (fresh instance) |
| read-only methods | agent metadata | declared for tools; until-write cache |
| HTTP mounts | agent metadata | filesystem bindings (regular agents), static bindings + OpenAPI provider (routers) |
| logging, clocks, random | `wasi:logging`, `wasi:clocks`, … | `logInfo!` … `sleep!`, `now!` |

## Gaps, in rough order of how much a real app misses them

1. **Dynamic HTTP router handler** (`handle(request) -> response` with
   `stream<u8>` bodies). Everything but the response body is implemented on
   `wip/router-handler`; the body needs the export to deliver the response head
   before writing it (`task.return`-then-continue), which the wit-bindgen fork's
   generated export does not express. See `docs/router-handler-status.md` there.
   Cost: fork-level work.
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
   exists).
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
8. **Databases** (`golem:rdbms/{postgres,mysql,ignite2}`), **blobstore**
   (`wasi:blobstore`), **key-value** (`wasi:keyvalue`), **durability**
   (`golem:durability`). Cost: small each (a Roc effect per operation), but the
   Roc-facing API design matters: the SDKs expose typed clients.
9. **The agent's own filesystem**: the SDKs read/write files in the agent's
   durable filesystem; we can only *serve* files through mounts. Needs
   `wasi:filesystem` in the world plus effects. Cost: medium.
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
11. **Tool middleware** (`golem:tool/middleware`) and **tool streams**
    (`golem:tool/streams`, stdin/stdout/stderr for CLI-shaped tools). Cost:
    medium.
12. **RPC across components**: `rpc!` resolves the target through the calling
    component's own agent types, so cross-component calls fail with
    `Agent type not found` (documented in the README). Cost: unknown — needs
    server-side support or a different mechanism.

## How to close them

The pattern is established and cheap to repeat: add the interface to
`wit/world.wit`, write a host function that calls it (synchronous effects use
`wit_bindgen::block_on`, as `websocketReceive!` and `http!` do), declare the
effect in `platform/Effect.roc`, wrap it in `platform/Golem.roc`, then cover it
in the ABI harness (recording stubs) or the e2e (real server) — the suites and
the patterns for both already exist.

Items 1 and 2 are the two that are *not* mechanical: they need the stream/export
machinery discussed above.
