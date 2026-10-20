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
