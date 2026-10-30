---
name: golem-call-tool-roc
description: "Calling another component's tool from a Roc Golem agent. Use when the user asks an agent to invoke a tool, consume an MCP server's tool, or reuse another component's published tool."
---

# Calling Another Component's Tool

> **Status: pending.** This platform has no tool *client* half yet. Nothing below
> compiles today.

## What works instead

An agent can call another **agent** — see `golem-call-another-agent-roc` — and
the target agent can expose the same logic as a method. That is the supported way
to reach another component's behaviour today:

```roc
match rpc!("roc-golem:analyzer", "analyzer-agent(\"a-1\")", "analyze", argsJson) {
	Ok(reply) => reply
	Err(err) => "analyzer failed: ${err}"
}
```

## The gap

Calling a *tool* is a different interface (`golem:tool/host`), which this platform
does not import. It is not a naming problem: the client half needs the tool
surface's stream machinery and a typed way to describe the call.

## Placeholder

The Roc shape for the client half has not been agreed yet, so this skill shows
none rather than inventing one:

```roc
# <tool client — shape not yet designed>
# Invoking a published tool from a Roc agent will look roughly like:
#   invokeTool!(<component reference>, <tool name>, <input>, <stdin?>)
# and will return the tool's result and its attached streams.
```

When the host grows that interface, this skill becomes concrete. Until then, use
agent-to-agent RPC.
