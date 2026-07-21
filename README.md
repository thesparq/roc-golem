# roc-golem

Write [Golem Cloud](https://golem.cloud) agents in [Roc](https://roc-lang.org).

A Roc platform that handles all WIT type serialization, canonical ABI encoding, and memory management — your app code deals with simple `Str` and `I32`.

```
(Golem Runtime) → (Rust Host: WIT bridge + ABI) → (Roc Platform: dispatch) → (Your App: business logic)
```

## Using the platform

The platform is distributed as a `.tar.zst` bundle from [Releases](https://github.com/<your-org>/roc-golem/releases).

In your Roc app, reference the platform by its release URL:

```roc
app [main, getAgentType, initialize, invoke, discoverTypes, save, load] {
    pf: platform "https://github.com/<your-org>/roc-golem/releases/download/v0.1.0/<hash>.tar.zst"
}
```

Or for local development, use a relative path:

```roc
app [main, getAgentType, initialize, invoke, discoverTypes, save, load] {
    pf: platform "../platform/main.roc"
}
```

Then build:

```bash
bash build.sh
# Output: out/golem-component.wasm — a valid Golem component
wasm-tools validate out/golem-component.wasm
```

## App API

| Function | Signature | Description |
|----------|-----------|-------------|
| `main` | `{}` | App entry point |
| `getAgentType` | `Str -> Str` | Agent type name → JSON agent-type definition |
| `initialize` | `Str, Str -> I32` | (agent-type, input) → 0=ok |
| `invoke` | `Str, Str -> Str` | (method-name, input) → JSON output |
| `discoverTypes` | `{} -> Str` | () → JSON list of agent types |
| `save` | `{} -> Str` | () → JSON snapshot payload |
| `load` | `Str -> I32` | (snapshot) → 0=ok |

## Repo structure

| Path | Purpose |
|------|---------|
| `platform/main.roc` | Platform declaration + provides→requires dispatch |
| `platform/targets/wasm32/host.wasm` | Pre-built Rust host binary |
| `app/main.roc` | Demo agent — start here |
| `host/src/lib.rs` | Rust host source (Golem exports + canonical ABI) |
| `wit/` | WIT dependency files (golem 1.5.0 + wasi) |
| `build.sh` | Full build pipeline |
| `scripts/bundle.py` | Build Rust host + produce platform bundle |
| `golem.yaml` | Golem app manifest |

## Requirements

- [Roc nightly](https://roc-lang.org/install) (`release-fast-afef9119` or later)
- [wasm-tools](https://github.com/bytecodealliance/wasm-tools) (`cargo install wasm-tools`)
- Python 3 (for memory export fix)
- Rust nightly + `wasm32-unknown-unknown` (only if rebuilding the host)

## Build pipeline

1. Builds Rust host → wasm32 relocatable object
2. Links host + platform + app via `roc build --target=wasm32`
3. Fixes memory import → export (Roc imports memory from `env`)
4. Embeds WIT metadata via `wasm-tools component embed`
5. Wraps as WASM component via `wasm-tools component new`
6. Validates output

## License

UPL-1.0
