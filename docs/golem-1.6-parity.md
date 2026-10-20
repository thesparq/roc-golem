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
| user-defined snapshots | `golem:api/save-snapshot`/`load-snapshot` | state JSON; e2e-covered across an incompatible revision |
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
4. **Webhooks** (`golem:agent/host.create-webhook`) — the SDKs expose them on
   agent methods. Cost: small (one effect + manifest support).
5. **Multimodal values** (`golem:core` text/binary/…): agent methods that take
   or return multimodal content, as the TS/Rust SDKs do. Cost: medium — schema
   graph construction (the router work already builds a graph, so the pattern
   exists).
6. **Permissions** (`golem:permissions/{types,inspect,derive,revoke,wallet,kernel-introspection}`):
   cards, grants, and the principal the runtime passes to methods (we accept
   `Principal` and ignore it). Cost: medium-large.
7. **Resource quotas** (`golem:quota/types`) and **retry policies**
   (`golem:api/retry`). Cost: small each.
8. **Databases** (`golem:rdbms/{postgres,mysql,ignite2}`), **blobstore**
   (`wasi:blobstore`), **key-value** (`wasi:keyvalue`), **durability**
   (`golem:durability`). Cost: small each (a Roc effect per operation), but the
   Roc-facing API design matters: the SDKs expose typed clients.
9. **The agent's own filesystem**: the SDKs read/write files in the agent's
   durable filesystem; we can only *serve* files through mounts. Needs
   `wasi:filesystem` in the world plus effects. Cost: medium.
10. **Oplog/execution control**: `get-oplog-index`, `mark-begin-operation`,
    `get-idempotence-mode`, interrupt/suspend, `fork`/`revert`. Cost: small each,
    mostly unused by app code.
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
