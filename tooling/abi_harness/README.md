# ABI harness

Instantiates a built roc-golem component with stubbed Golem imports and drives
its lifecycle, so a broken Roc-to-Rust ABI shows up locally instead of as a
remote deployment failure.

```bash
./tooling/build.sh all
cd tooling/abi_harness
cargo run --release -- \
  --type counter-agent --method get_count --message increment \
  --reply "Counter incremented to 1" \
  ../../build/counter_agent.wasm
```

It exits non-zero when any of these fail:

- `get-definition` / `discover-agent-types` do not report the app's agent type
  (the host falls back to `roc-agent` when it cannot read the app's metadata,
  which also makes multi-component deployments fail with
  `Wrapper agent type name roc-agent is defined by multiple components`),
- `initialize` traps or returns an error,
- `invoke handle-message` traps, returns an error, or returns a payload that is
  not valid JSON / does not contain the expected reply text,
- the reply has no `state` field.

## Why this exists

The link step joins two halves whose memory contract is written down nowhere in
this repo: Roc's compiled code and the Rust host in `host/`. Golem is otherwise
the only thing that would notice a mismatch, and by then it surfaces as a
component that fails to deploy or traps on its first call. The 2026-09-30
release is an example: `counter_agent.wasm` from that release traps inside
`initialize` and reports the fallback agent type.

## WIT copies

`wit/` holds a small, self-contained world (`harness-world`) describing exactly
what a linked component imports, so the harness can supply the missing host
interfaces with no Golem runtime present. `wit/deps/` is copied from the repo's
`wit/deps/` (`golem-core`, `golem-agent`, `clocks`, `io`), trimmed to the
interfaces the harness world needs. When the upstream WIT changes, re-copy
these files; the only intended difference is that `golem-agent/guest.wit` keeps
just the `guest` interface and drops the `agent-guest` / `agent-host` worlds,
which import Golem host interfaces the harness does not model.
