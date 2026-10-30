---
name: golem-durable-streams-roc
description: "Reading and writing an external Durable Streams service from a Roc Golem agent. Use when the user asks to append to a stream, consume a stream, resume from a checkpoint, or connect to a Durable Streams endpoint."
---

# External Durable Streams

> **Status: partial.** External streams work end to end. A method whose *parameter
> or result* is a stream does not — see `golem-streaming-agent-roc`.

A durable stream here is a **service** speaking the Durable Streams protocol,
addressed by URL. The host owns the HTTP; the app owns the checkpoints, the producer
identity and the sequence numbers — exactly as the official SDKs document.

## The five effects

```roc
import pf.Golem exposing [
	durableStreamAppend!, durableStreamRead!, durableStreamReader!,
	durableStreamRelease!, durableStreamWriter!,
]
```

| Effect | Takes | Answers |
| --- | --- | --- |
| `durableStreamReader!` | `{"url":.., "mode":"json"\|"bytes", "timeoutMs":..}` | `Try(U32, Str)` handle |
| `durableStreamRead!` | handle, `{"checkpoint":{..}, "transport":"catch-up"\|"long-poll"\|"sse", "contentType":..}` | the batch |
| `durableStreamWriter!` | `{"url":.., "contentType":.., "producerId":.., "producerEpoch":.., "timeoutMs":..}` | `Try(U32, Str)` handle |
| `durableStreamAppend!` | handle, `{"payload":.., "sequence":.., "close":..}` | the receipt |
| `durableStreamRelease!` | handle | `Try({}, Str)` |

Deadlines must be 1–300000 ms.

## Appending

```roc
append! = |url, text| {
	match durableStreamWriter!(streamWriterOptions(url, "application/json")) {
		Err(err) => Err("no writer: ${err}")
		Ok(writer) => {
			payload = "{\"tag\":\"json\",\"value\":[${Json.to_str(Json.to_str(text))}]}"
			request = "{\"payload\":${payload},\"sequence\":0,\"close\":false}"
			match durableStreamAppend!(writer, request) {
				Err(err) => Err("append failed: ${err}")
				Ok(receipt) => Ok({ state, out: receipt })
			}
		}
	}
}
```

The `json` payload is a list of **individually encoded** values — each element is
the encoded value, quotes included, because the host frames the outer array from
them. That is why the text is encoded twice above.

## Reading from a checkpoint

```roc
request = "{\"checkpoint\":{\"offset\":\"-1\",\"cursor\":null},\"transport\":\"catch-up\",\"contentType\":null}"
match durableStreamRead!(reader, request) {
	Ok(batch) => { ..state, cursor: nextOffsetOf(batch) }
	Err(err) => { ..state, lastError: err }
}
```

`-1` means "from the beginning". The batch is
`{"payload":.., "payloadBytes":N, "next":{"offset":..,"cursor":..}, "upToDate":.., "closed":..}`.
Advance the app's checkpoint to `next.offset` once you have consumed the batch.

## Rules the app owns

- **Checkpoint** — carry it in state; nothing here remembers it for you.
- **Producer identity** — `producerId` plus `producerEpoch`. A higher epoch at
  sequence 0 fences older producers with the same id.
- **Sequence** — counts *requests* for that producer. Resending the same sequence,
  payload and `close` is the protocol's idempotent retry of an uncertain append;
  new data takes the next sequence.
- **Release** — Roc holds a plain number, so the host cannot tell when you are done.
  Release what you do not keep.

## Not available

Authentication. The constructors take an optional secret and this platform passes
none, because it has no secret-handle surface (`golem-add-secret-roc`). An
authenticated stream is a follow-up.

## Notes

- Every effect is one protocol attempt, recorded in the invocation log, so a replay
  answers from the recording rather than reaching the service again.
- A stream that does not exist is reported by the first read (`not-found: ...`),
  not by the reader constructor.
