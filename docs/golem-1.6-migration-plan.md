# Golem 1.6 migration plan

Status: draft for review · 2026-10-12
Supersedes the scoping note in `docs/golem-2.0-migration.md` (that note is still
accurate about the protocol change; this plan is the actionable version).

Target: Golem **v1.6.0** (GA 2026-10-19, launch event 13:00–15:00 EDT).
Verified against **v1.6.0-rc2** (published 2026-10-11): the WIT bundle, the
SDK releases (`golem-rust-v3.0.0-rc3`, `golem-ts-v2.0.0-rc3`, `golem-effect-v2.0.0-rc2`,
`golem-scala-v2.0.0-RC2`, `golem-moonbit-v0.6.0-rc4`) and the announcement post
"Golem 1.6: Agents, Tools, and Artifacts" (2026-10-09).

Local copies of the key 1.6 WIT files: `~/.cache/golem-rc2/wit/{agent,core,tool}`.

---

## 1. Decision summary

- **We build a second platform line for 1.6** (`v0.6.x`, branch `feat/golem-1.6`),
  keeping the 1.5 line (`v0.5.x`) frozen for critical fixes only. The two ABIs are
  mutually incompatible: 1.5 components do not run on 1.6 servers and vice versa.
- The critical path is a **P0 spike**: a minimal counter agent exporting
  `golem:agent/guest@2.0.0` deployed and invoked on a 1.6 server. Everything else
  (tools, snapshots, new effects, artifacts) layers on after that works.
- The 1.6 release is themed *agents, tools, and artifacts*; the plan below targets
  all three, in that order, plus the authority model (permission cards, secrets).

---

## 2. The protocol delta (1.5.9 → 1.6.0-rc2), verified

| Surface | 1.5.x (what we ship today) | 1.6.0-rc2 |
| --- | --- | --- |
| Agent export | `golem:agent/guest@1.5.0`, synchronous | `golem:agent/guest@2.0.0`: **`initialize` and `invoke` are `async func`**; `get-definition` / `discover-agent-types` stay sync |
| Value model | `data-value` / `data-schema` tuples | `schema-value-tree` + `schema-graph` (`golem:core/types@2.0.0`): flat-with-indices recursive model, `typed-schema-value`, capability handles |
| Required exports | `golem:agent/guest` | **`golem:agent/guest@2.0.0` only** (verified against rc2 source, §7.1). The CLI's deploy check is an **OR** across agent/tool/middleware discovery; the server requires `golem:tool/guest@0.1.0` only when the deployment declares tools, and `tool-middleware-guest@0.1.0` only when it declares middlewares; snapshot exports are needed only when snapshotting is enabled. The SDKs export all of them with empty fallbacks. |
| `agent-type` | name, description, source-language, constructor, methods, dependencies, mode, http-mount, snapshotting, config | + `kind` (`regular` / `http-router`), `schema: schema-graph`, per-field `input-schema` (`named-field` with `field-source`: `user-supplied` / `auto-injected(principal)`), `output-schema` (`unit` / `single(index)`), `read-only` cache policies (`no-cache` / `until-write` / `ttl`), richer `http-mount` (static + filesystem bindings, `openapi-provider-method`, phantom agents, durable-stream routes) |
| RPC (`golem:agent/host`) | `WasmRpc::new` + `invoke_and_await` | resource with `create`/`constructor`, `invoke-and-await`, `invoke` (fire-and-forget), `async-invoke-and-await` (future), `schedule-invocation`, `schedule-cancelable-invocation`, permission-card scoping; `make-agent-id` / `parse-agent-id`; `get-agent-type[s]`; `create-webhook`; `get-config-value` |
| Persistence | `set-oplog-persistence-level` + persistence levels | **removed** — durable agents are durable all the way; our `setPersistence!` effect dies with 1.5 |
| New imports available | — | `golem:agent/durable-streams@2.0.0`, `golem:secrets/{types,reveal}@0.1.0`, `golem:permissions/*@0.1.0` (vouch), `golem:tool/{host,underlying}@0.1.0` |
| Versions that stay | `golem:api/host@1.5.0`, `api/retry@1.5.0`, `api/context@1.5.0`, `api/oplog@1.5.0`, `rdbms/*@1.5.0`, `websocket/client@1.5.0`, `quota/types@1.5.0` | `golem:durability@1.6.0` (bumped), `wasi:clocks` 0.2.6 **and** 0.3.0, `wasi:http@0.2.6`, blobstore / keyvalue / config-store / logging |
| Runtime | sync component model | **WASI P3 + Wasmtime 46, async component model, streaming HTTP** |
| Toolchain | `wit-bindgen` 0.36, `wasm-tools` 1.220 | **`wit-bindgen` fork `golemcloud/wit-bindgen`, branch `wit-bindgen-golem-1.6` (=0.59.0)** with `async`, `async-spawn`, `inter-task-wakeup`; **`wasm-tools` ≥ 1.248.0** (Golem's own CI pins 1.248.0) |

Breaking changes called out in the announcement that touch us: components must be
rebuilt with 1.6 SDKs; manifests auto-upgrade except bridge config (moves under
`external`); persistence levels and environment shares removed; oplog/REST payload
shapes change; a migration guide ships with the GA on Oct 19.

### Why the wit-bindgen fork is required

The 2.0 schema model makes the generated lift/lower wrappers enormous. Golem's
fork ("outline-lift") moves them out of the main function; upstream cannot build
this today. Two reference pins in the wild:

- SDK: `branch = "wit-bindgen-golem-1.6"`, `version = "=0.59.0"`, features
  `["async", "async-spawn", "inter-task-wakeup"]` (`sdks/rust/golem-rust/Cargo.toml`).
- Effect SDK: exact rev `4407232ead86d9bcbd06cbebd790a52120a4087a` of the same
  fork (`effect-golem` branch `golem-1.6`, `scripts/generate-agent-template.mjs`).

---

## 3. Impact on this repository, file by file

### `wit/` — full swap
Replace the 1.5 bundle with the 1.6.0-rc2 bundle (`wit/host.wit` + `wit/deps/*`),
including the new `golem-core-v2`, `golem-tool`, `golem-secrets`,
`golem-permissions`, `golem-agent@2.0.0` packages. Our world keeps exporting
`golem:agent/guest@2.0.0` and must add `golem:tool/guest@0.1.0`,
`golem:api/save-snapshot@1.5.0`, `golem:api/load-snapshot@1.5.0`.

### `host/Cargo.toml`
- `wit-bindgen` 0.36 → the Golem fork (pin the branch, or the exact rev for
  reproducibility; the SDK uses the branch, effect-golem pins a rev).
- Consider `wasip3` only if we need P3-native types; the fork embeds its own
  runtime (no separate `wit-bindgen-rt`).
- `serde_json` stays (our JSON bridge to Roc apps is unchanged in spirit).

### `host/src/lib.rs` — the bulk of the work
1. **Async exports.** `initialize` / `invoke` become `async fn` in the generated
   trait. The Roc side stays a synchronous C-ABI call, so the hook layer shape is
   unchanged — the async is only at the WIT boundary.
2. **`build_agent_type` → schema model.** Build a `schema-graph` for the agent
   (name/description/version/tools → type nodes), map the constructor and each
   method to `input-schema` (`named-field` list) and `output-schema`
   (`single(string)` today), keep `mode: durable`, `snapshotting`, and add
   `kind: regular`. The existing strict-metadata behaviour (fail loudly on
   unreadable metadata) carries over — with `Err(agent-error)` now the natural
   channel for `discover-agent-types`.
3. **Value codec.** New JSON ↔ `schema-value-tree` conversion for constructor
   input and method input/output. Today's app-facing contract (a single `Str` in,
   a `Str` out) can be preserved by declaring string schemas; the codec then
   wraps/unwraps a one-node tree. This keeps all four examples' Roc code valid.
4. **New exports (optional until their feature lands).** `golem:tool/guest@0.1.0`
   (expose the app's tools) and `save-snapshot` / `load-snapshot` (state-JSON
   snapshot wired to Roc). None of these are required for a 1.6 component to
   deploy or run (§7.1) — add each when the corresponding feature lands. Caveat:
   once the manifest declares tools/middlewares, the matching guest export
   becomes a hard server requirement.
5. **Effects updates.** `rocFxGetWorkerId` → `get_self_metadata` now returns
   `result<agent-metadata, _>`; `rocFxSetPersistence` and `PersistenceLevel`
   disappear (remove the effect on the 1.6 line). `rocFxRpcInvoke` moves to the
   new `wasm-rpc` resource shape. HTTP/WebSocket/clocks/logging keep working
   (interface versions unchanged for websocket; clocks has 0.2.6 available).

### `platform/*.roc`
- Hooks (`init!`, `handleMessage!`, `handleToolCall!`, `metadata`) are unchanged;
  the `Str` bridge stays.
- `Golem.roc`: drop `setPersistence!`; everything else keeps its signature.
- `Effect.roc`: drop `setPersistenceMode!`; add new effects in phases (section 5).
- Metadata shape stays `{name, version, description, tools}` — the host maps it
  onto the schema model. Richer metadata (per-method input schemas, read-only
  flags, HTTP mounts) can be added later without breaking apps.

### `tooling/abi_harness`
- New world + stubs for the 1.6 imports; async invocation support. The harness
  already uses Wasmtime 49 with `component-model`; it needs the async feature
  enabled and `call_*` awaited. The `--tool`/`--tool-reply` flags map naturally
  onto `golem:tool/guest` checks later.

### `tooling/build.sh`
- `wasm-tools component new` must move to ≥ 1.248.0 (async lowering). Verify
  whether componentizing async exports needs extra flags; record the exact
  command in the script and the README.

### `golem.yaml` + deploy
- Bump `manifestVersion` to 1.6 (the 1.6 CLI auto-migrates; bridge config, if we
  ever add it, goes under `external`). Add tool/middleware bindings only when we
  adopt the standard toolkit.

### CI / e2e
- Keep the 1.5.9 job green on the 1.5 branch.
- Add a 1.6 matrix leg: install `v1.6.0-rc2` (then GA) CLI+server, run the ABI
  harness and `tooling/e2e.sh` against it. The e2e script only needs
  `GOLEM_CLI`/port overrides — it is already parameterised.

---

## 4. What "fully featured" means here (capability roadmap)

Ordered by value/effort. T0–T1 are in scope for the first 1.6 release; T2–T3
are follow-ups.

**T0 — parity (must have)**
- All four examples run on 1.6: counter, ai_tool, streaming_agent, effects.
- Metadata/schema mapping, strict failure on broken metadata.
- Harness + e2e green against a 1.6 server; CI matrix.

**T1 — the 1.6 headline capabilities we can expose to Roc apps**
- **Agent tools**: expose the app's declared tools through `golem:tool/guest`
  (typed command tree built from metadata), and invoke ambient/standard tools
  (`bash`, `read-file`, `git`, …) through `golem:tool/host` — this is what makes
  a Roc agent agentic rather than just durable.
- **Snapshots**: user-defined `save-snapshot` / `load-snapshot` wired to Roc
  hooks, enabling manual updates of incompatible agents.
- **RPC upgrades**: fire-and-forget `invoke`, `async-invoke-and-await` futures,
  `schedule-invocation` (+ cancellation) as Roc effects.
- **Config**: declared config keys (local + secret) read through
  `get-config-value`.
- **Read-only methods**: metadata flag + cache policy, enforced by the runtime.

**T2 — authority and streaming**
- **Secrets**: opaque `secret` handles passed to tools/host capabilities;
  `reveal` only where explicitly wanted (audited in the oplog).
- **Durable Streams**: read/append effects against external streams
  (checkpoints, long-poll, SSE), and durable-stream routes on HTTP mounts.
- **Permission cards**: pass and scope cards (thin first: hold/pass/inspect),
  `golem card` integration in docs.

**T3 — artifacts and the wider toolkit**
- **HTTP routers / artifacts**: `http-router` agent kind, static + filesystem
  bindings, OpenAPI provider method, subdomain claim — so a Roc agent can ship a
  front-end/report from its own filesystem.
- **RDBMS / blobstore / keyvalue / retry-policy** effects.
- **Tool middleware** authored in Roc (monomorphic + universal scopes).

---

## 5. Phased plan

Each phase ends with a concrete, checkable exit criterion. Estimates are rough
days of focused work.

**P0 — spike (1–2 d)**
Branch `feat/golem-1.6`. Swap the WIT bundle, move the host to the forked
wit-bindgen, make `initialize`/`invoke` async, build a minimal counter
component, deploy to a local `v1.6.0-rc2` server, invoke via CLI.
Exit: `"Counter incremented to 1"` rendered from a 1.6 server. Records the exact
`wasm-tools` command and component size. (Only the agent guest export is
required — §7.1.)

**P1 — parity (3–5 d)**
Full metadata → `schema-graph` mapping, JSON↔value-tree codec, all four examples
ported, harness world/stubs updated and async-capable, unit tests for the codec.
Exit: harness suite green on all four components; `tooling/e2e.sh` green against
a 1.6 RC server.

**P2 — tools + snapshots (3–5 d)**
Expose app tools via `golem:tool/guest`; wire `save-snapshot`/`load-snapshot` to
Roc hooks; config declarations; read-only method flag.
Exit: e2e checks a tool call through the tool protocol and a manual
snapshot-based update of the counter agent.

**P3 — effects expansion (3–5 d)**
Roc effects for the upgraded RPC surface, config, secrets, durable streams.
Exit: examples (or a new `agentic` example) exercise each effect live in e2e.

**P4 — artifacts + authority depth (3–5 d)**
HTTP router kind, static/filesystem bindings; permission cards thin surface.
Exit: a Roc agent serves a file from its filesystem through its mount, e2e-checked.

**P5 — CI, docs, release (2–3 d)**
1.6 matrix in CI, README/version-matrix rewrite, migration notes for 1.5 users,
platform release `v0.6.0` + tag, packaged against 1.6 GA.
Exit: tagged release installable from the docs snippet, CI green on both lines.

**P6 — 1.5 LTS policy**
Freeze `v0.5.x`; backport only critical fixes; document the cut-over in the README.

---

## 6. Verification strategy

- **Unit**: schema codec round-trips (JSON → value tree → JSON), metadata
  mapping, strict-failure paths.
- **ABI harness**: the existing five checks, ported to the 1.6 world; plus
  `--tool` checks routed through the tool protocol once P2 lands.
- **e2e**: `tooling/e2e.sh` against a 1.6 server (deploy, invoke, state, tools,
  HTTP, RPC), plus snapshot update and a tool-protocol check in later phases.
- **CI matrix**: 1.5.9 (existing line) + 1.6 RC → GA.
- **Size budget**: record component sizes per phase (the schema model inflates
  lift/lower; outline-lift mitigates — measure, don't assume).

---

## 7. Risks and open questions

1. ~~**Mandatory exports**~~ — **resolved against v1.6.0-rc2 source**: only
   `golem:agent/guest@2.0.0` is required. The "discover-agent-types in interface
   … or discover-tools … or discover-tool-middlewares …" message is a
   **CLI-side** build/deploy check (`golem-common/src/model/agent/extraction.rs`),
   and it is an OR across the three interfaces. The server
   (`golem-registry-service/src/services/component/write.rs`,
   `validate_component_metadata_invariants`) enforces `golem:tool/guest@0.1.0`
   only when the deployment declares tools, and `tool-middleware-guest@0.1.0`
   only when it declares middlewares. Snapshot exports are never required up
   front; a snapshot-enabled agent that omits `save-snapshot` fails lazily at
   snapshot time. The `wit/host.wit` "every agent component exports these
   interfaces" comment describes SDK output, not an enforced contract.
2. **Fork stability** — the wit-bindgen fork is the only supported path for the
   schema model. Mitigation: pin the exact rev we verify with; keep the branch
   pin in a comment; watch upstream for an equivalent feature.
3. **wasm-tools async componentization** — ≥ 1.248.0 required; our CI pins
   1.220.0. Bump and record the exact invocation. (Open: any extra flags.)
4. **Component size / compile time** — outline-lift exists because wrappers
   explode; measure in P0 and keep the Roc glue thin.
5. **Dual maintenance** — 1.5 and 1.6 lines diverge in `host/` and `wit/`; keep
   the Roc-facing contract identical so `platform/*.roc` and examples differ
   only where the protocol demands (e.g. `setPersistence!`).
6. **Manifest/migration guide** — GA ships a migration guide (Oct 19); read it
   before P1 finishes and fold any deltas into this plan.
7. **Permission model maturity** — vouch v2 adds per-(agent, tool, middleware)
   binding axes; keep the Roc surface thin until it settles.
8. **Tool model fidelity** — the command-tree model (positionals, options,
   flags, constraints, stream specs, error cases) is large. Start with
   metadata-driven simple commands; full fidelity is a follow-up.

---

## 8. References

- Announcement: https://golem.cloud/blog/golem-1-6-agents-tools-and-artifacts/
  (2026-10-09; GA Oct 19, launch 13:00–15:00 EDT)
- Release: https://github.com/golemcloud/golem/releases/tag/v1.6.0-rc2
- WIT (rc2): `wit/host.wit`, `wit/deps/golem-agent/{guest,common,host,durable-streams}.wit`,
  `wit/deps/golem-core-v2/golem-core-v2.wit`, `wit/deps/golem-tool/*`,
  `wit/deps/golem-secrets/*`, `wit/deps/golem-permissions/*`
- Rust SDK (rc3): `sdks/rust/golem-rust` at tag `golem-rust-v3.0.0-rc3` —
  `src/agentic/exports.rs` is the canonical export wiring.
- Effect SDK migration: `golemcloud/effect-golem` branch `golem-1.6`.
- Toolchain pins: `sdks/rust/golem-rust/Cargo.toml` (fork branch + features),
  `effect-golem/scripts/generate-agent-template.mjs` (fork rev),
  golem CI `wasm-tools@1.248.0`.
- Related posts: "The Rise of the Agent Runtime" (2026-06-10),
  "Making decisions with Jev in Golem" (2026-09-29, uses a 1.6 pre-release),
  "Golem 1.5: The Agent Runtime" (2026-05-08).
