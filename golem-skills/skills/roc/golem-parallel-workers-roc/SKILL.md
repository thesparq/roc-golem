---
name: golem-parallel-workers-roc
description: "Splitting an agent's work across parallel copies in Roc. Use when the user asks to fan out, run work in parallel from one agent, or process a batch concurrently."
---

# Parallel Workers

> **Status: partial.** The primitive is `fork!`, which splits the *agent*; there is
> no worker-pool API and no scatter-gather primitive.

## Forking

```roc
import pf.Golem exposing [fork!]

match fork!({}) {
	Ok(tag) => {
		# One branch sees "original:<phantom-id>", the other "forked:<phantom-id>".
		state: { ..state, lastFork: tag },
		replies: ["this copy is ${tag}"],
	}
	Err(err) => { state, replies: ["fork failed: ${err}"] }
}
```

Both copies continue from the same point, each with its own state and its own
future invocations. The fork is durable, so neither copy replays the work that led
to it.

## What this is good for

- Two independent continuations of one decision (a release that also opens an
  investigation).
- Handing a long-running tail to a copy while the original stays responsive.

## What it is not

- **Not a worker pool.** You cannot fork N ways and join the results in one place.
  Copies are separate agents after the fork; they can only rejoin through a
  message (an `rpc!` to a shared coordinator) or a webhook.
- **Not cheap fan-out.** Each fork is a real agent with its own state and oplog.

For N-way fan-out, the ordinary pattern is one call per unit of work into an agent
keyed by that unit — `agent-type("unit-3")` — rather than forking.

## Placeholder

No pool or scatter-gather surface exists:

```roc
# <parallel workers — not available>; address one agent per unit of work,
# or fork when you need two continuations rather than N results.
```
