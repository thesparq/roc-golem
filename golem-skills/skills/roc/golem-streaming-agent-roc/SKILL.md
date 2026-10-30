---
name: golem-streaming-agent-roc
description: "Declaring an agent method whose parameter or result is a stream in Roc. Use when the user asks for a streaming method, incremental output, resumable upload, or an HTTP streaming endpoint."
---

# Streaming Methods

> **Status: pending.** No method in this platform takes or returns a stream.
> Nothing below compiles today.

## What exists instead

- **External** durable streams — reading and writing a Durable Streams *service* —
  work today. See `golem-durable-streams-roc`.
- **A router's response body** may be built incrementally by the app: the host
  writes it as bounded chunks, and the end-to-end suite covers a 100 KB body.
- **Tool streams** (stdin/stdout/stderr) are attached by the caller, but the app's
  own output is buffered until the handler returns.

## What is missing

Two of the three pieces of the SDKs' story:

1. A method whose **parameter** is a stream. The machinery is proven — a router's
   request body is a host-created stream the app already reads — but the surface is
   not exposed.
2. A method whose **result** is a stream, and the HTTP streaming endpoints derived
   from it. A guest-created stream does reach a client today (that is what a router
   body proves), but a *declared method* cannot return one.

Each is "one more declared method plus one more hook", the same shape as
`handleRequest!` and `handleMultimodal!`, plus the durable-session route options
(`HttpEndpointDetails.durable_streams`) this platform does not build.

## The intended shape

The agreed direction, not yet available — a declared method noting that its input or
output is a stream:

```roc
# <streaming method declaration — not yet available>
# Likely shape: a method declared with a stream marker, bound to a handler that
# receives an item cursor and writes items, e.g.
#   watchEvents = methodStream({ name: "watch_events", description: "..." })
#   handleWatch! : State, {} => Stream(ReleaseEvent)
```

Do not build against this yet; it will move when the surface lands.

## Placeholder for the missing pieces

```roc
# <method stream parameter — not yet available>
# <method stream result — not yet available>
# <durable-session route options — not yet available>
```
