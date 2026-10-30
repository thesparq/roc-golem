---
name: golem-configure-durability-roc
description: "Controlling an invocation's persistence, traps and oplog commits in Roc. Use when the user asks about persistence levels, deliberate failure, idempotency keys, or forcing the oplog to commit."
---

# Durability Controls

> **Status: implemented, except atomic regions** (see the end).

## Persistence level

```roc
import pf.Golem exposing [setPersistence!]

setPersistence!(PersistStateOnly)
```

`PersistNothing`, `PersistStateOnly`, `PersistEverything`. Use the narrowest level
an invocation can tolerate: `PersistNothing` for a pure read that needs no replay
record, `PersistEverything` when the invocation's side effects must be replayed
exactly.

## Deliberate failure

```roc
trap!("the upstream contract is broken")
```

`trap!` fails the invocation permanently. Golem does not retry it and the app gets
no result to branch on, so use it for a condition that must not be retried — not
for an ordinary error. An error belongs in the handler's `Err`.

## Idempotency keys

```roc
key = idempotencyKey!({})
```

A durable key, as a UUID string, derived from the call's position in the
invocation log. A replay hands the same call the same key, which is what makes a
repeated write duplicate-safe at the peer. Pass it as whatever the peer treats as
an idempotency key; two calls in one invocation get two different keys.

## Committing the oplog

```roc
oplogCommit!(2)
```

Blocks until the invocation log is durable on that many replicas (or all, when the
deployment has fewer). It is a replication barrier for this agent's own history —
not a transaction, and no guarantee about an external system.

## Forking

`fork!` splits the agent in two from this point; both copies continue independently.
See `golem-parallel-workers-roc`.

## Atomic regions — not available

There is no `atomically(|| { ... })`: a way to group effects so they commit or fail
together. Everything an agent does is already recorded in its invocation log, so
the wrapper would add ceremony rather than a guarantee. If you need
all-or-nothing across an *external* system, it has to be that system's transaction:

```roc
# <atomic region — not available>; use the peer's own transaction instead
```
