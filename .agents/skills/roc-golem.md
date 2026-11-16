---
name: roc-golem-platform
description: Comprehensive guide and best practices for developing Roc agents on Golem Cloud with the roc-golem platform, covering multi-agent bundling, ephemeral workers, zero-bottleneck serialization bypass, stateless tool handlers, raw binary I/O, opaque secret handles, and typed RPC.
---

# Roc Golem Platform Development Skill

This skill provides guidelines and patterns for building and deploying WebAssembly agents on Golem Cloud using the Roc programming language.

## Architecture & Core Concepts

- **Pre-compiled Host Architecture**: The Rust translation layer (`host/src/lib.rs`) is pre-compiled for target `wasm32-wasip2` into `targets/wasm32/libroc_golem_host.a` and packaged with WASI `libc.a`, implementing Golem Cloud 1.6.3 async Component Model interfaces (`golem:agent/guest@2.0.0`).
- **Universal Pass-Through Bridge**: Guest methods, tools, and message handling pass through JSON/string representations across the Roc↔Rust boundary.
- **Worker Instance Lifecycle**: In Golem Cloud, a worker instance is initialized for one specific `agent-type` and stays hot in memory.
- **Pure Functional Agents in Roc**: Agents are implemented as pure state machines with idiomatic Roc syntax, decoupled from low-level WebAssembly Component Model plumbing.

---

## Agent Patterns

### 1. Canonical Stateful Agent (`Golem.agent`)

The simplest, zero-boilerplate way to define a durable agent in Roc:

```roc
app [agent] {
    pf: platform "https://github.com/thesparq/roc-golem/releases/download/v1.4.1/R_vqlBFvierawpLae6SLv2hk7RGk3-POnPTztiG2PTY.tar.zst",
}

import pf.Golem exposing [Agent, Tool, agent, reply, readOnly, tool, param]
import pf.Types exposing [Result]

State : {
    count : I64,
}

getCountTool : Tool(State)
getCountTool = tool({
    name: "get_count",
    description: "Returns current counter value",
    params: [],
    execute: |state, _args| Ok("{\"count\":${Str.inspect(state.count)}}"),
})

agent : Agent(State)
agent = Golem.agent({
    name: "counter-agent",
    initial: { count: 0.I64 },
    tools: [getCountTool],
    onMessage: |state, message|
        match message {
            "increment" =>
                reply({ count: state.count + 1.I64 }, "Incremented"),
            "get" =>
                readOnly(state, "Current count: ${Str.inspect(state.count)}"),
            _ =>
                readOnly(state, "Unknown command"),
        },
})
```

- **`reply(nextState, response)`**: Commits the state mutation (`stateChanged: True`) and appends an entry to the durable oplog.
- **`readOnly(state, response)`**: Activates the zero-serialization bypass protocol ($O(1)$ read path, `stateChanged: False`) avoiding state re-serialization.

---

### 2. Effectful Tools & Host Effects in High-Level Agents (`v1.4.0+`)

High-level `Golem.agent`, `Golem.agentWith`, and `Golem.ephemeral` natively support host effects (`now`, `rpcSchedule`, `httpRequest`, `getSecretHandle`, etc.) inside `onMessage` and tools:

```roc
import pf.Golem exposing [
    Agent, Tool, agent, reply, readOnly, tool, effectfulTool,
    runEffect!, now, param,
]
import pf.Effect exposing [map]

# Pure and effectful tools freely mix in the same tool list:
statusTool : Tool(State)
statusTool = tool({
    name: "status",
    description: "Returns agent status",
    params: [],
    execute: |st, _| Ok("Active bookings: ${Str.inspect(st.count)}"),
})

# Effectful tool returning an Effect that automatically runs on the host:
bookTool : Tool(State)
bookTool = effectfulTool({
    name: "book",
    description: "Records a booking with host timestamp",
    params: [param("guest", "Guest name")],
    execute: |st, guest|
        map(now, |ts|
            Ok({
                state: { count: st.count + 1.U64, lastUpdated: ts },
                stateChanged: True,
                output: "Booked for ${guest} at ${Str.inspect(ts)}",
            })
        ),
})

# onMessage can directly execute host effects using runEffect!:
agent : Agent(State)
agent = Golem.agent({
    name: "booking-coordinator",
    initial: { count: 0.U64, lastUpdated: 0.U64 },
    tools: [statusTool, bookTool],
    onMessage: |state, msg|
        match msg {
            "sync_now" => {
                currentTime = runEffect!(now)
                reply({ count: state.count, lastUpdated: currentTime }, "Synced at ${Str.inspect(currentTime)}")
            }
            _ => readOnly(state, "ok"),
        },
})
    # Attach dynamic HTTP API mount for durable webhooks (v1.4.1+)
    |> withAgentHttpMount({
        pathPrefix: "/booking/{id}",
        corsOrigins: ["*"],
        webhookSuffix: "webhooks",
    })
```

> [!NOTE]
> **Dynamic HTTP Webhook Endpoints (`withAgentHttpMount`)**: Golem Cloud requires agents to have an active HTTP API deployment or `httpMount` before calling `createWebhook` on durable promises. Pipeline chaining `|> withAgentHttpMount({ pathPrefix, corsOrigins, webhookSuffix })` attaches this cleanly to `Agent(state)` containers without dropping down to low-level builder pipelines.
>
> **Direct Platform Types**: In `v1.4.1+`, `HttpMount`, `HttpRequest`, `SecretHeader`, `SecretHandle`, `ToolDefinition`, `ToolParameter`, `ToolCall`, and `ToolResult` are exported directly by `pf.Golem`, so there is no need to import `pf.Types`.

---

### 3. Stateful Durable Agent (Builder Pattern)

For advanced scenarios requiring custom serialization codecs, use `define`, `register`, and `registerReadOnly` with phantom-typed `Method(input, output)`:

```roc
import pf.Golem exposing [Agent, Method, define, register, registerReadOnly, method, build]

State : {
    count : I64,
}

incrementMethod : Method(Str, Str)
incrementMethod = method({
    name: "increment",
    description: "Increments the counter",
    promptHint: "Call to increment",
})

statusMethod : Method(Str, Str)
statusMethod = method({
    name: "get_status",
    description: "Returns current status",
    promptHint: "Query status",
})

agent : Agent(State)
agent =
    define(
        {
            name: "counter-agent",
            version: "1.2.0",
            description: "Durable Roc counter agent",
        },
        |_config| Ok({ count: 0 }),
        |state| "{\"count\":${Num.to_str(state.count)}}",
        |_json| Ok({ count: 0 }),
    )
        |> register(incrementMethod, handleIncrement)
        |> registerReadOnly(statusMethod, handleStatus)
        |> build

handleIncrement : State, Str -> Result({ state : State, out : Str }, Str)
handleIncrement = |state, _input|
    Ok({ state: { count: state.count + 1 }, out: "Incremented" })

handleStatus : State, Str -> Result(Str, Str)
handleStatus = |state, _input|
    Ok("Current count is ${Num.to_str(state.count)}")
```

### 2b. Effectful Method Handlers (`registerEffectfulTyped`)

When mutating methods need to invoke host effects (such as `rpcSchedule`, `http`, or `writeFileBytes`), use `registerEffectfulTyped`:

```roc
agent =
    define(...)
        |> registerEffectfulTyped(startCanary, handleStartEffectful, decodeStartInput, encodeStartOutput)
        |> build

handleStartEffectful : State, StartInput -> Effect.Effect(Result({ state : State, out : StartOutput }, Str))
handleStartEffectful = |state, input|
    Effect.map(
        rpcSchedule("worker-id", timeoutMethod, "payload", 300000),
        |_| Ok({ state: nextState, out: startOutput })
    )
```

---

### 4. Stateless Ephemeral Worker

For stateless tasks (log analysis, computation, formatting), use `defineEphemeral` and `registerEphemeral` (or `ephemeral`):

```roc
import pf.Golem exposing [Agent, Method, defineEphemeral, registerEphemeral, method, build]

processMethod : Method(Str, Str)
processMethod = method({
    name: "process",
    description: "Stateless processing",
    promptHint: "Execute pure processing task",
})

agent : Agent({})
agent =
    defineEphemeral({
        name: "worker-agent",
        version: "1.2.0",
        description: "Stateless worker",
    })
        |> registerEphemeral(processMethod, handleProcess)
        |> build

handleProcess : Str -> Result(Str, Str)
handleProcess = |input|
    Ok("Processed: ${input}")
```

- Handlers take `input -> Result(out, Str)` directly without threading dummy `State`.
- Emits `AgentMode::Ephemeral` in WIT discovery; the host avoids storing or caching state between calls.

---

### 5. Multi-Agent Bundling in One Wasm Component (`bundle2`–`bundle5`, `bundleWith`)

Combine multiple agents with completely different state types into a single WebAssembly component using distinct typed combinators (`bundle2`, `bundle3`, `bundle4`, `bundle5`) or pipeline chaining (`bundleWith`):

```roc
app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [bundle3, bundleWith]
import BookingCoordinator
import PricingWorker
import AuditWorker

# Bundling via distinct typed combinator:
agent = bundle3(
    BookingCoordinator.agent,
    PricingWorker.agent,
    AuditWorker.agent,
)

# Or via pipeline chaining:
agent =
    BookingCoordinator.agent
        |> bundleWith(PricingWorker.agent)
        |> bundleWith(AuditWorker.agent)
```

> [!IMPORTANT]
> **Compiler Monomorphization Safeguard**: Avoid placing multiple erased agents in a `List(Agent({}))` literal (such as `bundleAll([erase(a), erase(b), erase(c)])`) if they share similar closure signatures. Placing erased agents into a homogeneous list forces Roc's lambda-set solver into a recursive unification loop, which can cause severe build slowdowns or compiler timeouts. Always prefer `bundle2`–`bundle5` or `bundleWith`.

- Golem worker instances are created for a specific agent type (`golem agent new '<agent-type>()'`).
- Dispatch automatically routes initialization, methods, and tool calls to the designated agent by metadata name.

---

### 5. Tool Parameter Helpers

Tool definitions can declare strongly-typed parameters using platform helpers:

```roc
import pf.Golem exposing [param, numberParam, boolParam, typedParam]

# String parameter (required by default)
nameP = param("name", "Customer full name")

# Number parameter
ageP = numberParam("age", "Customer age in years")

# Boolean parameter
vipP = boolParam("is_vip", "Whether the customer is a VIP")

# Custom typed parameter (name, description, type, required)
filterP = typedParam("filter", "Optional query filter expression", "string", False)
```

---

### 6. Stateless vs. Stateful Tool Handlers

- **Stateful Tool Handlers**: Use `statefulTool({ name, description, params, execute })` when tool execution reads or mutates the agent's internal state.
- **Pure / Read-Only Tools**: Use `tool({ name, description, params, execute })` when tool execution reads state without mutation.
- **Stateless Tools**: Use `statelessTool({ name, description, params, execute })` for tools independent of agent state.

---

### 7. Metadata-Driven Dynamic HTTP Mounts (`withHttpMount`)

Any agent can declare dynamic HTTP mount routes in its metadata:
```roc
import pf.Golem exposing [withHttpMount]

agent : Agent(State)
agent =
    define(...)
        |> withHttpMount({
            pathPrefix: "/canary/{id}",
            corsOrigins: ["*"],
            webhookSuffix: "webhooks",
        })
        |> build
```
The host parses `path_prefix` into literal segments and `{var}` path variables (e.g. `/api/{tenant_id}/orders/{order_id}`), configures CORS allowed patterns, and sets the webhook endpoint suffix.

---

## Strongly-Typed HTTP Dispatch (`HttpRequest`)

Use `Golem.httpRequest` with the strongly-typed `HttpRequest` record:

```roc
import pf.Golem exposing [httpRequest]
import pf.Types exposing [HttpRequest]

req : HttpRequest
req = {
    url: "https://api.example.com/v1/notify",
    method: "POST",
    headers: [{ name: "Content-Type", value: "application/json" }],
    body: "{\"status\":\"ok\"}",
    secretHeaders: [],
}

Effect.map(httpRequest(req), |res| ...)
```

---

## Raw Binary Data Boundary & Lossy UTF-8 Remediation

Converting arbitrary binary data (PNG images, zip/tar archives, protobuf streams) via UTF-8 string decoders causes irreversible data corruption, as invalid byte sequences are replaced with `U+FFFD`.

`roc-golem` provides clean, raw `List(U8)` byte primitives across all storage and communication subsystems (backed by live `wasi:keyvalue/eventual@0.1.0`, `wasi:blobstore/blobstore`, and `wasi:http/outgoing-handler@0.2.3` on wasm32):

- **Filesystem**:
  - `readFileBytes : Str -> Effect(Result(List(U8), Str))`
  - `writeFileBytes : Str, List(U8) -> Effect(Result({}, Str))`
  - `appendFileBytes : Str, List(U8) -> Effect(Result({}, Str))`
- **Key-Value Store**:
  - `kvGetBytes : Str, Str -> Effect(Result(List(U8), Str))`
  - `kvSetBytes : Str, Str, List(U8) -> Effect(Result({}, Str))`
- **Blobstore**:
  - `blobGetBytes : Str, Str -> Effect(Result(List(U8), Str))`
  - `blobPutBytes : Str, Str, List(U8) -> Effect(Result({}, Str))`
- **HTTP**:
  - `httpRequestBytes : Str, List(U8) -> Effect(Result(List(U8), Str))`

---

## Opaque Secret Handles & Host-Mediated Security

To ensure API tokens, cryptographic keys, and database passwords never reside in guest linear memory or leak into durable state snapshots:

1. **Acquire an Opaque Handle**:
   ```roc
   apiKeyHandle : Result(SecretHandle, Str)
   apiKeyHandle = getSecretHandle("OPENAI_API_KEY")
   ```
   Secrets are represented as an opaque nominal type `SecretHandle := [SecretHandle(U32)]`. Use `wrapSecretHandle` and `unwrapSecretHandle` if raw handle translation is required.

2. **Host-Mediated Outbound Injection**:
   Specify headers referencing the secret handle in HTTP requests:
   ```roc
   secretHeader : SecretHeader
   secretHeader = {
       name: "Authorization",
       prefix: "Bearer ",
       handle: apiKeyHandle,
   }
   ```
   When performing `http` or `httpRequest`, the Golem host resolves the secret handle directly into WASI HTTP request headers without passing plaintext to the guest.

3. **Explicit Reveal (Opt-In)**:
   ```roc
   secretText : Result(Str, Str)
   secretText = revealSecret(handle)
   ```

---

## Durability, Timers & Golem 1.6 Host Primitives

- **Durable Sleep**: `sleep(millis)` uses Golem's durable timer subsystem; workers pause execution without consuming CPU, surviving restarts.
- **Monotonic Timestamp**: `now` yields current Unix timestamp in milliseconds.
- **Persistence Modes**: `setPersistence(mode)` configures Golem persistence (`PersistNothing`, `PersistRemoteSideEffects`, `Smart`).
- **Durable Promises & Webhooks**:
  - `createPromise : Effect(Result(U64, Str))` — allocates a durable promise handle.
  - `createWebhook : U64 -> Effect(Result(Str, Str))` — returns an external webhook URL associated with the promise.
  - `completePromise : U64, List(U8) -> Effect(Result(Bool, Str))` — completes promise with byte payload.
  - `getPromise : U64 -> Effect(Result(List(U8), Str))` — retrieves promise result or returns Err if still pending.
- **Idempotency & Oplog Controls**:
  - `generateIdempotencyKey : Effect(Result(Str, Str))` — acquires unique idempotency UUID.
  - `getOplogIndex : Effect(U64)` — retrieves current worker oplog index.
    *(⚠️ Note on Golem Cloud wasm32: Returns `0` due to a WIT export gap in Golem 1.6. Do not pass this value to `revertAgent`, as doing so rolls back the worker to its initial state.)*
  - `oplogCommit : U8 -> Effect({})` — commits oplog up to current index with replica quorum.
  - `getIdempotenceMode : Effect(Bool)` / `setIdempotenceMode : Bool -> Effect({})` — toggles idempotency mode.
  - `revertAgent : U64 -> Effect(Result({}, Str))` — rolls back agent state to specified oplog index.
- **WebSockets**:
  - `websocketConnect : Str -> Effect(Result(U32, Str))`
  - `websocketSend : U32, Str -> Effect(Result({}, Str))`
  - `websocketReceive : U32 -> Effect(Result(Str, Str))`
  - `websocketClose : U32 -> Effect(Result({}, Str))`
- **Durable RDBMS (PostgreSQL & MySQL)**:
  - `dbExecute : Str, Str -> Effect(Result(U64, Str))` — executes SQL statement returning modified rows count.
  - `dbQuery : Str, Str -> Effect(Result(Str, Str))` — executes SQL query returning JSON column/row data.
- **Distributed Tracing**:
  - `traceId : Effect(Str)` / `spanId : Effect(Str)` — OpenTelemetry trace and span identifiers.
  - `traceHeaders : Effect(Result(Str, Str))` — W3C traceparent / tracestate headers JSON.
- **Configuration Store**:
  - `configGet : Str -> Effect(Result(Str, Str))` — queries dynamic runtime configuration values via `wasi:config/store`.

---

## Standardized Numerical Parsing (`pf.Parse` & `pf.Golem`)

`roc-golem` provides comprehensive numeric parsing functions with ASCII digit extraction, overflow bounds checks, whitespace trimming, and optional sign (`+`, `-`) handling:

```roc
import pf.Golem exposing [
    parseU64, parseU32, parseU16, parseU8, parseU64Or,
    parseI64, parseI32, parseI16, parseI8, parseI64Or,
    parseF64, parseF64Or,
]

# Parsing unsigned numbers with explicit overflow detection:
countRes = parseU64("18446744073709551615") # Ok(18446744073709551615.U64)
overflowRes = parseU64("18446744073709551616") # Err(Overflow)
invalidRes = parseU64("not_a_number") # Err(InvalidNumStr)

# Signed integer parsing with sign preservation:
balanceRes = parseI64("-10500") # Ok(-10500.I64)

# Fallback helpers for configuration & query parameters:
port = parseU64Or(configStr, 8080.U64)
temperature = parseF64Or(tempStr, 0.7.F64)
```

---

## Build & Testing Verification

1. **Pre-compile Platform Host**:
   ```bash
   ./tooling/build.sh platform
   ```
   Produces `platform/targets/wasm32/libroc_golem_host.a`, `platform/targets/wasm32/libc.a`, and `platform/libhost.a`.

2. **Run Host Unit Tests**:
   ```bash
   cargo test --manifest-path host/Cargo.toml --features stub-guest --lib
   ```

3. **Package Platform Release**:
   ```bash
   ./tooling/build.sh package v1.4.1 thesparq/roc-golem
   ```

4. **Build & Deploy Agent**:
   ```bash
   golem build
   golem deploy --yes
   golem agent new '<agent-name>()'
   golem agent invoke '<agent-name>()' handle-message '{"message":"hello"}'
   ```
