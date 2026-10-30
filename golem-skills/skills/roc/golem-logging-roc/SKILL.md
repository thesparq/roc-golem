---
name: golem-logging-roc
description: "Emitting log lines from a Roc Golem agent at each level. Use when the user asks to add logging, diagnostics, or tracing to an agent."
---

# Logging

> **Status: implemented.**

Logging goes through `wasi:logging` under the fixed target `roc-agent`, so lines
are attributed to the platform rather than to your app.

## The effects

```roc
import pf.Golem exposing [logTrace!, logDebug!, logInfo!, logWarn!, logError!]

logInfo!("canary ${version} at ${U64.to_str(weight)}%")
logWarn!("upstream answered ${U32.to_str(status)}")
logError!("cannot reach ${url}: ${err}")
```

Each takes a `Str` and returns `{}`. They are effects (`!`), so they belong in
effectful handlers — a read-only method cannot log.

## Reading them back

```shell
golem agent invoke 'my-agent("id")' get_status '{}'   # invocation records
golem agent logs 'my-agent("id")'                     # the agent's log
```

Everything an agent does is also in its invocation log (the oplog), which is what
a replay reads. A log line is for an operator; the oplog is what makes the agent
durable. Do not use logging to record state you need back.

## Notes

- Build a line with interpolation rather than concatenation:
  `"count=${I64.to_str(n)}"`.
- `U64.to_str`, `I64.to_str`, `U32.to_str` and `U16.to_str` are the conversions in
  scope; there is no general `to_str`.
