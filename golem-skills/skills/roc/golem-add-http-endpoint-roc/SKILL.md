---
name: golem-add-http-endpoint-roc
description: "Publishing an HTTP surface from a Roc Golem agent. Use when the user asks to serve files, add a web endpoint, expose a dashboard, or make an agent reachable at a URL."
---

# HTTP Endpoints

> **Status: implemented.**

An agent publishes HTTP through a *mount*, declared in its metadata. There are
three shapes; pick by who computes the response.

| Shape | Serves | Served by |
| --- | --- | --- |
| filesystem mount | files in the agent's own durable filesystem | the runtime |
| static router | files uploaded at deploy time | the runtime |
| dynamic router | anything the app computes | your `handleRequest!` |

## Filesystem mount

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

agent =
	define(cfg, |service| { .. })
		|> withMount(dashboardMount)
		|> build
```

`{id}` captures the agent's constructor parameter, so the URL selects the
instance. A matching `GET` never invokes the agent — it reads the filesystem,
which means a file the agent *writes* is served too:

```roc
writeFile!("/dashboard.html", "<!doctype html>...")
```

Other bindings: `Subtree({ publicPrefix: ["assets"], filesystemRoot: "/assets" })`
serves everything under a prefix.

## Dynamic router

```roc
import pf.Golem exposing [HttpResponse]

handleRequest! : State, HttpRequest => { state : State, response : HttpResponse }
handleRequest! = |state, request| {
	{
		state,
		response: {
			status: 200,
			headers: [{ name: "content-type", value: "application/json" }],
			body: "{\"path\": ${Json.to_str(request.path)}}",
		},
	}
}
```

Declare it with `agentKind: HttpRouter` and `httpHandler: HandlesRequests`.

## The manifest must list the agent type

```yaml
localServer:
  customRequestPort: 9882
httpApi:
  deployments:
    local:
      - subdomain: roc-golem
        agents:
          my-agent: {}
```

A mount is only served for an agent type in an HTTP API deployment. `subdomain`
expands to `<subdomain>.localhost:<customRequestPort>`.

```shell
curl -H 'Host: roc-golem.localhost:9882' http://127.0.0.1:9882/canary/prod/dashboard.html
```
