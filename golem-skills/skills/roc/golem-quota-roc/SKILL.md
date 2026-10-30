---
name: golem-quota-roc
description: "Reserving and committing resource quota from a Roc Golem agent. Use when the user asks about rate limiting, resource quotas, quota tokens, or charging work against a limit."
---

# Quotas

> **Status: implemented.**

A quota is a resource the manifest declares, with a capacity and an enforcement
action. An agent asks for a token on it, reserves from the token, and commits what
it actually used.

## The flow

```roc
import pf.Golem exposing [newQuotaToken!, quotaCommit!, quotaReserve!]

handleWork! : State, U64 => Try({ state : State, out : Str }, Str)
handleWork! = |state, amount| {
	token = newQuotaToken!("example-requests", 10)
	match quotaReserve!(token, amount) {
		Err(rejected) => Err("quota refused: ${rejected}")
		Ok(reservation) =>
			# ... do the work ...
			match quotaCommit!(reservation, amount) {
				Err(err) => Err("commit failed: ${err}")
				Ok(_) => Ok({ state, out: "used ${U64.to_str(amount)}" })
			}
	}
}
```

## The effects

| Effect | Meaning |
| --- | --- |
| `newQuotaToken!(resourceName, expectedUse)` | `U32` handle. `expectedUse` sizes the token's reservations; it consumes nothing by itself. |
| `quotaReserve!(token, amount)` | `Ok(handle)` for a reservation, `Err` the rejection a `reject`ing resource answered with. |
| `quotaCommit!(reservation, amount)` | Less than reserved returns the unused capacity; more charges the excess as debt. A reservation never committed counts as zero. |
| `quotaSplit!(token, childExpectedUse)` | A second token; both stay in this instance. |
| `quotaMerge!(token, other)` | Consumes `other`; both must name the same resource. |

A handle is spent once — committing twice is an error.

## The manifest declares the resource

```yaml
resourceDefaults:
  local:
    example-requests:
      limit:
        type: Capacity
        value: 10
      enforcementAction: reject
      unit: request
      units: requests
```

`enforcementAction` decides what a reservation does when capacity is short:
`reject` answers an `Err` immediately, `throttle` waits, `terminate` fails the
invocation.

The capacity is **shared by the whole environment**, so a test that exercises
rejection needs a fresh one.

## Notes

- Tokens cannot travel between agents: `quotaSplit!` and `quotaMerge!` work within
  one instance.
- Reserve before an expensive operation, commit after it, so a crash in between
  charges zero rather than the estimate.
