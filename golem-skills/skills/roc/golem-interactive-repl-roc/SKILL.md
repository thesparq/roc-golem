---
name: golem-interactive-repl-roc
description: "Exploring a Roc Golem agent interactively from a shell. Use when the user asks to poke at an agent, try methods by hand, inspect state, or iterate without redeploying."
---

# Interactive Exploration

> **Status: platform-level.** There is no REPL in this platform. What exists is
> `golem agent invoke` and the CLI's own exploration commands, which cover the same
> ground for a Roc agent.

## Trying a method

```shell
golem agent invoke 'counter-agent("c-1")' increment '{}'
golem agent invoke 'counter-agent("c-1")' get_status '{}'
```

The reply is the method's output as JSON. Repeating a call continues from the state
the last one left — that is what makes this useful for exploration rather than a
one-shot test.

## Inspecting an agent

```shell
golem agent list                                    # agents in the environment
golem agent get 'counter-agent("c-1")'              # details and state
golem agent logs 'counter-agent("c-1")'             # the agent's log
golem agent files 'counter-agent("c-1")' --path /   # its filesystem
```

## Trying a tool

```shell
golem tool list
golem tool invoke --agent 'counter-agent("c-1")' get-count
```

## The loop that works well

1. `./tooling/build.sh <app>` — rebuild the component.
2. `golem deploy --yes` — push it.
3. `golem agent invoke ...` — exercise the method.

A changed component gets a new revision; agents created before it stay on the old one
unless they are updated. For iterating on one agent, delete it and let the next
invocation recreate it on the current revision:

```shell
golem agent delete 'counter-agent("c-1")'
```

## Placeholder

No in-process REPL over an agent's methods exists:

```roc
# <repl — not available>; use `golem agent invoke` as above
```

## Notes

- `golem agent invoke -i <key> ...` pins an idempotency key, which is how you check
  that a repeated call is answered from the record rather than re-run.
- For a UI over an agent's own data, a filesystem mount serving a file the agent
  writes is quicker than a REPL: see `golem-add-http-endpoint-roc`.
