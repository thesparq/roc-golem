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
- `--config` is the agent config handed to `initialize` (the effects example
  takes its base URL from it).
- `--expect-log` matches a line the agent emitted through `logInfo!` etc.,
  `--expect-sent` matches a WebSocket message the agent sent, `--expect-http`
  matches a request line the HTTP stub received, and `--expect-rpc` matches a
  remote call the agent made. All of them are recorded by the stubs, so effect
  wiring is verified, not just reply text.

It exits non-zero when any of these fail:

- `get-definition` / `discover-agent-types` do not report the app's agent type
  (the host falls back to `roc-agent` when it cannot read the app's metadata,
  which also makes multi-component deployments fail with
  `Wrapper agent type name roc-agent is defined by multiple components`),
- `initialize` traps or returns an error,
- `invoke handle-message` traps, returns an error, or returns a reply that does
  not contain the expected text,
- an expected log line, WebSocket send, HTTP request or RPC call was never
  recorded.

Current state of the components built from this tree: all checks pass for the
four examples — including the streaming agent's connect-then-send flow and the
effects agent's HTTP and RPC calls — and CI runs this as a required step.
`initialize` used to trap inside the allocator path and poison the instance; see
the Development status section of the root README for what was wrong.

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
connection) and `wasi:logging` (recorded).

`src/stubs.rs` implements the WASI interfaces by hand. wasmtime's own
`wasi:http` implementation tracks a newer WASI version (`wasi:http@0.2.12`) than
Golem's WIT bundle pins (`@0.2.3`), so that crate cannot be linked here; the
stubs were generated from this crate's `bindgen!` expansion and then hand-tuned.

`wit/deps/` is copied from the repo's `wit/deps/`. When the upstream WIT
changes, re-copy these files.
