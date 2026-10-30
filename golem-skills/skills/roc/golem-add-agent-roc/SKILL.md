---
name: golem-add-agent-roc
description: "Adding a durable agent to a Roc Golem component. Use when the user asks to create, add, or define a new agent type, or to add methods to an existing agent."
---

# Adding an Agent to a Roc Golem Component

> **Status: implemented.**

An agent is a durable, stateful unit of computation. In Roc an agent is a single
value named `agent`, built with `define` and finished with `build`. The platform
derives the metadata, the JSON dispatch and the state codecs from it.

## Layout

An application is a Roc package whose platform is this one. The entry file must
define `agent`:

```roc
app [agent] { pf: platform "https://github.com/thesparq/roc-golem/releases/download/v0.6.1/A2kLeoXZzEUKd60XKgT3ZGz_1YeDo4l0YZZpbi69wqE.tar.zst" }

import pf.Golem exposing [Agent, build, define, method, register, registerReadOnly]

## The state the runtime snapshots between invocations.
State : { count : I64 }

increment = method({
	name: "increment",
	description: "Increments the counter and returns the new value.",
	promptHint: "Call this to add one to the counter.",
})

handleIncrement! : State, {} => Try({ state : State, out : I64 }, Str)
handleIncrement! = |state, _input| {
	next = state.count + 1
	Ok({ state: { count: next }, out: next })
}

agent : Agent(State)
agent =
	define(
		{
			name: "counter-agent",
			version: "1.0.0",
			description: "A durable counter written in Roc.",
		},
		|_id| { count: 0 },
	)
		|> register(increment, handleIncrement!)
		|> build
```

Replace `https://github.com/thesparq/roc-golem/releases/download/v0.6.1/A2kLeoXZzEUKd60XKgT3ZGz_1YeDo4l0YZZpbi69wqE.tar.zst` with the platform release URL (see the README of this skill
set; `roc check` will otherwise fail to resolve `pf`).

## The pieces

- `define(cfg, init!)` — starts the spec. `init!` receives the agent's
  constructor parameter (its identity, as text) and returns the initial state.
  That return type pins `State` for everything chained after it.
- `register(method, handler!)` — binds an effectful handler.
- `registerReadOnly(method, handler)` — binds a pure handler (see
  `golem-mark-read-only-roc`).
- `build` — finishes the spec and produces the `Agent(state)` the platform wants.

Method names are snake_case strings, and they are what the CLI and RPC invoke:

```shell
golem agent invoke 'counter-agent("c-1")' increment '{}'
```

## Several agents in one deployment

One component holds **one** agent type. To ship more than one, build more
components: each gets its own `main.roc` and its own entry in `golem.yaml` under
`components:`.

## Notes

- Roc has no `var`, `for` or `return`. A handler that needs a loop is written as
  recursion or with the `List` functions.
- State must be encodable and parseable, which the platform derives from the
  type — so keep `State` a plain record of data, not a function.
