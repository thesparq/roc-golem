---
name: golem-add-config-roc
description: "Reading per-agent configuration from the manifest in a Roc Golem agent. Use when the user asks to add a region, an endpoint, a feature flag, or any value supplied at deploy time."
---

# Configuration

> **Status: implemented.**

An agent declares the config keys it reads, and the manifest supplies their
values per environment. The app reads them with `getConfig!`, which is an effect
— so config is read inside a handler, not at definition time.

## Declare the keys

```roc
import pf.Golem exposing [Agent, build, define, getConfig!, method, register, withConfig]

configDeclarations = [
	{ name: "region", secret: False },
	{ name: "upstream_url", secret: False },
]
```

`secret: False` means the value is ordinary config. See `golem-add-secret-roc`
before using `secret: True`.

## Chain it in

```roc
agent : Agent(State)
agent =
	define(cfg, |_id| { region: "unknown" })
		|> withConfig(configDeclarations)
		|> register(start, handleStart!)
		|> build
```

## Read it

```roc
handleStart! : State, {} => Try({ state : State, out : Str }, Str)
handleStart! = |state, _input| {
	region = match getConfig!("region") {
		Ok(value) => value
		Err(_) => "unknown"
	}
	logInfo!("starting in ${region}")
	Ok({ state: { ..state, region: region }, out: region })
}
```

`getConfig!` answers `Try(Str, Str)` — config is text, so parse it if you need a
number: `U64.from_str(value)`.

## Supply the values in the manifest

```yaml
agents:
  canary-release:
    config:
      region: "eu-west-1"
      upstream_url: "https://example.test"
```

## Notes

- A key the manifest does not supply is an `Err` at read time, not a startup
  failure. Treat a missing key as a case to handle.
- Config is per agent type per environment. Two agents of the same type share it.
