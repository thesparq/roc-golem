---
name: golem-retry-policies-roc
description: "Reading, resolving and walking Golem retry policies in a Roc Golem agent. Use when the user asks about retries, backoff, retry policy, or the delay before a retry."
---

# Retry Policies

> **Status: implemented.**

Retry policy is mostly the runtime's: it retries failed invocations per the
deployment's rules. These effects let an app *see* the policies, choose the one that
applies, and ask how long to wait.

## What is available

```roc
import pf.Golem exposing [
	getRetryPolicies!, getRetryPolicy!, nextRetryDelay!, resolveRetryPolicy!,
	setRetryPolicy!, removeRetryPolicy!,
]
```

| Effect | Answers |
| --- | --- |
| `getRetryPolicies!({})` | a JSON array of named policies |
| `getRetryPolicy!(name)` | one policy's JSON, or `""` |
| `resolveRetryPolicy!(verb, nounUri, propertiesJson)` | the first policy whose predicate matches, or `""` |
| `nextRetryDelay!(policyJson, stateJson, elapsedNanos, contextJson)` | `{"state": [...], "delayNanos": ...}`, delay `null` when exhausted |
| `setRetryPolicy!(name, policyJson)` | records it in the oplog, so it outlives a restart |
| `removeRetryPolicy!(name)` | removes the agent's own copy |

## Choosing a policy and waiting

```roc
checkDelay! = |verb| {
	policies = getRetryPolicies!({})
	chosen = resolveRetryPolicy!(verb, "", "{}")
	match chosen {
		"" => Ok({ state, out: "no policy matches" })
		policyJson =>
			match nextRetryDelay!(policyJson, "[]", 0, "{\"verb\":\"${verb}\",\"nounUri\":\"\",\"properties\":{}}") {
				Err(err) => Err("could not evaluate: ${err}")
				Ok(decision) => Ok({ state, out: decision })
			}
	}
}
```

- `stateJson` is the `state` the previous decision returned — `"[]"` starts a
  sequence. Carry it between attempts.
- `elapsedNanos` is the time since the first attempt.
- A `null` delay means the policy is exhausted. A **zero** delay still allows
  another attempt.

## The manifest declares the defaults

```yaml
retryPolicyDefaults:
  local:
    send-retries:
      priority: 100
      predicate:
        propEq:
          property: "verb"
          value: "send"
      policy:
        countBox:
          maxRetries: 2
          inner:
            periodic:
              secs: 0
              nanos: 100000000
```

A duration is an object (`{secs, nanos}`), not a string like `"100ms"` — a string is
rejected by the schema at deploy time.

## Notes

- The policy trees are a flattened form: the root is `nodes[0]` and children are
  indices into the same list.
- A manifest-declared policy comes back on the next deployment; `removeRetryPolicy!`
  only removes the agent's own copy.
- A predicate the host cannot evaluate is an `Err`, which does not fail the
  invocation.
