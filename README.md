# roc-golem

Write [Golem Cloud](https://golem.cloud) agents in [Roc](https://roc-lang.org).

This is a Roc platform that compiles to a valid [Golem WASM component](https://component-model.dev). The platform handles all WIT type serialization, canonical ABI encoding, and memory management — Roc app code deals only with simple `Str` and `I32` types.

## How it works

```
(Golem Runtime) → (Rust Host: WIT bridge + canonical ABI) → (Roc Platform: dispatch) → (Roc App: business logic)
```

- **`host/`** — Rust crate (`no_std`, `wasm32-unknown-unknown`). Implements Golem guest exports, canonical ABI encoding, delegates to Roc.
- **`platform/`** — Roc platform package. Declares `provides {}` matching the Rust host's `extern "C"` imports, dispatches to the app via `requires {}`.
- **`app/`** — Your Roc agent. Implement the 6 required functions.

## Requirements

- [Roc nightly](https://roc-lang.org/install) (`release-fast-afef9119` or later)
- [wasm-tools](https://github.com/bytecodealliance/wasm-tools) (`cargo install wasm-tools`)
- Python 3 (for memory export fix)
- Rust nightly + `wasm32-unknown-unknown` target (only if rebuilding the host)

## Quick start

```bash
# Build the example agent
bash build.sh

# Output: out/golem-component.wasm — a valid Golem component
wasm-tools validate out/golem-component.wasm
```

## App API

Edit `app/main.roc` to implement your agent. The platform requires:

| Function | Signature | Description |
|----------|-----------|-------------|
| `main` | `{}` | App entry point |
| `getAgentType` | `Str -> Str` | Agent type name → JSON agent-type definition |
| `initialize` | `Str, Str -> I32` | (agent-type, input) → 0=ok |
| `invoke` | `Str, Str -> Str` | (method-name, input) → JSON output |
| `discoverTypes` | `{} -> Str` | () → JSON list of agent types |
| `save` | `{} -> Str` | () → JSON snapshot payload |
| `load` | `Str -> I32` | (snapshot) → 0=ok |

## Platform API (for the platform developer)

Edit `app/main.roc` for your agent. The `build.sh` script:

1. Builds Rust host → wasm32 relocatable object
2. Links host + platform + app via `roc build --target=wasm32`
3. Fixes memory import → export (Roc imports memory from `env`)
4. Embeds WIT metadata via `wasm-tools component embed`
5. Wraps as a WASM component via `wasm-tools component new`
6. Validates the output

## License

UPL-1.0
