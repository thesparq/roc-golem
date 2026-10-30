---
name: golem-multi-instance-agent-roc
description: "Designing an agent type that runs as many independent instances in Roc. Use when the user asks for per-tenant agents, one agent per user or service, or to shard work across instances."
---

# Multi-Instance Agents

> **Status: implemented.**

One agent *type* runs as many instances. An instance is identified by its
constructor parameter, so designing the parameter is designing the sharding.

## The parameter is the key

```roc
agent : Agent(State)
agent =
	define(
		{ name: "release-guardian", version: "1.0.0", description: "..." },
		|service| { service: service, canary: "", weight: 0 },
	)
		|> build
```

```shell
golem agent invoke 'release-guardian("shop")' start_canary '{"version":"1.1.0","weight":10}'
golem agent invoke 'release-guardian("billing")' start_canary '{"version":"1.1.0","weight":10}'
```

Two instances, two states, one component. Neither can see the other's state.

## Choosing the key

- **Per tenant / service / user** — the common case. The key is the subject the
  state belongs to.
- **Per task** — `job("job-42")`, when each unit of work deserves its own durable
  history and retry policy.
- **Avoid high-cardinality keys you do not need.** Every instance holds state and a
  log forever; a key per request turns one agent into millions.

## Composite keys

There is one constructor parameter, a `Str`. Pack several values and split them in
`init!`:

```roc
	|key| {
		{ env: envOf(key), service: serviceOf(key), weight: 0 }
	}
```

```shell
golem agent invoke 'release-guardian("prod/shop")' get_status '{}'
```

Document the separator — the id is the API, and a caller has to build it.

## Notes

- Instances are not collected when idle. Delete the ones you are done with
  (`golem agent delete`).
- A `State` should describe one subject. If a handler needs to look at several
  instances' data, the data belongs somewhere else (a shared store, or a
  coordinator agent).
