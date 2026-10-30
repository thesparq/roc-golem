---
name: golem-tools-middleware-roc
description: "Adding middleware that intercepts tool calls in Golem. Use when the user asks to audit, police, throttle, or rewrite another component's tool invocations."
---

# Tool Middleware

> **Status: pending, and deliberately so.** This platform does not support tool
> middleware, and it is not on the roadmap without a concrete use case.

## Why not

Golem identifies a middleware component by its exported interface name, so a
middleware must be a component of its own shape — it cannot be reached from an
agent-shaped component, which is what a Roc component is. Supporting it would mean
a second component shape and a second export for every app that wants it, for no
benefit to an app that just wants to publish tools.

## What to do instead

If the goal is to police a *specific* tool, put the check inside the tool's own
handler — it is the same code, just located where it runs anyway:

```roc
handleTool = |state, call|
	if Str.count_utf8_bytes(call.arguments) > 1_048_576 {
		{
			state,
			result: {
				id: call.id,
				success: False,
				output: "{\"error\": \"payload exceeds the 1 MiB limit\"}",
				stdout: "",
				stderr: "",
			},
		}
	} else {
		runTool(state, call)
	}
```

If the goal is to audit *every* caller across components, that is a runtime
concern — use Golem's own invocation log and metrics rather than an in-app
interceptor.

## Placeholder

No Roc surface exists:

```roc
# <tool middleware — not supported; a middleware is a separate component shape>
```
