---
name: golem-invoke-agent-roc
description: "Invoking an agent method and reading its result in Roc. Use when the user asks to call an agent, run a method, pass a JSON input, or read what a method returned."
---

# Invoking an Agent Method

> **Status: implemented.**

A method declared with `register` or `registerReadOnly` is reachable by its name.

## From the CLI

```shell
golem agent invoke 'counter-agent("c-1")' increment '{}'
golem agent invoke 'canary-release("prod")' start_canary '{"version":"1.1.0","weight":10}'
```

The third argument is the method's input as JSON, and the reply is the method's
output as JSON. A method whose input type is `{}` takes `'{}'`.

## Declaring what can be invoked

```roc
increment = method({ name: "increment", description: "Adds one.", promptHint: "" })

handleIncrement! : State, {} => Try({ state : State, out : I64 }, Str)
handleIncrement! = |state, _input| {
	next = state.count + 1
	Ok({ state: { count: next }, out: next })
}

agent = define(cfg, |_id| { count: 0 }) |> register(increment, handleIncrement!) |> build
```

The handler's annotation is the contract: `State, {} => Try({ state : State, out : I64 }, Str)`
means the caller sends `{}` and reads an `I64`.

## The two return shapes

- `Ok({ state, out })` — success. `state` becomes the agent's new state; `out` is
  encoded to JSON and returned to the caller.
- `Err(message)` — the invocation fails with that text. The state is unchanged.

## From another agent

```roc
rpc!("counter-agent(\"c-1\")", "increment", "{}")
```

`handle-message` is the other entry point: it takes the raw message rather than a
JSON input, and is what an app that declares no methods answers through.

## Notes

- An invocation is durable and recorded, so a replay does not re-run it.
- Use `golem agent invoke -i <key> ...` to pin an idempotency key explicitly.
