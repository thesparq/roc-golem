---
name: golem-call-from-external-roc
description: "Reaching a Roc Golem agent from outside Golem, such as a CI pipeline, a script, or a webhook. Use when the user asks to call an agent from CI, from a shell script, from a browser, or from another system."
---

# Calling an Agent from Outside

> **Status: implemented.**

Three ways in, depending on who is calling.

## The CLI

```shell
golem agent invoke 'canary-release("prod")' start_canary '{"version":"1.1.0","weight":10}'
```

Needs Golem credentials for the environment. This is what CI uses.

## Tools

```shell
golem tool invoke --agent 'canary-release("prod")' analyze-logs -- "PANIC boot"
```

Or over MCP, if the deployment's MCP server is configured.

## HTTP

Declare a mount and list the agent type in the deployment's HTTP API, then a
plain HTTP request reaches it:

```shell
curl -H 'Host: roc-golem.localhost:9882' \
  http://127.0.0.1:9882/canary/prod/dashboard.html
```

For a *dynamic* endpoint — a POST that starts a rollout — the agent needs a router
with `handleRequest!`, or the request has to be turned into an invocation by
something that can make one.

## The reverse direction: callbacks

If the outside system must tell the agent something after it has run, do not poll.
Create a webhook and hand out its URL:

```roc
match createWebhook!({}) {
	Ok(url) => Ok({ state: { ..state, callback: url }, out: url })
	Err(err) => Err("could not create webhook: ${err}")
}
```

The outside system `POST`s to that URL and the promise completes; see
`golem-add-webhook-roc`.

## Notes

- For CI, prefer the CLI over HTTP: it reports the invocation's result, whereas a
  mount is designed for content.
- An HTTP mount answers `GET`/`HEAD` from the filesystem without invoking the
  agent, so it cannot start work by itself.
