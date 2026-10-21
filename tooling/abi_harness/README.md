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

- `--reply` matches the app's reply text from one `handle-message` call. It may
  be given more than once: every substring has to be in the reply, which is how a
  multi-step command (`permission roundtrip`) is checked in one run.
- `--then-message` / `--then-reply` send a second (possibly different) message,
  which is how state persistence and multi-step flows are checked. `--then-reply`
  is repeatable too.
- `--tool NAME` / `--tool-reply SUBSTRING` invoke a registered tool (which the
  host routes through the app's `handleToolCall!`) and check its result.
- `--tool-stream NAME` / `--tool-stdin TEXT` invoke `golem:tool/guest.invoke`
  itself — the export `golem tool invoke` drives — with the stdin text supplied
  as one stream chunk and both output writers attached, and
  `--expect-tool-stdout SUBSTRING` / `--expect-tool-stderr SUBSTRING` check what
  the component wrote through them. The writers record their terminal as well,
  so a component that leaves one open fails the check (`abandoned`).
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
  `postgres execute(statement=…, params=…)`, `mysql query`, …), and
  `--expect-permission` for the permission calls (`golem:permissions`:
  `self-wallet`, `inspect-card`, `derive(parent=…, retain=[…], lifespan=…)`,
  `derive-scope`, `revoke-card`, `install-card(card=…, target=…)`, and the
  `drop(card)` a `permissionRelease!` performs), and `--expect-stream` for the
  external-durable-stream calls (`golem:agent/durable-streams`: `reader(…)`,
  `writer(…)`, `read(…)`, `append(…)`, and the `drop(…)` a
  `durableStreamRelease!` performs). All of
  them are recorded by the stubs, so effect wiring is verified, not just reply
  text — and the two stores, the filesystem, the database and the permission
  cards are stubbed with a
  small store of their own (a relational one for the database: the statements
  the counter's `db` command sends against a tiny store of committed rows and
  one copy per open transaction; a card table for permissions, so a derivation
  produces a card the app can inspect, revoke and release; a stream store for
  durable streams, whose appends are merged the way the reference service
  merges them), so a round trip the
  component drives has to
  come back out of it changed (for the filesystem that includes *across* the two
  messages, which is what makes the read a real read).
- The durable-stream stub keeps its streams implicitly, the way the key-value
  stub keeps its buckets: a stream exists once the component has appended to
  it, and reading a URL that was never written is the `not-found` the error
  check drives (a real service answers the same for a stream it does not have).
  Two of the host's own preflight rules are mirrored so the harness catches
  them — a JSON payload needs an `application/json` content type, and each
  element of one has to be encoded JSON — and a read at the stream's own
  checkpoint answers an empty, up-to-date batch, which is what makes the
  second read of the round trip mean something. The offsets are the stub's own
  opaque tokens.
- `--expect-trap SUBSTRING` expects the first message to *fail the invocation*,
  with SUBSTRING in the trap's text, instead of returning a reply: `trap!` never
  returns, so the reply and tool checks are skipped and nothing else is asked of
  that store.
- `--multimodal ENVELOPE` drives the multimodal method (`handle-multimodal`) with
  a value tree built from that JSON envelope — the platform's documented one, a
  list of `{"tag":"text"|"binary","value":{…}}` parts — and renders the tree the
  component answered with back into an envelope. The harness builds and reads
  those trees itself, so both directions are checked against the documented
  envelope rather than against the host's own conversion.
  `--expect-multimodal SUBSTRING` checks the answer envelope and may be given
  more than once. `--multimodal-shape-mismatch` builds the first part's payload
  as a plain string instead of a text value (what a caller's value would look
  like if the declared schema were a bare string) and
  `--expect-multimodal-error SUBSTRING` expects the call to *fail* with that text:
  a conversion the host cannot do is the invocation's error, never a trap.
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
  filesystem call, database call or durable-stream call was never recorded,
- a `--multimodal` answer envelope does not contain an expected substring, or a
  call expected to be refused (`--expect-multimodal-error`) answered instead,
- a `--tool-stream` call failed, returned an error or trap, answered with
  attachment streams instead of using the writers, left a writer unfinished, or
  wrote something the `--expect-tool-stdout` / `--expect-tool-stderr` substring
  is not in,
- a message expected to trap (`--expect-trap`) returned a reply or failed some
  other way instead.

Current state of the components built from this tree: counter, ai_tool, site and
static_site pass every check, including the effects the counter carries — the
quota and retry-policy calls, the execution-control calls, the key-value and
blobstore round trips, which run against the stubs' own stores, the agent
filesystem round trip, which runs against the stubs' own directory tree, the
database round trip, which runs against the stubs' own relational store (a
rolled-back and a committed transaction included), the durable-stream round
trip, whose appends the stub's own stream store answers back (the JSON payload,
its byte length, the checkpoint the second read continues from, and the
`not-found` of a stream that does not exist included), and the multimodal method,
whose parts the harness builds and whose answer it reads back as the documented
envelope. The ai_tool checks also drive
`golem:tool/guest.invoke` with stdin and both output writers (`--tool-stream`),
which is the only call whose parameters are component-model streams. The
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
  `component-model-async` enabled, and the counter's `stream` commands drive
  `durable-stream-reader.read` and `durable-stream-writer.append` the same way
  (both are `async func`s on a resource). What keeps the *effects* agent out is a
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
`src/main.rs` for the statement shapes it understands), and the six
`golem:permissions` interfaces (a small card table of their own, so a derivation
produces a card the app can inspect, revoke and release; the two refusals the
checks drive — an `account` grant in a derivation and a non-agent installation
target — are answered the way the runtime answers them, everything else is
accepted, and the `drop` a `permissionRelease!` performs is recorded). Those
interfaces are synchronous, which is why they can be driven here at all. The
world also *exports* `golem:tool/guest@0.1.0` next to `golem:agent/guest@2.0.0`,
so `--tool-stream` can call the tool export; its `tool-output-writer` stubs
record the bytes and the terminal the component selected (a writer dropped open
reads `abandoned`, the way the real host treats it).

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

The two durable-stream resources are stubbed in `src/main.rs` rather than
`src/stubs.rs`: their methods are async imports of a `golem:agent` interface,
not WASI, so they need the store's own state (the recorded calls and the stream
table) the way the WebSocket and permission stubs do.

`src/stubs.rs` implements the WASI interfaces by hand. wasmtime's own
`wasi:http` implementation tracks a newer WASI version (`wasi:http@0.2.12`) than
Golem's WIT bundle pins (`@0.2.3`), so that crate cannot be linked here; the
stubs were generated from this crate's `bindgen!` expansion and then hand-tuned.

`wit/deps/` is copied from the repo's `wit/deps/`. When the upstream WIT
changes, re-copy these files.
