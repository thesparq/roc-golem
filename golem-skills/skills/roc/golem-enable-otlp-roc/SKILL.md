---
name: golem-enable-otlp-roc
description: "Turning on OpenTelemetry export for an agent deployment. Use when the user asks to trace an agent, collect metrics, send telemetry to an OTLP endpoint, or debug performance with Jaeger."
---

# OpenTelemetry Export

> **Status: platform-level.** This is not application code. A Roc agent is traced by
> the runtime, so there is nothing to add to the agent itself.

## How tracing works here

Golem records every invocation in the agent's oplog and exports traces through its
own OTLP configuration. The agent's logs go through `wasi:logging` under the
`roc-agent` target, which is what makes them attributable in a trace.

So from the app side there is exactly one thing to do: log usefully.

```roc
import pf.Golem exposing [logInfo!, logWarn!]

logInfo!("start_canary ${version} weight=${U64.to_str(weight)}")
```

## Turning the export on

It is deployment configuration — an OTLP exporter plugin and endpoint on the Golem
side, not a manifest key in your app. See Golem's operations documentation for the
exporter plugin; this platform's own `docs/[REDACTED]-readiness.md` notes that there
is no platform-side metrics surface, because there is nothing to add.

## What you can see

- Invocation start/finish per method, with the invocation identity.
- Host calls the agent made — HTTP, database, RPC — since each is a recorded effect.
- Log lines the agent emitted, under the `roc-agent` target.

## Placeholder

There is no in-app tracing API:

```roc
# <manual span API — not available, and not planned>
# The runtime already spans every invocation; an app-side span would duplicate it.
```

If you need finer detail, put it in log lines with identifiers that let you
correlate them.
