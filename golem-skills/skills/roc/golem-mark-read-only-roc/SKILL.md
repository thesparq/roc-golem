---
name: golem-mark-read-only-roc
description: "Declaring a method read-only in a Roc Golem agent so the runtime may cache it and the compiler forbids it from writing. Use when the user asks for a pure getter, a status method, or caching of an unchanging answer."
---

# Read-Only Methods

> **Status: implemented.**

A read-only method promises the runtime it will not write. Golem may then cache
the answer, and it refuses a write made from the call. In Roc that promise is
enforced by the compiler, not by convention.

## The difference is the arrow

```roc
register         : Spec(state), Method, (state, input => Try({ state : state, out : output }, Str)) -> Spec(state)
registerReadOnly : Spec(state), Method, (state, input ->  output)                             -> Spec(state)
```

The read-only handler uses `->`, the *pure* arrow. It receives the state and the
input and returns only the output — it cannot return a new state, and it cannot
call any `!` effect. Code that tries does not compile.

```roc
statusMethod = method({
	name: "get_status",
	description: "Reports the rollout state without writing anything.",
	promptHint: "",
})

handleStatus : State, {} -> Status
handleStatus = |state, _input| {
	service: state.service,
	canary: state.canary,
	weight: state.weight,
}

agent : Agent(State)
agent =
	define(cfg, |service| { .. })
		|> registerReadOnly(statusMethod, handleStatus)
		|> build
```

Compare a mutating handler, which is effectful and returns the next state:

```roc
handleRollback! : State, RollbackInput => Try({ state : State, out : Str }, Str)
handleRollback! = |state, input| {
	logInfo!("rolling back: ${input.reason}")
	Ok({ state: { ..state, weight: 0 }, out: "rolled back" })
}
```

## Tools too

A published tool carries the same flag:

```roc
tools = [
	{ name: "analyze_logs", description: "Scans log lines for faults.", parameters: [], readOnly: True },
]
```

## Notes

- If a getter needs to read config, that is an effect — `getConfig!` — so it
  cannot be read-only. Read config in a mutating method and keep it in state.
- Keep read-only methods cheap: the runtime may answer them from a cache.
