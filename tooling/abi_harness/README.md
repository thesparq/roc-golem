# ABI harness

Instantiates a built roc-golem component with stubbed Golem imports and drives
its lifecycle, so a broken Roc-to-Rust ABI shows up locally instead of as a
remote deployment failure.

```bash
./tooling/build.sh all
cd tooling/abi_harness
cargo run --release -- \
  --type counter-agent --method get_count --message increment \
  --reply "Counter incremented to 1" --then-reply "Counter incremented to 2" \
  ../../build/counter_agent.wasm
```

Flags:

- `--reply` matches the app's reply text from one `handle-message` call.
- `--then-message` / `--then-reply` send a second (possibly different) message,
  which is how state persistence and multi-step flows are checked.
- `--tool NAME` / `--tool-reply SUBSTRING` invoke a registered tool (which the
  host routes through the app's `handleToolCall!`) and check its result.
- `--config` is the agent config handed to `initialize` (the effects example
  takes its base URL from it).
- `--expect-log` matches a line the agent emitted through `logInfo!` etc.,
  `--expect-sent` matches a WebSocket message the agent sent, `--expect-http`
  matches a request line the HTTP stub received, `--expect-rpc` matches a
  remote call the agent made, and `--expect-quota` / `--expect-retry` /
  `--expect-host-call` match the quota, retry-policy and `golem:api/host` calls
  the agent made (`trap`, `oplog-commit`, `resolve-component-id`, …).
  `--expect-kv` and `--expect-blob` do the same for the key-value
  (`wasi:keyvalue`: `open-bucket`, `set`, `get`, `delete`, `exists`) and
  blobstore (`wasi:blobstore`: `create-container`, `write-data`, `list-objects`,
  …) calls, `--expect-fs` for the agent's filesystem
  (`wasi:filesystem@0.2.6`: `open-at`, `write`, `read-directory`, …), and
  `--expect-db` for the databases (`golem:rdbms`: `postgres open`,
  `postgres execute(statement=…, params=…)`, `mysql query`, …). All of
  them are recorded by the stubs, so effect wiring is verified, not just reply
  text — and the two stores, the filesystem and the database are stubbed with a
  small store of their own (a relational one for the database: the statements
  the counter's `db` command sends against a tiny store of committed rows and
  one copy per open transaction), so a round trip the component drives has to
  come back out of it changed (for the filesystem that includes *across* the two
  messages, which is what makes the read a real read).
- `--expect-trap SUBSTRING` expects the first message to *fail the invocation*,
  with SUBSTRING in the trap's text, instead of returning a reply: `trap!` never
  returns, so the reply and tool checks are skipped and nothing else is asked of
  that store.
- `--expect-mount PATH` checks the mount path the agent type publishes
  (`site/{id}`), which is how the site example's `metadata.httpMount` is
  verified to survive the trip to the registered agent type.

CI runs these checks against all four examples, and also against a debug build of
the effects example (`roc build --debug`) because the platform has had
build-sensitive ABI bugs.

The harness binds the Golem 1.6 world (`golem:agent/guest@2.0.0`): `initialize`
and `invoke` are async exports, so it runs the guest on an async-enabled
wasmtime and calls them through `Store::run_concurrent`; values are
`schema-value-tree` parameter records, wrapped and unwrapped by the helpers in
`src/main.rs`.

It exits non-zero when any of these fail:

- `get-definition` / `discover-agent-types` do not report the app's agent type
  (the host falls back to `roc-agent` when it cannot read the app's metadata,
  which also makes multi-component deployments fail with
  `Wrapper agent type name roc-agent is defined by multiple components`),
- `initialize` traps or returns an error,
- `invoke handle-message` traps, returns an error, or returns a reply that does
  not contain the expected text,
- an expected log line, WebSocket send, HTTP request, RPC call, quota call,
  retry-policy call, `golem:api/host` call, key-value call, blobstore call,
  filesystem call or database call was never recorded,
- a message expected to trap (`--expect-trap`) returned a reply or failed some
  other way instead.

Current state of the components built from this tree: counter, ai_tool, site and
static_site pass every check, including the effects the counter carries — the
quota and retry-policy calls, the execution-control calls, the key-value and
blobstore round trips, which run against the stubs' own stores, the agent
filesystem round trip, which runs against the stubs' own directory tree, and the
database round trip, which runs against the stubs' own relational store (a
rolled-back and a committed transaction included). The
streaming agent passes here too, `websocket/client.receive` included. The effects
agent cannot even be *instantiated* by the harness: its p3 `wasi:http` import is
not in the harness world (see the toolchain notes below), so the e2e job stays its
authoritative check.

Toolchain notes found while porting to 1.6:

- **Build with the pinned Roc nightly** (`nightly-2026-10-19-601b54c`). A newer
  one produced a counter component whose second `handle-message` trapped in
  `RocStr::drop` → `__rust_dealloc` → `core::panicking::panic` — the hand-written
  `Str` layout/allocator contract in `host/src/roc_std.rs` tracks that
  compiler. `tooling/build.sh` now refuses a mismatched `roc`.
- `Str.trim` in an agent's own code trips the layout-sensitive Roc codegen trap
  (`wasm unreachable`) in the harness — the streaming example's state
  deserializer used it defensively and now parses its compact JSON directly.
  If a component starts trapping in `initialize`, look for `Str.trim` (or
  similar slicing helpers) first.
- **Async host imports work here.** The earlier note that components importing
  an async host function trap in the harness is out of date: the streaming agent
  passes a `--message connect --then-message receive` run, which calls
  `websocket/client.receive` — a WIT `async func` — under wasmtime 46 with
  `component-model-async` enabled. What keeps the *effects* agent out is a
  missing interface rather than an ABI gap: the harness world does not implement
  the p3 `wasi:http` interfaces, so instantiation fails with "a matching
  implementation was not found in the linker" before any call runs.

## Why this exists

The link step joins two halves whose memory contract is written down nowhere in
this repo: Roc's compiled code and the Rust host in `host/`. Golem is otherwise
the only thing that would notice a mismatch, and by then it surfaces as a
component that fails to deploy or traps on its first call. The 2026-09-30
release is an example: `counter_agent.wasm` from that release traps inside
`initialize` and reports the fallback agent type. `tooling/glue/abi_facts.roc`
dumps the compiler's own layout facts for comparison.

## WIT copies and stubs

`wit/` holds a small, self-contained world (`harness-world`) describing what the
examples' components import, so the harness can supply the missing host
interfaces with no Golem runtime present: agent types, the clocks, `wasi:io`,
`wasi:http` (answered from a canned response while recording the request),
`golem:agent/host` (RPC calls recorded), the WebSocket client (a recording
connection), `wasi:logging` (recorded), `golem:api/retry` / `golem:quota/types`
/ `golem:api/host` (recorded too), the two shared stores `wasi:keyvalue` and
`wasi:blobstore` (each with a small in-memory store behind it), the agent's
filesystem `wasi:filesystem@0.2.6` (an in-memory directory tree; the preopens
answer the sandbox root under both `/` and `.` the way Golem does), and the
three `golem:rdbms` database interfaces (a tiny relational store; see
`src/main.rs` for the statement shapes it understands). Those
interfaces are synchronous, which is why they can be driven here at all.

Two functions of the blobstore move a component-model stream — the outgoing
value's `write-body` and the container's `list-objects` — and attaching a host
side producer or consumer to a stream needs the store, so the `bindgen!` call
marks exactly those two `store | trappable` and the stubs implement the
generated `WithStore` companion traits. That is the only configuration the
harness needs; the guest still calls both synchronously, as the WIT declares.

`golem:api/host`'s `trap` is the one stub that cannot fail the way the real host
does: the WIT function has no result an error could travel in, and on the
concurrent path wasmtime lets a panic inside a host call unwind instead of
turning it into a guest trap. The stub panics and the harness catches that panic
itself (`--expect-trap`); the e2e run is what verifies that an invocation really
fails.

`src/stubs.rs` implements the WASI interfaces by hand. wasmtime's own
`wasi:http` implementation tracks a newer WASI version (`wasi:http@0.2.12`) than
Golem's WIT bundle pins (`@0.2.3`), so that crate cannot be linked here; the
stubs were generated from this crate's `bindgen!` expansion and then hand-tuned.

`wit/deps/` is copied from the repo's `wit/deps/`. When the upstream WIT
changes, re-copy these files.
