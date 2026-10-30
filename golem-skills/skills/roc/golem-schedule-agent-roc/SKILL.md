---
name: golem-schedule-agent-roc
description: "Scheduling an agent invocation for later from a Roc Golem agent. Use when the user asks to delay a call, set a timer, run something after a timeout, or schedule work for a future time."
---

# Scheduling a Call for Later

> **Status: implemented.**

`rpcSchedule!` queues a method invocation to run after a delay and returns at
once.

```roc
import pf.Golem exposing [rpcSchedule!, workerId!]

match rpcSchedule!(
	workerId!({}),
	"emergency_rollback",
	"{\"reason\":\"approval gate timed out\"}",
	300000,
) {
	Ok(identity) => logInfo!("timer set: ${identity}")
	Err(err) => logWarn!("could not schedule: ${err}")
}
```

Arguments: target agent id, method name, parameter, and **delay in milliseconds**.
The delay is relative to now; there is no absolute-time form.

## The safety-timer pattern

A canary release wants to roll itself back unless a human approves in time.
Scheduling a self-call at the start of the rollout gives exactly that:

```roc
handleStart! = |state, input| {
	events = List.append(state.events, "canary up")
	ignoreText(rpcSchedule!(workerId!({}), "emergency_rollback", "{\"reason\":\"gate timed out\"}", 300000))
	Ok({ state: { ..state, weight: input.weight, events: events }, out: input.version })
}
```

The timer is durable: it is recorded in the invocation log, so a restart does not
lose it.

## Cancelling

`rpcSchedule!` gives no handle, so an approved gate cannot cancel its timer — the
rollback simply runs and finds the release already promoted. If you need to
cancel, use `rpcScheduleCancelable!` (see `golem-schedule-future-call-roc`).

## Notes

- The invocation identity `Ok` returns is JSON, not the eventual reply.
- A scheduled call runs in the future state of the target, so a rollback firing
  after an approval sees the promoted state — check it and return early if there
  is nothing to do.
