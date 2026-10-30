---
name: golem-call-another-agent-roc
description: "Calling another agent by name and awaiting its reply in a Roc Golem agent. Use when the user asks two agents to talk, to delegate work to another agent, or to call a method on a different component."
---

# Calling Another Agent

> **Status: implemented.**

`rpc!` invokes a method on another agent and waits for the reply.

```roc
import pf.Golem exposing [rpc!]

match rpc!("effects-agent(\"other\")", "handle-message", "status") {
	Ok(reply) => logInfo!("reply: ${reply}")
	Err(err) => logWarn!("call failed: ${err}")
}
```

Three arguments, no more: the **target agent id**, the **method name**, and the
**parameter**.

## The target is an agent id

`<agent-type>(<constructor params>)`, exactly the string the CLI accepts:

```roc
rpc!("counter-agent(\"c-1\")", "handle-message", "\"increment\"")
```

For an agent in another component, build the id from that component's name as the
manifest declares it:

```roc
match makeAgentId!("roc-golem:counter", "counter-agent(\"c-1\")") {
	Ok(id) => match rpc!(id, "handle-message", "\"get\"") { .. }
	Err(err) => logWarn!("no such agent: ${err}")
}
```

Golem 1.6 resolves the target's agent type from the deployment's registered types,
so a cross-component call works as long as the target component is deployed.

## The parameter is JSON text

Everything crosses as text. For `handle-message` that is the message itself
(`"\"get\""` is the JSON string `"get"`). For a declared method it is that
method's input JSON:

```roc
rpc!("release-guardian(\"prod\")", "start_canary", "{\"version\":\"1.1.0\",\"weight\":10}")
```

## Addressing yourself

`workerId!({})` is this invocation's own agent id — the clean way to schedule or
fork relative to yourself:

```roc
rpcSchedule!(workerId!({}), "emergency_rollback", "{\"reason\":\"timeout\"}", 300000)
```

## Notes

- A call is durable: a replay answers from the recorded result rather than calling
  again. Do not add your own deduplication around it.
- The reply is text. Parse it with the built-in `Json` codec if you need a value.
