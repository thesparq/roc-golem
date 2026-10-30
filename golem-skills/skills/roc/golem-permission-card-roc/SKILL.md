---
name: golem-permission-card-roc
description: "Inspecting, deriving and revoking Golem permission cards in a Roc Golem agent. Use when the user asks about authority, capabilities, wallet, permission cards, or restricting what an agent may do."
---

# Permission Cards

> **Status: implemented.**

Golem decides what an agent may do with **cards**. A card is an unforgeable
capability listing grants (`class(owner) @ recipient : verb : resource-id`); the set
an agent holds is its **wallet**, and the runtime checks the wallet before an
operation that needs one.

The effects expose the wallet and the inspect/derive/install/revoke operations. They
do **not** create authority: a card can only be narrowed.

## A card is a `U32` handle

```roc
import pf.Golem exposing [permissionsWallet!, permissionInspect!, permissionDerive!, permissionRelease!]
```

Ownership is explicit, so using a handle twice is an error rather than a
use-after-free:

- **Borrow** — `permissionId!`, `permissionParents!`, `permissionExpiresAt!`,
  `permissionInspect!`, `permissionDerive!`, `permissionDeriveScope!`. The card stays
  usable.
- **Consume** — `permissionRevoke!`, `permissionInstall!`. The handle is spent.
- **Release** — `permissionRelease!`. Drops the card and frees the handle.

## Reading the wallet

```roc
match permissionsWallet!({}) {
	Err(err) => Err("no wallet: ${err}")
	Ok(walletJson) => Ok({ state, out: walletJson })
}
```

The answer is a JSON array of `{"tag":"concrete"|"template"|"scope","handle":N}`.
Each handle is a card this instance now holds — and therefore one it must release if
it does not keep it.

## Deriving a narrower child

```roc
bounds = "{\"lowerPositive\":[{\"class\":\"filesystem\",\"owner\":\"alice@example.test/shop\",\"recipient\":\"alice@example.test/shop\",\"verb\":\"read\",\"resourceId\":\"/data/**\"}],\"lowerNegative\":[],\"upperPositive\":[],\"upperNegative\":[],\"lifespan\":{\"iso8601\":\"P30D\"}}"

match permissionDerive!(handle, bounds) {
	Ok(child) => Ok({ state, out: "derived child ${U32.to_str(child)}" })
	Err(err) => Err("cannot derive: ${err}")
}
```

A child can only narrow its parent: a grant the parent does not cover is refused
(`lower-bound-too-broad` / `upper-bound-too-broad`). `Ok` is the new handle — it is
**not** installed anywhere, and the parent is only borrowed.

## Validating a grant before using it

```roc
match permissionValidateGrant!(grantJson) {
	Ok(_) => Ok({ state, out: "the kernel accepts this grant" })
	Err(err) => Err(err)
}
```

That catches `unknown-resource-class`, `invalid-verb`, `invalid-pattern` locally
rather than at the operation.

## Declaring an agent's own card

An agent type that declares none gets Golem's broad default card. To narrow it, the
manifest declares an initial card:

```yaml
agents:
  permissions-agent:
    initialCard:
      lowerBound:
        positive:
          - "filesystem(?agent) @ ?agent : read : /seed/**"
          - "network() @ ?agent : connect : api.openai.com:443"
          - "card(?account) @ ?agent : inspect : *"
      upperBound:
        negative:
          - "secret(?env) @ ?agent : reveal : billing.*"
```

The `card(?account) @ ?agent : inspect : *` grant is required for an agent to inspect
its own cards; without it `permissionInspect!` reports `not-permitted`.

## Notes

- The principal the invocation ran as is `getPrincipal!({})` — JSON, `null` for a
  tool invocation, which carries no principal.
- Release what you do not keep: a handle neither consumed nor released stays in the
  instance for its lifetime.
- Errors arrive as `"<case-name>: <message>"`, e.g. `not-permitted: ...`.
