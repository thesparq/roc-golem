---
name: golem-add-http-auth-roc
description: "Requiring authentication on an agent's HTTP mount in Roc. Use when the user asks to protect an endpoint, require a token, or keep an agent's HTTP surface private."
---

# HTTP Authentication

> **Status: pending.** The mount has no authentication surface. Nothing below
> compiles today.

## What exists

A mount publishes paths that the runtime serves **without invoking the agent**, so
there is nowhere for per-request auth to run:

```roc
dashboardMount = Mount({
	pathPrefix: ["canary", "{id}"],
	staticBindings: [],
	filesystemBindings: [
		Exact({ publicPath: ["dashboard.html"], filePath: "/dashboard.html" }),
	],
	fileResponseHeaders: [],
	corsAllowedPatterns: [],
})
```

Exposure is controlled by *where* the deployment lives, not by the app: keep the
deployment off the public internet, or put the whole deployment behind a gateway
that authenticates.

## The dynamic alternative

If a check must run per request, the request has to reach the app — which means a
**dynamic router** with `handleRequest!` rather than a filesystem mount. The app can
then inspect a header itself:

```roc
handleRequest! : State, HttpRequest => { state : State, response : HttpResponse }
handleRequest! = |state, request| {
	authorized = match Str.split_first(request.headersJson, "\"authorization\"") {
		Err(_) => False
		Ok(_) => True
	}
	if authorized {
		{ state, response: { status: 200, headers: [], body: "ok" } }
	} else {
		{ state, response: { status: 401, headers: [], body: "unauthorized" } }
	}
}
```

That is a header check, not real authentication: verifying a token means comparing a
secret, and the platform's secret story is a value in app space
(`golem-add-secret-roc`).

## The gap

Two pieces are missing for proper auth:

```roc
# <mount authentication — not yet available>
# - declaring that a mount requires auth, with the scheme the runtime enforces
#   before it serves a file
# - a principal in the request, so the app can authorize rather than guess
```

An agent invocation already carries a principal (`getPrincipal!`), but an HTTP mount
does not pass one through. Until both exist, treat a mount as public and put the
gateway in front.
