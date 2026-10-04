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
- `--expect-log` matches a line the agent emitted through `logInfo!` etc., and
  `--expect-sent` matches a WebSocket message the agent sent. Both are recorded
  by the stubs, so effect wiring is verified, not just reply text.

It exits non-zero when any of these fail:

- `get-definition` / `discover-agent-types` do not report the app's agent type
  (the host falls back to `roc-agent` when it cannot read the app's metadata,
  which also makes multi-component deployments fail with
  `Wrapper agent type name roc-agent is defined by multiple components`),
- `initialize` traps or returns an error,
- `invoke handle-message` traps, returns an error, or returns a reply that does
  not contain the expected text,
- an expected log line or WebSocket send was never recorded.

Current state of the components built from this tree: all checks pass for the
three examples, including the streaming agent's connect-then-send flow, and CI
runs this as a required step. `initialize` used to trap inside the allocator path
and poison the instance; see the Development status section of the root README
for what was wrong.

## Why this exists

The link step joins two halves whose memory contract is written down nowhere in
this repo: Roc's compiled code and the Rust host in `host/`. Golem is otherwise
the only thing that would notice a mismatch, and by then it surfaces as a
component that fails to deploy or traps on its first call. The 2026-09-30
release is an example: `counter_agent.wasm` from that release traps inside
`initialize` and reports the fallback agent type. `tooling/glue/abi_facts.roc`
dumps the compiler's own layout facts for comparison.

## WIT copies

`wit/` holds a small, self-contained world (`harness-world`) describing what the
examples' components import, so the harness can supply the missing host
interfaces with no Golem runtime present: agent types, the monotonic clock, the
WebSocket client (stubbed with a recording connection) and `wasi:logging`
(recorded). `wit/deps/` is copied from the repo's `wit/deps/`, trimmed to those
interfaces. When the upstream WIT changes, re-copy these files; the only intended
difference is that `golem-agent/guest.wit` keeps just the `guest` interface and
drops the `agent-guest` / `agent-host` worlds, which import Golem host interfaces
the harness does not model.
