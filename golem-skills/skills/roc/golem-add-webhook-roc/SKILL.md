---
name: golem-add-webhook-roc
description: "Creating a webhook an outside system can call back on in a Roc Golem agent. Use when the user asks to wait for a callback, receive a notification from another system, or hand out a URL that completes a wait."
---

# Webhooks

> **Status: implemented.**

A webhook is a durable promise plus a URL. The agent hands the URL out; a `POST` to
it completes the promise; `awaitWebhook!` then returns the body.

```roc
import pf.Golem exposing [awaitWebhook!, createWebhook!]

handleArm! : State, {} => Try({ state : State, out : Str }, Str)
handleArm! = |state, _input| {
	match createWebhook!({}) {
		Ok(url) => Ok({ state: { ..state, callback: url }, out: url })
		Err(err) => Err("could not create a webhook: ${err}")
	}
}

handleWait! : State, {} => Try({ state : State, out : Str }, Str)
handleWait! = |state, _input| {
	match awaitWebhook!(state.callback) {
		Ok(body) => Ok({ state: { ..state, answer: body }, out: body })
		Err(err) => Err("no callback: ${err}")
	}
}
```

## The two-step shape

1. **Create** — `createWebhook!` registers the promise and returns its URL. Give
   the URL to the outside system (in a reply, a request body, a log line) and store
   it in state.
2. **Await** — `awaitWebhook!(url)` blocks until the `POST` arrives, then returns
   the request body.

The promise is durable, so the POST may arrive between two invocations, or while
the agent is not running. The URL identifies the promise, which is why it has to be
kept in state rather than a local variable.

## The manifest needs an HTTP API deployment

Golem only lets an agent type deployed through an HTTP API create webhooks, and the
callback route lives on the deployment's domain under its `webhookUrl` prefix
(default `/webhooks/`) followed by the agent type name:

```yaml
httpApi:
  deployments:
    local:
      - subdomain: roc-golem
        agents:
          effects-agent: {}
```

Without a deployment entry, `createWebhook!` **traps** rather than answering `Err`.
The effects example carries a binding-less mount for exactly this reason.

## Notes

- A POST to a URL whose id does not verify answers `404`, so a forged callback
  cannot complete someone else's promise.
- `awaitWebhook!` blocks the invocation. Do not await inside a method a caller needs
  to stay responsive on — await in the invocation that is *waiting* for the answer.
  See `golem-wait-for-external-input-roc`.
