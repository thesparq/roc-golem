---
name: golem-http-params-roc
description: "Reading an incoming HTTP request's method, path, query, headers and body in a Roc Golem router. Use when the user asks to handle a request, route on a path, read a query parameter, or answer with computed headers."
---

# HTTP Request Parameters

> **Status: implemented.**

A dynamic router receives the request as a record and answers with a record.

## The request

```roc
HttpRequest : {
	method : Str,
	scheme : Str,
	authority : Str,
	path : Str,
	query : Str,
	headersJson : Str,
	body : Str,
}
```

```roc
handleRequest! : State, HttpRequest => { state : State, response : HttpResponse }
handleRequest! = |state, request| {
	answer = if request.method == "GET" {
		"got ${request.path}"
	} else {
		"got ${request.method} with ${U64.to_str(Str.count_utf8_bytes(request.body))} bytes"
	}
	{ state, response: { status: 200, headers: [], body: answer } }
}
```

- `query` is the raw query string (`"a=1&b=2"`), empty when the URL had none.
- `headersJson` is the header list **as JSON text**
  (`[{"name":...,"value":...}]`, in request order), because a header may occur
  more than once and order can matter. Parse it with the built-in `Json` codec.
- `body` is the whole body as text — it arrives buffered, so there is no
  incremental read. The platform parses with the built-in `Json` codec, so a body
  that is itself JSON arrives whole, quotes and backslashes included.

## The response

```roc
HttpResponse : { status : U16, headers : List(HttpHeader), body : Str }
```

Any status, any headers, and a body of any size — the host writes it as bounded
chunks.

## Reading a field out of headersJson

```roc
headerValue = |request, wanted|
	match Str.split_first(request.headersJson, "\"${wanted}\"") {
		Err(_) => ""
		Ok(found) =>
			match Str.split_first(found.after, "\"value\":\"") {
				Err(_) => ""
				Ok(v) =>
					match Str.split_first(v.after, "\"") {
						Err(_) => ""
						Ok(pair) => pair.before
					}
			}
	}
```

Roc has no `Str.split` in this nightly, so walking with `Str.split_first` is the
idiom.
