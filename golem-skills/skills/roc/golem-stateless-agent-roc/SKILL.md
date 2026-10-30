---
name: golem-stateless-agent-roc
description: "Writing an agent that keeps no state between invocations in Roc. Use when the user asks for a stateless worker, a pure function exposed as an agent, or to avoid persisting state."
---

# Stateless Agents

> **Status: pending.** A Roc agent is always durable: it has a state type and the
> runtime snapshots it. The one stateless shape today is a `HttpRouter` agent type,
> which is parameterless, ephemeral and has no state — but it is a website, not a
> callable worker.

## What to do today

**Keep the state empty.** For an agent whose work carries no memory, use unit-ish
state and ignore it:

```roc
State : {}

agent : Agent(State)
agent =
	define({ name: "transformer", version: "1.0.0", description: "..." }, |_id| {})
		|> register(transform, handleTransform!)
		|> build

handleTransform! : State, Input => Try({ state : State, out : Output }, Str)
handleTransform! = |state, input| {
	Ok({ state, out: run(input) })
}
```

The runtime still records the invocation, which is what makes the call durable and
replayable — usually what you want anyway. What you save by going stateless is the
state snapshot, which is nothing when the state is `{}`.

## For a stateless *web endpoint*

Use a router:

```roc
httpMount: Mount({
	pathPrefix: ["static"],
	staticBindings: [ Exact({ publicPath: ["index.html"], filePath: "/index.html" }) ],
	filesystemBindings: [],
	fileResponseHeaders: [],
	corsAllowedPatterns: [],
}),
agentKind: HttpRouter,
```

That agent type is ephemeral: no state, no filesystem, no invocation history. It
answers from files uploaded at deploy time.

## The gap

A general ephemeral agent — stateless, parameterless, callable by method, not a
router — does not exist here. The agreed direction is a third agent kind alongside
`Regular` and `HttpRouter`; no Roc shape is settled yet:

```roc
# <ephemeral agent kind — not yet available>
```

For now, a `Regular` agent with `State : {}` is the practical answer.
