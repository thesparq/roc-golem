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
  remote call the agent made, and `--expect-quota` / `--expect-retry` match the
  quota and retry-policy calls the agent made. All of them are recorded by the
  stubs, so effect wiring is verified, not just reply text.
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
- an expected log line, WebSocket send, HTTP request, RPC call, quota call or
  retry-policy call was never recorded.

Current state of the components built from this tree: counter, ai_tool, site and
effects pass every check, including the effects agent's HTTP and RPC calls and
the counter's quota and retry-policy calls.

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
- **Components that import an async host function trap in the harness** (at the
  first export call, `wasm unreachable` inside the guest runtime) under both
  wasmtime 46 and 49. Today that is only the streaming agent, once its
  `websocket/client.receive` import is linked in (it uses `wit_bindgen::block_on`
  to call the async import from the synchronous C-ABI effect). The same
  component initializes and serves requests on a real Golem 1.6 server, so this
  is a fork-vs-wasmtime async-ABI gap in the harness environment, not a platform
  bug. Until it is resolved, treat the e2e run as the authoritative check for
  components with async imports.

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
connection), `wasi:logging` (recorded), and `golem:api/retry` /
`golem:quota/types` (recorded too). Both of the last two are synchronous
interfaces, which is why they can be driven here at all; a component that
imports an *async* host function cannot be (see the toolchain notes above).

`src/stubs.rs` implements the WASI interfaces by hand. wasmtime's own
`wasi:http` implementation tracks a newer WASI version (`wasi:http@0.2.12`) than
Golem's WIT bundle pins (`@0.2.3`), so that crate cannot be linked here; the
stubs were generated from this crate's `bindgen!` expansion and then hand-tuned.

`wit/deps/` is copied from the repo's `wit/deps/`. When the upstream WIT
changes, re-copy these files.
