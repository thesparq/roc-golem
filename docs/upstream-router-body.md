# Upstream report: a guest-created `schema-value-stream` in a returned value tree is never delivered

**Filed upstream as [golemcloud/golem#4291](https://github.com/golemcloud/golem/issues/4291), and answered by a Golem developer: the bug was ours.**

> **Correction, after the reply.** The Golem developer read the handler code and
> pointed at the actual cause: this platform built each body *chunk* as a value
> tree holding only its `ListValue`, while that list's indices referred to the
> **response** tree around it. A chunk's indices must refer to its own
> `value_nodes`. With self-contained chunks (`body_chunks` in `host/src/lib.rs`)
> the response body arrives — verified on a real server for a small body, a POST
> echo, and 100 KB written as several chunks. So the report below is wrong about
> the runtime: `wrap`, the pipe, the session relay and the client framing were all
> fine, and the items it lists as "not the response head" were a symptom of the
> malformed chunk rather than an independent finding. Kept for the measurements
> and because the misdiagnosis is instructive: the only visible symptom was an
> empty body. The one request in it that stands is the diagnostics ask — a
> structural stream error should not look like a generic HTTP body failure.

This is the report for the one thing the dynamic HTTP router handler
cannot do on Golem 1.6.0 (parity item 1 in `docs/golem-1.6-parity.md`). The
measurements behind it are in `docs/router-handler-status.md`; this file is the
short, self-contained version for the Golem issue tracker.

## Summary

A guest component that returns a value tree containing a stream it created itself
(`golem:core/types@2.0.0` `schema-value-stream.wrap`) — the documented way to put
a native stream into a `schema-value-tree`, and what an HTTP router's response
body is — gets **no error and no delivery**: the guest's write into that stream
completes, the runtime registers an output stream for it, and the items never
reach the invocation session. The client sees the response head with an empty
body. The official Rust SDK's router delivers bodies, so the runtime can do this;
the difference is in how the stream gets into the value tree.

## Environment

- Golem 1.6.0 GA (`v1.6.0`), local server (`golem server run`), single component
  deployed through an HTTP API, `http-router` kind, mount `/static`.
- Roc platform component (`roc-golem`) built with Golem's `wit-bindgen` fork
  (`wit-bindgen-golem-1.6`, rev `3e7d6bd6e79809b2d64a151515d51800448fab2a`,
  `wit-bindgen 0.59.0`) and the component model async ABI (`[task-return]invoke`
  is present in the built component).
- Control: the official Rust SDK probe (`golem-rust v3.0.0`, `#[http_router]`,
  `AgentStream::<Vec<u8>>::new()` plus `spawn_local` for the write) deployed on
  the *same* server.

## What happens

| | SDK probe | this component |
| --- | --- | --- |
| client | `200`, 35 bytes, clean chunked framing | `200`, 0 bytes, curl reports a truncated transfer |
| oplog | `StreamSession`, **`StreamItems`**, `StreamEnd` | `StreamRegistered`, `StreamSession`, `StreamEnd` — **no `StreamItems`** |
| guest write | blocks until the runtime reads | returns immediately ("0 bytes left over") |

So the chunk is accepted by something on the host side and then dropped: it never
enters the durable session journal, so the gateway has nothing to relay.

## What it is not

- **Not timing.** Queuing the write with `spawn_local`, calling
  `wit_bindgen::yield_async()` before it, and blocking on a WASI clock for 300 ms
  before it (the oplog shows the output stream registering *during* that wait)
  all leave the body empty.
- **Not the response head or the value-tree shape.** The same handler returns a
  `204` correctly, and the same response record with a **host-created** stream in
  the body slot (the request's own body stream, which the runtime created for the
  session) delivers the client's upload back to it (`POST /static/echo` → `200`,
  `echo me`). The head, the slot, the relay and the framing are all fine.
- **Not the guest's item encoding.** The item is a `schema-value-tree` whose root
  is a `list<u8>` — the same shape the SDK's `AgentStream<Vec<u8>>` produces.

## Reproducer sketch

Any component whose exported `golem:agent/guest@2.0.0` `invoke` returns a
`schema-value-tree` with a `stream-value` node in it, where the stream was created
by the guest:

```rust
// inside an async guest export (the router handler's method)
let (mut writer, reader) = wit_stream::new();          // stream<schema-value-tree>
let handle = golem::core::types::SchemaValueStream::wrap(reader).await;  // host takes the read end
// ... return the response record with `StreamValue(handle)` in its body field ...
writer.write_all(vec![chunk]).await;                    // accepted, never delivered
drop(writer);
```

- `wrap` is implemented by the executor as
  `reader.pipe(&mut access, consumer)` into an `accounted_output_stream_pair`
  (`golem-worker-executor/src/durable_host/schema_value_stream.rs`, tag
  `v1.6.0`), and the handle pushed into the table wraps that live output stream.
- Writing the same chunk from a task spawned *after* the export's `task.return`
  (the SDK's shape) changes nothing, and writing it *before* the return
  deadlocks only in the SDK's case — in this component it is accepted
  immediately, which is the asymmetry that points at the pipe.

## Two candidate causes, for whoever knows the code

1. The `wrap` pipe is not driven outside the export call: the piped items sit in
   the live output stream and are never pulled into the session.
2. The transport stream id the returned value carries is not the one the session
   publishes under, so the gateway's `output_step` (which matches
   `transport_stream_id` against the id parsed out of the response value) skips
   the items. The session mapping logged for this response is
   `{ transport_stream_id: 2, source: Local((19)), role: Output }`.

## Requested API shape (optional)

The tool surface already has the shape that would sidestep this:
`golem:tool/host@0.1.0.create-output` gives the guest a *writer* and keeps the
read end host-side. A router body endpoint of the same kind — a host-created
output stream a handler can write into and return — would make the response body
host-owned, which this platform has demonstrated it can deliver. Failing that,
making the `wrap` path deliver would fix it for every guest language at once.
