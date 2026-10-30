---
name: golem-agent-reflection-roc
description: "Looking up agent and component identities at runtime in a Roc Golem agent. Use when the user asks to resolve an agent id, find a component id, or discover another agent's identity before calling it."
---

# Agent Reflection

> **Status: partial.** Identity resolution works. Enumerating an agent type's
> methods at runtime does not, so an agent cannot discover a remote surface — it has
> to know the method names it will call.

## What works

```roc
import pf.Golem exposing [
	makeAgentId!, parseAgentId!, resolveAgentId!, resolveComponentId!, workerId!,
]

match resolveComponentId!("roc-golem:counter") {
	Ok(componentId) => logInfo!("counter component is ${componentId}")
	Err(err) => logWarn!("not deployed: ${err}")
}

match resolveAgentId!("roc-golem:counter", "counter-agent(\"c-1\")") {
	Ok(idsJson) => logInfo!("found: ${idsJson}")
	Err(err) => logWarn!("no such agent: ${err}")
}
```

- `resolveComponentId!(componentReference)` — a deployed component's UUID, as text.
  The reference is the name the manifest declares (`"roc-golem:counter"`).
- `resolveAgentId!(componentReference, agentName)` — `{"componentId":..., "agentId":...}`.
  The agent name is **not** checked for existence.
- `parseAgentId!(agentId)` — splits an id into `{"agentType":..., "constructor":...,
  "phantomId":...}`.
- `makeAgentId!(agentTypeName, constructorParam)` — the inverse.
- `workerId!({})` — this invocation's own agent id.

## The usual reason to reach for this

Building a target id when the component is not known at compile time:

```roc
match makeAgentId!("roc-golem:notifier", "notifier-agent(\"n-1\")") {
	Ok(id) => rpc!(id, "notify", payloadJson)
	Err(err) => Err(err)
}
```

`makeAgentId!` does not verify that the target exists; the call itself reports that.

## The gap

There is no way to enumerate an agent type's methods, so a "call whatever the remote
provides" pattern cannot be built:

```roc
# <method enumeration — not yet available>
# The Rust SDK's reflection can list an agent type's methods and invoke one by
# name. This platform can resolve identities only.
```

In practice a Roc agent knows its collaborators statically: the method name is a
string in its own source. Reflection here is for *addressing*, not for discovering
capability.
