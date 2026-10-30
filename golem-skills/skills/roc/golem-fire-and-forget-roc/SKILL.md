---
name: golem-fire-and-forget-roc
description: "Sending a message to another agent without waiting for the reply. Use when the user asks to notify another agent, fan out a message, or start work in the background."
---

# Fire and Forget

> **Status: implemented.**

`rpcFireAndForget!` returns as soon as the invocation is accepted. Nothing waits
for the target.

```roc
import pf.Golem exposing [rpcFireAndForget!]

match rpcFireAndForget!("notifier-agent(\"n-1\")", "handle-message", "\"release started\"") {
	Ok(identity) => logInfo!("queued: ${identity}")
	Err(err) => logWarn!("could not queue: ${err}")
}
```

Same three arguments as `rpc!`: agent id, method name, parameter.

## The answer is an invocation identity

`Ok` is JSON, not the target's reply — no reply exists yet:

```json
{"agentId": "...", "idempotencyKey": "..."}
```

That identity is what lets you reason about the invocation later. Keep it if you
might want to look the invocation up; drop it if you do not.

## When to use it

- Notifying several agents about one event.
- Starting work whose result nobody branches on.
- Avoiding a call chain that would otherwise block on a slow target.

Use `rpc!` instead when the answer decides what happens next — a fire-and-forget
call gives you no way to react to a failure in the target.

## Notes

- The invocation is durable and will be retried by the runtime according to its
  retry policy, so the target's method should be safe to run more than once. An
  `idempotencyKey!` in the target, or a check on its state, is how to make that
  true.
- An `Err` here means the invocation could not be *enqueued*, not that it failed.
