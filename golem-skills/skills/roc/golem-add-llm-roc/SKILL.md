---
name: golem-add-llm-roc
description: "Calling a large language model from a Roc Golem agent. Use when the user asks to use an LLM, generate text, call a model provider, or build an AI agent."
---

# Calling an LLM

> **Status: pending.** The platform has no LLM interface. Nothing below compiles
> today.

## What works instead

A model provider is an HTTP API, and `http!` works, so an agent can call one today —
the same way any other service is called:

```roc
import pf.Golem exposing [http!, getConfig!]

ask! : Str => Try(Str, Str)
ask! = |prompt| {
	match getConfig!("openai_api_key") {
		Err(err) => Err("no key: ${err}")
		Ok(key) => {
			request = Json.to_str({
				url: "https://api.openai.com/v1/chat/completions",
				method: "POST",
				headers: {
					"content-type": "application/json",
					authorization: "Bearer ${key}",
				},
				body: Json.to_str({
					model: "gpt-4o-mini",
					messages: [{ role: "user", content: prompt }],
				}),
			})
			match http!(request) {
				Err(err) => Err("request failed: ${err}")
				Ok(responseJson) => Ok(responseJson)
			}
		}
	}
}
```

Two caveats that matter: the API key is a plaintext secret in app space (see
`golem-add-secret-roc`), and the whole response is buffered as text — so a streaming
completion is not possible.

## What a first-class surface would add

- A typed client per provider, so a request is a record rather than hand-built JSON.
- Host-held credentials, so the key never enters app memory.
- Streaming responses, which need the streaming-method surface this platform does
  not have (`golem-streaming-agent-roc`).
- Golem's own model routing, if the deployment provides one.

## Placeholder

```roc
# <llm client — not yet available>
# Intended shape: a typed call such as
#   Llm.complete!(<model>, <messages>) -> Try(<completion>, <error>)
# with the provider and credentials supplied by the deployment.
```

Until then, `http!` against the provider's REST API is the supported route.
