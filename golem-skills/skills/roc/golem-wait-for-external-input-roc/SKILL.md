---
name: golem-wait-for-external-input-roc
description: "Pausing an agent until an outside system or a human answers in Roc. Use when the user asks for human-in-the-loop approval, to wait for a callback before continuing, or to block until an external decision arrives."
---

# Waiting for External Input

> **Status: implemented.**

The mechanism is a durable webhook promise: the agent hands out a URL, an outside
system (or a person) POSTs to it, and the agent resumes with the body as input.

## The approval-gate pattern

A release needs a human to approve promotion. Do not poll; park the release on a
promise and let the callback carry the decision.

```roc
import pf.Golem exposing [awaitWebhook!, createWebhook!, rpcSchedule!, workerId!]

## Start the rollout and hand back the gate's URL.
handleStart! = |state, input| {
	match createWebhook!({}) {
		Err(err) => Err("could not open a gate: ${err}")
		Ok(gateUrl) => {
			# The safety timer: roll back if nobody answers.
			ignoreText(rpcSchedule!(workerId!({}), "emergency_rollback", "{\"reason\":\"gate timed out\"}", 300000))
			Ok({
				state: { ..state, canary: input.version, weight: input.weight, gate: gateUrl },
				out: gateUrl,
			})
		}
	}
}

## Answer the gate.
handleApprove! = |state, decision| {
	if decision.approved {
		Ok({ state: { ..state, weight: 100 }, out: "promoted ${state.canary}" })
	} else {
		Ok({ state: { ..state, weight: 0 }, out: "rejected ${state.canary}" })
	}
}
```

Two ways to consume the answer, and the choice matters:

- **The callback completes a promise the app later awaits.** The POST body is the
  answer; `awaitWebhook!(url)` returns it. Use this when the *agent* should resume
  on its own.
- **The callback is a separate invocation.** The reviewer calls a normal method (as
  above) with their decision; the URL exists to let an external system reach it.
  Use this when a human is driving.

## Avoid blocking a method a caller needs

`awaitWebhook!` blocks until the POST arrives. If the caller of `start_canary`
waits on the reply, awaiting inside `start_canary` would hang it. Return the URL,
let the caller act, and await in the invocation that is genuinely waiting.

## Pair it with a timeout

A promise can wait forever. Always set a safety timer (`rpcSchedule!`), as above,
so an unanswered gate does not leave a release half-rolled-out indefinitely.

## Notes

- The URL goes in the agent's state, not a local: the POST can arrive between
  invocations.
- Creating a webhook requires the agent type to be in an HTTP API deployment (see
  `golem-add-webhook-roc`), otherwise the effect traps.
