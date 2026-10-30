---
name: golem-custom-snapshot-roc
description: "Controlling how a Roc Golem agent's state is serialized and restored. Use when the user asks to migrate an old state format, store state compactly, or survive an incompatible revision change."
---

# Custom State Snapshots

> **Status: implemented.**

State crosses every invocation as JSON, and the platform derives the codec from
the state type. When you need the *text* — a migration, a compact form — write the
codecs yourself.

## Explicit codecs

```roc
import pf.Golem exposing [Agent, defineAgent]

State : { count : I64, note : Str }

agent : Agent(State)
agent =
	defineAgent({
		init!: |_config| { count: 0, note: "fresh" },

		handleMessage!: |state, message| {
			{ state, replies: ["count is ${I64.to_str(state.count)}"] }
		},

		serializeState: |state|
			Json.to_str({ count: state.count, note: state.note }),

		deserializeState: |stateJson| {
			parsed : Try({ count : Try(I64, [Missing]), note : Try(Str, [Missing]) }, [InvalidJson(Str)])
			parsed = Json.parse(stateJson)
			match parsed {
				Ok(fields) => Ok({ count: fields.count ?? 0, note: fields.note ?? "migrated" })
				Err(_) => Err("could not read the stored state")
			}
		},
		# ... handleRequest!, handleMultimodal!, handleToolCall!, metadata
	})
```

The `?? default` on a `Try(field, [Missing])` is how Roc reads an absent field —
this is the shape that lets an old `{"count":3}` migrate into a state that now has
a `note`.

## Snapshots across a revision change

Golem can carry an agent across an *incompatible* component revision by snapshot.
The platform exports `golem:api/save-snapshot` / `load-snapshot` backed by this
same state JSON, so updating an agent (`golem agent update <id> manual`) hands the
old revision's JSON to the new revision's `deserializeState`. That is the hook
where a migration belongs: read the old shape, return the new one.

## Notes

- `serializeState` must be a pure function — it runs outside a normal handler.
- Keep the stored shape stable. Renaming a field is a migration, not a rename: the
  data already on disk carries the old name.
- The derived codec is fine for a state whose shape never changes. Reach for these
  hooks when it does.
