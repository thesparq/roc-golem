---
name: golem-create-agent-instance-roc
description: "Creating an agent instance and choosing its identity in Roc. Use when the user asks to construct an agent, pass a constructor parameter, or address one of several instance of the same agent type."
---

# Creating an Agent Instance

> **Status: implemented.**

An agent instance is created on first use. Its identity *is* the constructor
parameters, so creation is not a separate step — addressing a new id creates it.

## The constructor

```roc
agent : Agent(State)
agent =
	define(
		{ name: "canary-release", version: "1.0.0", description: "..." },
		|service| { service: service, canary: "", weight: 0 },
	)
		|> build
```

`define`'s second argument is `init!`: it takes the constructor parameter — the
agent's identity, as text — and returns the initial state. The platform calls it
once, when the instance is first created.

## Addressing an instance

```shell
golem agent invoke 'canary-release("prod")' start_canary '{"version":"1.1.0","weight":10}'
golem agent invoke 'canary-release("staging")' start_canary '{"version":"1.1.0","weight":10}'
```

Those are two instances with two independent states. Same code, same component.

## From another agent

```roc
match makeAgentId!("roc-golem:canary-release", "canary-release(\"prod\")") {
	Ok(id) => rpc!(id, "get_status", "{}")
	Err(err) => Err(err)
}
```

## Notes

- The parameter is a single `Str`. Roc's surface takes one constructor parameter;
  encode several values in it (`"prod/eu-west-1"`) and split them in `init!`.
- An instance is not garbage collected because you stopped calling it; it stays
  until it is deleted (`golem agent delete`).
- Creating an instance is cheap. Do not build a registry agent to hand out ids —
  compute the id and address it.
