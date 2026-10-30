---
name: golem-make-http-request-roc
description: "Making an outgoing HTTP request from a Roc Golem agent. Use when the user asks to call a REST API, fetch a URL, POST JSON to a service, or call a webhook from an agent."
---

# Making an HTTP Request

> **Status: implemented.**

`http!` sends one request and answers the whole response. It goes through
`wasi:http/client` (p3), and the call is recorded in the invocation log, so a
replay is answered from the recording instead of reaching the service again.

## The request

```roc
import pf.Golem exposing [http!]

request = "{\"url\":\"https://example.test/api/notes\",\"method\":\"GET\"}"

match http!(request) {
	Ok(responseJson) => parseStatus(responseJson)
	Err(err) => Err("request failed: ${err}")
}
```

Fields, as the host reads them:

| Field | Meaning |
| --- | --- |
| `url` | required (`uri` is accepted as an alias) |
| `method` | defaults to `GET` |
| `headers` | either an object `{"content-type":"application/json"}` or an array of `["name","value"]` pairs |
| `body` | a string, sent as UTF-8; any other JSON value is serialized |

A bare `"https://..."` string is also accepted as the whole request and is treated
as a `GET`.

## POSTing JSON

```roc
request = Json.to_str({
	url: "https://example.test/api/notes",
	method: "POST",
	headers: { "content-type": "application/json" },
	body: Json.to_str({ note: "hello", id: 1 }),
})
```

`body` is a string, so a JSON payload is encoded once into that string.

## The response

```json
{"status": 200, "headers": {"content-type": "text/plain"}, "body": "..."}
```

```roc
parseStatus : Str -> Try({ state : State, out : Str }, Str)
parseStatus = |responseJson| {
	parsed : Try({ status : Try(U16, [Missing]), body : Try(Str, [Missing]) }, [InvalidJson(Str)])
	parsed = Json.parse(responseJson)
	match parsed {
		Err(_) => Err("the response was not JSON")
		Ok(r) => Ok({ state: {}, out: "${U16.to_str(r.status ?? 0)} ${r.body ?? \"\"}" })
	}
}
```

- `status` is a number, not text — a `4xx`/`5xx` is a **successful** `http!` call.
  Check it; the effect only fails on transport problems.
- `headers` is an object, so a repeated header collapses to one value.
- `body` is lossy UTF-8 text: bytes that are not valid UTF-8 are replaced. For a
  binary payload, fetch it as text and decode, or use the blobstore.

## Notes

- Keep the URL out of the code: read it from config (`golem-add-config-roc`) so the
  same component works in each environment.
- A failed body stream fails the call rather than returning a short body, so a
  truncated answer is never silently accepted.
