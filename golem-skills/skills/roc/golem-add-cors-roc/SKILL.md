---
name: golem-add-cors-roc
description: "Configuring CORS for a Roc Golem HTTP mount so browsers can call it. Use when the user asks about cross-origin requests, CORS headers, or a browser refusing a request to an agent."
---

# CORS

> **Status: implemented.**

CORS is declared on the mount. The runtime answers the preflight and adds the
headers; the app does not see an `OPTIONS` request.

## Declare the allowed origins

```roc
dashboardMount = Mount({
	pathPrefix: ["canary", "{id}"],
	staticBindings: [],
	filesystemBindings: [
		Exact({ publicPath: ["dashboard.html"], filePath: "/dashboard.html" }),
	],
	fileResponseHeaders: [],
	corsAllowedPatterns: ["*"],
})
```

```roc
		|> withMount(dashboardMount)
```

`corsAllowedPatterns` is a list of patterns matched against the request's
`Origin`. `["*"]` allows any origin; a specific entry narrows it:

```roc
	corsAllowedPatterns: ["https://console.example.test", "https://*.example.test"],
```

An empty list (`[]`) allows nothing — which is what a mount nobody calls from a
browser should declare.

## Notes

- This covers a mount. A **dynamic router** that computes its own responses
  should also set the headers it needs in `handleRequest!` — the mount-level
  setting is about the runtime-served paths.
- Golem rejects response headers it owns (`content-length`, `cache-control`,
  `etag`, …) in `fileResponseHeaders`, so keep those out.
