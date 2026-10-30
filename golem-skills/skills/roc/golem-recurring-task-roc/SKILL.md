---
name: golem-recurring-task-roc
description: "Making an agent repeat work on a fixed interval in Roc. Use when the user asks for a cron-like job, a periodic poll, a heartbeat, or a task that reschedules itself."
---

# Recurring Work

> **Status: partial.** There is no cron or calendar scheduling. What exists is
> relative-delay scheduling (`rpcSchedule!`), which a self-rescheduling method can
> turn into a fixed interval.

## The pattern: reschedule at the end of each run

```roc
import pf.Golem exposing [rpcSchedule!, workerId!]

tick! : State => Try({ state : State, out : Str }, Str)
tick! = |state| {
	# ... do the periodic work ...
	intervalMs = 60000
	ignoreText(rpcSchedule!(workerId!({}), "tick", "{}", intervalMs))
	Ok({ state, out: "ticked" })
}
```

Each run schedules the next one, so the chain continues for as long as the agent
exists. The chain is durable — every scheduled invocation is in the invocation
log, so a restart resumes it.

## Keeping one chain, not many

A second `tick` scheduled while one is pending gives you two chains, doubling
every interval. Guard it in state:

```roc
State : { ticks : U64, running : Bool }
```

Set `running` when you start the chain and check it before scheduling again.

## What is missing

- **No cron or absolute times.** Only "after N milliseconds".
- **No "every N" primitive.** The interval drifts by however long each run takes,
  and the runtime offers no catch-up semantics.

For work that must happen at a wall-clock time, schedule a single call with the
computed delay and have the handler reschedule for the next occurrence.

## Placeholder

If a durable periodic primitive lands, it will look roughly like:

```roc
# <recurring schedule — not yet available>; today use the self-reschedule above
```
