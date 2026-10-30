---
name: golem-trigger-agent-roc
description: "Triggering an agent method without awaiting the result in Roc. Use when the user asks to start an agent's work, kick off a run, or send a one-way message to an agent."
---

# Triggering an Agent

> **Status: implemented.**

"Trigger" and "fire and forget" are the same mechanism here: `rpcFireAndForget!`
starts an invocation and returns its identity rather than its result.

```roc
import pf.Golem exposing [rpcFireAndForget!]

match rpcFireAndForget!("release-guardian(\"prod\")", "start_canary", "{\"version\":\"1.1.0\",\"weight\":10}") {
	Ok(identity) => logInfo!("triggered: ${identity}")
	Err(err) => logWarn!("not triggered: ${err}")
}
```

The identity is JSON:

```json
{"agentId": "...", "idempotencyKey": "..."}
```

## Triggering from outside

`golem agent invoke` is the awaiting form. For a one-way trigger from a script,
call the method that does the work and ignore the reply — or, if the target is
behind an HTTP mount, `POST` to it and let the router start the work.

## Notes

- A trigger is durable: the runtime retries it per its policy, so the target must
  tolerate being run twice. `idempotencyKey!` in the target is the usual guard.
- To react to the outcome, you need `rpc!` (awaiting) or a webhook back
  (`golem-add-webhook-roc`); a trigger reports only that it was accepted.
