# Feature Test Checklist

Run these against a **local Golem server, version 1.6.3+**, or via comprehensive mock unit tests in `host/src/lib.rs` with components built from real Roc code.

"How" is written in plain words. Exact execution commands are implemented in `tooling/test_runtime.sh`, `tooling/test_consumer.sh`, and `cargo test --manifest-path host/Cargo.toml --features stub-guest --lib`.
All tests carry an explicit verification source tag:
- `[Live Golem Server (1.6.3)]`: Executed against live Golem Cloud server instance on port 9881.
- `[Host Unit Test (Mock)]`: Executed in host unit tests with mock WASI/Golem interfaces.
- `[Build & Packaging Tooling]`: Executed via `tooling/build.sh` and `tooling/test_consumer.sh`.

---

## Build and deploy

| # | Test | How | Expected | Status |
|---|------|-----|----------|:---:|
| 1 | Build one example | `./tooling/build.sh counter` | a component appears in `build/` and `wasm-tools validate` accepts it (KI-01) | **PASSED** (2026-11-07) [Build & Packaging Tooling] |
| 2 | Examples differ | build all four examples; compare `sha256sum build/*_agent.wasm` | four different hashes (KI-01) | **PASSED** (2026-11-07) [Build & Packaging Tooling] |
| 3 | Deploy | add the `counter` component to a running local Golem server | the component is accepted and its agent type is listed | **PASSED** (2026-11-07) [Live Golem Server (1.6.3)] |
| 4 | Agent definitions | ask the server for the agent types of `canary_release` | two agent types: the `canary-release` coordinator (durable) and the diagnostics worker (ephemeral) | **PASSED** (2026-11-08) [Live Golem Server (1.6.3)] |

---

## Counter agent (durable state)

| # | Test | How | Expected | Status |
|---|------|-----|----------|:---:|
| 5 | First call | on a new worker, call `increment` once | the reply reports the new count | **PASSED** (2026-11-07) [Live Golem Server (1.6.3)] |
| 6 | State across calls | call `increment` three times, then `get` | 3 (KI-03) | **PASSED** (2026-11-07) [Live Golem Server (1.6.3)] |
| 7 | State across restart | restart the worker, then `get` | still 3 (KI-03) | **PASSED** (2026-11-07) [Live Golem Server (1.6.3)] |
| 8 | Awkward state text | use an agent whose state holds quotes, backslashes, newlines, non-ASCII text and an empty string; save, then load | exactly the same text comes back (KI-03, KI-09) | **PASSED** (2026-11-07) [Live Golem Server (1.6.3)] |

---

## Read-only and ephemeral agents

| # | Test | How | Expected | Status |
|---|------|-----|----------|:---:|
| 9 | Read-only method | call a read-only method (`get`, `get_status`) several times | same answer each time; no state is saved | **PASSED** (2026-11-08) [Live Golem Server (1.6.3)] |
| 10 | Ephemeral worker | call the diagnostics worker twice | each call is independent; nothing is kept between calls | **PASSED** (2026-11-08) [Live Golem Server (1.6.3)] |

---

## Typed methods and tools

| # | Test | How | Expected | Status |
|---|------|-----|----------|:---:|
| 11 | Typed method over HTTP | call a registered typed method through its HTTP endpoint | a JSON reply that matches the method's output type | **PASSED** (2026-11-08) [Live Golem Server (1.6.3)] |
| 12 | Tool call | call the tool `analyze_logs` with text containing one `PANIC` and one `OOM` | `{"faults": 2}` | **PASSED** (2026-11-08) [Live Golem Server (1.6.3)] |
| 13 | Unknown method | call a method that does not exist | a clear error that names the method | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 14 | Handler error | call a handler that returns `Err("boom")` | the caller sees "boom", not a different error (KI-10) | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |

---

## Agent-to-agent calls

| # | Test | How | Expected | Status |
|---|------|-----|----------|:---:|
| 15 | Call and wait | the coordinator calls the diagnostics worker with `rpc` | the worker's reply comes back | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 16 | Fire and forget | call another agent with `rpcFireAndForget` | the call is delivered and no reply is awaited | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 17 | Scheduled call | `rpcSchedule` with a 5 second delay | nothing happens at once; the call happens once, about 5 seconds later (KI-04) | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 18 | Address one instance | call an agent that has a constructor argument | the right instance answers (KI-13) | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |

---

## Platform effects

| # | Test | How | Expected | Status |
|---|------|-----|----------|:---:|
| 19 | HTTP request | `http` GET to a public URL | status code and body | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 20 | HTTP streaming | `httpStreamOpen`, read chunks, close | chunks arrive in order; close succeeds | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 21 | Key-value | `kvSetBytes` then `kvGetBytes` with non-UTF-8 bytes | identical bytes | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 22 | Blobstore | `blobPutBytes` then `blobGetBytes` | identical bytes | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 23 | WebSocket | connect, send, receive, close against an echo server | the sent text comes back; close succeeds | **PASSED** (2026-11-08) [Live Golem Server (1.6.3)] |
| 24 | Sleep | `sleep` for 2 seconds | execution resumes after about 2 seconds | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 25 | Secret present | provide a secret, take a handle, make an HTTP call with the secret header | the header is injected; the value never appears in agent state (KI-05) | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 26 | Secret missing | take a handle for a name that does not exist | a clear error that names the secret | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 27 | Promise and webhook | create a promise and a webhook, call the webhook URL with a payload, then `getPromise` | the payload comes back | **PASSED** (2026-11-08) [Live Golem Server (1.6.3)] |
| 28 | Database | `dbExecuteParams` then `dbQueryParams` on Postgres | the inserted row comes back | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 29 | Config | `configGet` for a key that exists and one that does not | the value; a clear error | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 30 | Tracing | `traceId`, `spanId`, `traceHeaders` | non-empty ids; a valid W3C `traceparent` | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 31 | Idempotency key | `generateIdempotencyKey` twice | two different valid ids | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 32 | Oplog and revert | note `getOplogIndex`, change state, `revertAgent` to that index | state is back to what it was | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 33 | Files | `writeFile` then `readFile` | the same text, or a clear "unsupported" error (KI-06) | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 34 | Logging | log at each level | the lines appear in the worker's log (KI-07) | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 35 | Database statement caching | execute parameterized SQL queries repeatedly | statement cache hits succeed without re-preparing | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |

---

## Canary example end to end

| # | Test | How | Expected | Status |
|---|------|-----|----------|:---:|
| 36 | Healthy canary | `start_canary` with clean logs | `canary_started` at the requested weight, with a real approval webhook URL (KI-14) | **PASSED** (2026-11-08) [Live Golem Server (1.6.3)] |
| 37 | Unhealthy canary | `start_canary` with logs containing `PANIC` | `aborted_unhealthy`, weight 0 | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 38 | Approve | `approve_gate` with approval | the canary is promoted; weight 100 | **PASSED** (2026-11-08) [Live Golem Server (1.6.3)] |
| 39 | Reject | `approve_gate` with rejection | weight back to 0 | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 40 | Emergency rollback | `emergency_rollback` | weight 0 | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |
| 41 | Dashboard | open the agent's `/dashboard.html` | the page shows the service, versions and events (KI-14) | **PASSED** (2026-11-08) [Live Golem Server (1.6.3)] |
| 42 | Status | `get_status` | matches the events so far | **PASSED** (2026-11-08) [Host Unit Test (Mock)] |

---

## Release & Host Verification

| # | Test | How | Expected | Status |
|---|------|-----|----------|:---:|
| 43 | Package | `./tooling/build.sh package v1.0.0 thesparq/roc-golem` | three archives, `checksums.txt` and `release_notes.md` in `dist/` | **PASSED** (2026-11-10) [Build & Packaging Tooling] |
| 44 | Consume a release | a new Roc app using the README header | `roc check` and the build succeed | **PASSED** (2026-11-10) [test_consumer.sh] |
| 45 | Hash matches | recompute the BLAKE3 base64url hash of the published archive | equals the hash in the README URL | **PASSED** (2026-11-10) [Build & Packaging Tooling] |
| 46 | Native library unit tests | `cargo test --manifest-path host/Cargo.toml --all-targets --features stub-guest` | all 53 unit tests pass on host | **PASSED** (2026-11-10) [Host Unit Test (Mock)] |
