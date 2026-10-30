---
name: golem-add-secret-roc
description: "Reading a secret from the environment's secret store in a Roc Golem agent. Use when the user asks for an API key, a password, a token, or any value that must not sit in the manifest."
---

# Secrets

> **Status: partial.** A secret reaches the app as its **value**. There is no
> opaque handle, so an app holds the plaintext and cannot ask the host to use the
> secret on its behalf.

## Declaring

```roc
configDeclarations = [
	{ name: "region", secret: False },
	{ name: "api_key", secret: True },
]
```

```roc
agent =
	define(cfg, |_id| { .. })
		|> withConfig(configDeclarations)
		|> build
```

## Reading

```roc
handleCall! : State, {} => Try({ state : State, out : Str }, Str)
handleCall! = |state, _input| {
	key = match getConfig!("api_key") {
		Ok(value) => value
		Err(err) => return Err("no api key: ${err}")
	}
	# `key` is the plaintext secret.
	request = Json.to_str({
		url: "https://example.test/api",
		method: "GET",
		headers: { authorization: "Bearer ${key}" },
	})
	ignoreEffect(http!(request))
	Ok({ state, out: "called" })
}
```

Wait — Roc has no `return`. Write it as a `match`:

```roc
handleCall! = |state, _input| {
	match getConfig!("api_key") {
		Err(err) => Err("no api key: ${err}")
		Ok(key) => {
			request = Json.to_str({
				url: "https://example.test/api",
				method: "GET",
				headers: { authorization: "Bearer ${key}" },
			})
			match http!(request) {
				Ok(_) => Ok({ state, out: "called" })
				Err(err) => Err("request failed: ${err}")
			}
		}
	}
}
```

## Supplying the value

A `secret: True` key must be provided as a secret by the environment, not as plain
`config:`. The manifest's plain `config:` block is for `secret: False` keys.

## The gap, stated plainly

- **No handle.** `getConfig!` returns the secret's text. Passing it to `http!` means
  the plaintext is in the app's memory and in the request the app builds.
- **No host-mediated use.** There is no way to say "authenticate this request with
  that secret" and never see it.
- **It follows that authenticated external durable streams are not available
  either** — the stream constructors take a secret the platform cannot supply.

Treat any secret an app reads as being in app space. For the target shape:

```roc
# <secret handle — not yet available>
# The intended surface is a `Secret(a)` capability the host resolves, so an app
# can hand it to an effect without reading the value.
```

## Notes

- Never log a secret, and never put one in a returned `out`.
- Prefer a short-lived token from the environment over a long-lived key.
