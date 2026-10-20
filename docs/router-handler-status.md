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
