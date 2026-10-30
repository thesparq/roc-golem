---
name: golem-annotate-agent-roc
description: "Writing the descriptions and prompt hints a caller sees for a Roc Golem agent and its methods. Use when the user asks to document an agent, improve how a model picks its methods, or write prompt hints."
---

# Describing an Agent and its Methods

> **Status: implemented.**

Three kinds of text reach a caller: the agent's own name and description, a
method's description, and a method's prompt hint. They are data in the spec, not
comments — the CLI and MCP surface them.

## The agent

```roc
agent : Agent(State)
agent =
	define(
		{
			name: "release-guardian",
			version: "1.0.0",
			description: "Orchestrates canary releases and guards them.",
		},
		|service| { service: service, weight: 0 },
	)
		|> build
```

`name` is the agent type name the CLI uses:

```shell
golem agent invoke 'release-guardian("prod")' <method> '<json input>'
```

## A method

```roc
startCanary = method({
	name: "start_canary",
	description: "Starts a canary rollout and returns the gate's approval URL.",
	promptHint: "Call this to begin rolling a version out to a percentage of traffic.",
})
```

- `description` — what the method does, for a human or a model reading a list.
- `promptHint` — when to call it. This is the text that most changes whether a
  model picks the right method, so say the situation, not the mechanics. `""` is
  allowed and means "no hint".

## Rules of thumb

- Name methods with a verb in snake_case: `start_canary`, `get_status`,
  `emergency_rollback`.
- Say what a method *returns*, because a caller often chains on it.
- Do not restate the input schema in prose; the caller already has it. Say when
  to reach for the method instead.
