# Dynamic router handler: status and the response-body blocker

The request half works and is verified on Golem 1.6.0 (see the commit this file
came with): the schema graph Golem validates, the `handle` method, decoding the
request out of the value tree, draining the request body stream, calling the app
and returning a response tree. A handler answering `204` with no body round-trips.

A **non-empty response body** does not, and the two candidate fixes are ruled
out with evidence:

1. Writing the body before returning hangs the invocation (observed as a 504
   timeout). A guest-created stream (`wit_stream::new()`) is a *rendezvous*: the
   write completes only once the runtime reads, and the runtime starts reading
   the response body only after `invoke` returns its value tree. The canonical
   `stream.new` takes no capacity, so a guest cannot ask for a buffer.
2. `wit_bindgen::spawn_local` (the fork's "execution after returning") does not
   help in this export shape: the spawned future is dropped when the export's
   task completes, so the write never runs. Verified: a log line at the top of
   the spawned task never appeared, and the client saw an empty body because
   dropping the writer closed the stream.

Golem's durable streams are host-backed (append/read into host storage, no
rendezvous), but `wasi:http`-style response bodies are `stream<list<u8>>`
component-model streams, and `golem:agent/durable-streams` exposes no conversion
to a `schema-value-stream` — a guest can only `wrap` a native stream.

So the body needs the runtime to read the returned stream **while the guest task
is still alive**. The component model supports that (`task.return` followed by
more work — it is how wasi-http p3 streams response bodies), but the fork's
generated export completes the task when the async `invoke` future resolves.

The decisive next experiment, and the reason this is worth keeping: build a tiny
Rust SDK router (`#[http_router]` whose `handle` returns a response with a body)
against the same Golem 1.6.0 server and watch whether its body arrives. If it
does, the SDK's machinery has the keep-alive and this platform can adopt that
shape; if it does not, the response body is a runtime limitation and the
static/provider router is the right deliverable for this kind.

## Update: the ordering experiment, and what the SDK's probe changed

The Rust SDK probe (`tooling/sdk_router_probe`, built with `golem-rust v3.0.0`)
**does** deliver a body: its router answers `200` with `sdk probe body for
/sdk-probe/hello`. So the response body is not a runtime limitation.

That pointed at ordering, and moving the write *after* the read end is handed
over (`SchemaValueStream::wrap` first, then `write_all`, inline — no spawn)
changed the symptom but not the outcome:

- before: the write blocked and the invocation timed out (504);
- after: the write **completes** (no leftover bytes, so the runtime did consume
  the chunk) but the client receives `200` with a **0-byte, truncated** body.

The most consistent reading: the runtime drains the stream as soon as it owns the
read end — which is what unblocks our write — but it has not yet attached that
stream to a response, so the drained bytes are not the response body. The SDK
must therefore deliver the response head *before* the body is written, i.e. the
`task.return`-then-continue shape that wasi-http p3 uses and that the fork's
generated export (a plain async fn) does not express. That is the thing to study
next, and it is a change in the export machinery rather than in this handler.

## Update: the controlled comparison, and what it rules out

That reading is wrong. Everything below was measured on one local Golem 1.6.0
server, with this platform's `static_site` router and the SDK probe deployed
side by side:

1. The SDK probe delivers a body: `GET /sdk-probe/hello` answers `200` with
   `sdk probe body for /sdk-probe/hello` (35 bytes) and clean chunked framing.
   Ours answers `200` with `transfer-encoding: chunked` and **0 bytes**, and curl
   reports a truncated transfer (`exit 18`).
2. The oplogs differ in one decisive entry. Ours records the write completing
   (`router: body writer finished, 0 bytes left over`), the runtime registering
   an output stream (`StreamRegistered` + `StreamSession`, `transport_stream_id:
   2`) and then `StreamEnd` — with **no `StreamItems` entry**: the chunk never
   enters the durable session journal. The probe's oplog has `StreamItems`
   between its `StreamSession` and `StreamEnd`. So the item is not lost on the
   way to the client; it never reaches the session.
3. **Timing is ruled out.** Queuing the write with the fork's `spawn_local` (the
   SDK's shape), `yield_async()` before the write, and a 300 ms blocking clock
   wait before the write (during which the oplog shows the output stream being
   registered) all leave the body empty. Writing *after* the runtime has attached
   the stream does not help.
4. The SDK probe with an **inline** write (before it returns the response)
   deadlocks (`504`): its writes backpressure until the runtime reads. Ours never
   backpressure — the write is accepted immediately and the bytes go nowhere —
   so the two "wrapped" streams are not the same thing on the host side.
5. The response value's stream slot itself is fine. Returning the **request's**
   stream (a host-created endpoint, `transport_stream_id: 1`) in the response
   body slot delivers the client's own upload back to it: `POST /static/echo`
   with `echo me` answers `200` with `echo me` (7 bytes). The head, the body
   slot, the session relay and the client framing all work when the stream is
   one the runtime created.

So the blocker is narrower than "the export machinery": a **guest-created**
stream placed in a returned value tree is not delivered. `wrap` is the transfer
in question, and the executor implements it as `reader.pipe(&mut access,
consumer)` into an `accounted_output_stream_pair` live output stream
(`durable_host/schema_value_stream.rs`, tag `v1.6.0`) — the items our write
accepted never reach that live stream's subscriber, while the same slot works
for an endpoint the runtime itself created. Whether the pipe is not driven
outside the export call, or the transport id the response value carries is not
the one the session publishes under, needs executor-side instrumentation to
settle; it is not something this handler can decide.

### What is in place, and the two ways forward

Working and covered on this branch: the request path (schema graph, `handle`
method, request decode, request-body drain), the response head, bodyless
responses (`204`), the OpenAPI provider and static bindings.

1. **Upstream, if a host-created body endpoint is acceptable.** The tool surface
already has exactly that shape: `golem:tool/host.create-output` returns the
writer to the guest and keeps the read end. A router body endpoint of the same
kind would make the response body a host-owned stream, which this platform has
demonstrated it can deliver.
2. **Executor-side, to make `wrap` deliver.** Instrument
`durable_host/schema_value_stream.rs`'s `wrap`/pipe path on a live server, and
check the transport id the returned value carries against the session mapping.
Whichever it turns out to be, the platform-side change is small: the handler
already builds the correct tree.
