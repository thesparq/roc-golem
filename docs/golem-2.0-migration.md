# Golem 2.0 protocol migration (scoping)

Golem 1.6+ expects a new agent protocol. This note records what changed, what it
means for this platform, and the work it implies. It is based on the WIT shipped
with `v1.6.0-rc1` (`wit/deps/golem-agent/{guest,common}.wit`), fetched from the
golem repository.

## What changed

`golem:agent/guest@2.0.0` (was `@1.5.0`):

- `initialize` and `invoke` are **`async func`s** — the component-model async
  ABI, not the synchronous canonical ABI this host uses today.
- Values are `schema-value-tree` (`golem:core/types@2.0.0`) instead of
  `data-value`, and `invoke` returns `option<schema-value-tree>` (`none` when the
  method's output schema is `unit`).
- `agent-type` gained `kind: agent-type-kind` and `schema: schema-graph`, and its
  `%constructor`/`methods` describe parameters with `input-schema` /
  `output-schema` plus `named-field` lists instead of `data-schema` tuples.
- A component may instead export `golem:tool/guest@0.1.0` (`discover-tools`) or
  `golem:tool/tool-middleware-guest@0.1.0` (`discover-tool-middlewares`); the
  deploy error lists all three options.

## What it implies for the platform

1. **Async exports in the host.** `wit-bindgen` 0.36 (pinned in
   `host/Cargo.toml`) has no async-export support, so the host moves to a version
   that does and `initialize`/`invoke` become async. The calls into the Roc app
   stay synchronous C-ABI calls, so the hook layer itself does not change shape.
2. **Value model.** The JSON pass-through the platform exposes to apps keeps
   working, but the host must encode/decode `schema-value-tree` rather than
   `data-value`, and `build_agent_type` must be rebuilt around `schema-graph`,
   `input-schema`/`output-schema` and `named-field`.
3. **Roc side unchanged.** The four hooks (`init!`, `handleMessage!`,
   `handleToolCall!`, `metadata`) and the `Str` bridge stay as they are; only the
   host's WIT layer and metadata mapping change.
4. **Compatibility.** 2.0 is not backward compatible with 1.5: a component built
   against `@1.5.0` deploys on 1.5.x servers only (see the version matrix in the
   README). A migration would be a major platform release; shipping both worlds
   behind a build flag is possible but doubles the host surface.

## Suggested sequencing

1. Keep the 1.5 line deployable (it works today — see the README deploy section).
2. Prototype the async host on a branch: bump `wit-bindgen`, generate bindings
   from the 1.6 WIT, and get `discover-agent-types` accepted by a 1.6 server.
3. Rebuild `build_agent_type` for the new schema model (this is where the
   platform's metadata JSON gets mapped onto the WIT types).
4. Extend `tooling/abi_harness` to the 2.0 world — its `harness.wit` world and
   the stubs in `src/stubs.rs` are version-pinned, so they move with it.
5. Cut a platform release for 1.6+ once the smoke test passes on a 1.6 server.
