# Specification: Unit 16 — Developer Experience, Ergonomics and Boilerplate Elimination

- **Unit:** 16
- **File:** `docs/specs/16-developer-experience-and-ergonomics.md`
- **Status:** Complete
- **Phase:** Phase 4 — Ergonomics & Ecosystem
- **Focus:** Developer Experience (DX), Code Simplicity, Safe Oplog Bypass, Unified Tools

---

## 1. Goal & Motivation

When external developers and engineers from the Golem and WebAssembly communities inspect Roc agents running on Golem Cloud, the Roc code must be concise, expressive, idiomatic, and clearly demonstrate the benefits of functional state machines.

Currently, writing an agent in `roc-golem` incurs unnecessary ceremonial boilerplate:
1. **Repetitive State Codecs**: Every single agent is forced to implement identical `serializeState: |s| Json.encode(s)` and `deserializeState: |j| Ok(Json.decode_or(j, defaultState))` boilerplate.
2. **Double-Declaration of Tools**: Tools are declared twice — schemas are listed in `metadata.tools`, and execution logic is separated inside `handleToolCall: |state, toolCall| match toolCall.name { ... }`, with manual `toolCall.id` threading, manual `{ id, success, output }` wrapping, and error fallback boilerplate.
3. **Leaky Protocol Envelopes**: Handlers must return raw protocol representations like `Ok({ state: nextState, response: ... })` or manually thread `stateChanged: False` to benefit from Golem's $O(1)$ read-only bypass.
4. **Stateless Ephemeral Worker Friction**: Stateless workers (diagnostics, log scanners, routers) are forced to define a dummy state type `State : {}`, dummy initialization, and dummy serializer functions.

Unit 16 introduces an ergonomic, modern SDK tier directly in `platform/Golem.roc` without breaking changes, without runtime performance regressions, and without altering the low-level Rust C-ABI host contract.

---

## 2. Architecture & Design

### 2.1 Two-Tier SDK Design

```
+--------------------------------------------------------------------------+
|                        Tier 1: Canonical Agent API                       |
|               (Covers 90% of stateful agents & ephemeral workers)        |
|                                                                          |
|   Golem.agent({ name, initial, onMessage, tools })                       |
|   Golem.ephemeral({ name, onMessage, tools })                            |
+--------------------------------------------------------------------------+
                                     |
                                     v
+--------------------------------------------------------------------------+
|                     Tier 2: Composed Pipeline Builder                    |
|             (Covers multi-agent bundles, typed RPC, custom mounts)       |
|                                                                          |
|   Golem.spec({ name, initial })                                          |
|     |> Golem.withMount(httpMount)                                        |
|     |> Golem.withTool(tool)                                              |
|     |> Golem.onMethod(method, handler)                                   |
|     |> Golem.onQuery(method, queryHandler)                               |
|     |> Golem.build                                                       |
+--------------------------------------------------------------------------+
                                     |
                                     v
+--------------------------------------------------------------------------+
|                        Core Platform Abstractions                        |
|                                                                          |
|   - Unified Tool(state): combines schema (ToolDefinition) & handler      |
|   - Expressive Response Constructors: reply, readOnly, stay, fail        |
|   - Low-level backward-compatible engine: defineAgent, define, build     |
+--------------------------------------------------------------------------+
```

### 2.2 First-Class Unified Tools (`Tool(state)` & `StatelessTool`)

Instead of defining schema and implementation separately, a tool is a first-class record that bundles schema metadata with its execution function.

#### Type Definitions:
```roc
## Unified tool declaration with execution logic
Tool(state) : {
    definition : ToolDefinition,
    execute : state, Str => Result({ state : state, stateChanged : Bool, output : Str }, Str),
}

## Stateless tool declaration
StatelessTool : {
    definition : ToolDefinition,
    execute : Str => Result(Str, Str),
}
```

#### Smart Constructors:
- `Golem.tool({ name, description, params, execute })`:
  Creates a stateless tool where `execute: Str -> Result(Str, Str)`. Unhandled errors or invalid inputs automatically return `{ id: call.id, success: False, output: err }`.
- `Golem.statefulTool({ name, description, params, execute })`:
  Creates a stateful tool where `execute: state, Str -> Result({ state: state, stateChanged: Bool, output: Str }, Str)`.
- `Golem.param(name, description)`:
  Shorthand for `{ name, description, paramType: "string", required: True }`.
- `Golem.optionalParam(name, description)`:
  Shorthand for `{ name, description, paramType: "string", required: False }`.

#### Automatic Dispatch & Envelope Wrapping:
When a list of `Tool(state)` is supplied to an agent:
1. `metadata.tools` is synthesized automatically by extracting `tool.definition` from each tool.
2. Incoming tool invocations match on `call.name`:
   - If matched: call `tool.execute`, wrap the output into `{ id: call.id, success: True, output: res }`.
   - If execution fails: wrap into `{ id: call.id, success: False, output: errMsg }`.
   - If not matched: wrap into `{ id: call.id, success: False, output: "Tool not found: ${call.name}" }`.

### 2.3 Expressive Response Constructors

Instead of constructing manual record payloads, message handlers and tools use clear semantic constructors:

```roc
## 1. State changed: persists new state to Golem oplog
reply : state, Str -> Result({ state : state, stateChanged : Bool, response : Str }, Str)
reply = |nextState, responseText|
    Ok({ state: nextState, stateChanged: True, response: responseText })

## 2. Read-only: bypasses oplog and state serialization (O(1) in-memory response)
readOnly : state, Str -> Result({ state : state, stateChanged : Bool, response : Str }, Str)
readOnly = |currState, responseText|
    Ok({ state: currState, stateChanged: False, response: responseText })

## 3. No response: leaves state as-is, returns empty reply
stay : state -> Result({ state : state, stateChanged : Bool, response : Str }, Str)
stay = |currState|
    Ok({ state: currState, stateChanged: False, response: "" })

## 4. Failure: returns error string to host
fail : Str -> Result({ state : state, stateChanged : Bool, response : Str }, Str)
fail = |errMsg|
    Err(errMsg)
```

JSON variants are also provided for structured replies:
- `replyJson : state, a -> Result({ state : state, stateChanged : Bool, response : Str }, Str) where [a.Json.Encodable([])]`
- `readOnlyJson : state, a -> Result({ state : state, stateChanged : Bool, response : Str }, Str) where [a.Json.Encodable([])]`

### 2.4 The Canonical Single-Agent: `Golem.agent`

`Golem.agent` is the standard constructor for stateful agents:

```roc
agent : {
    name : Str,
    version ?? Str,
    description ?? Str,
    initial : state,
    onMessage : state, Str -> Result({ state : state, stateChanged : Bool, response : Str }, Str),
    tools ?? List(Tool(state)),
    serialize ?? (state -> Str),
    deserialize ?? (Str -> Result(state, Str)),
} -> Agent(state)
```

#### Behavior & Defaults:
- `version` defaults to `"1.0.0"`.
- `description` defaults to `""`.
- `tools` defaults to `[]`.
- `serialize` defaults to `Json.encode(state)`.
- `deserialize` defaults to `Ok(Json.decode_or(jsonStr, initial))`.
- Automatic read-only bypass: if `onMessage` returns `stateChanged: False`, the state is not re-serialized.

### 2.5 The Canonical Stateless Worker: `Golem.ephemeral`

`Golem.ephemeral` simplifies workers that maintain no durable state:

```roc
ephemeral : {
    name : Str,
    version ?? Str,
    description ?? Str,
    onMessage ?? (Str -> Result(Str, Str)),
    tools ?? List(StatelessTool),
} -> Agent({})
```

#### Behavior:
- Automatically uses `mode: Ephemeral`.
- Internal state type is `{}` with no serialization overhead.
- Handlers do not receive or return state records.

---

## 3. Detailed Before & After Walkthroughs

### 3.1 Counter Agent (`examples/counter/main.roc`)

#### Before (72 lines):
```roc
app [agent] { pf: platform "../../platform/main.roc" }

import pf.Json
import pf.Golem exposing [Agent, defineAgent]

State : { count : I64 }

agent : Agent(State)
agent = defineAgent({
    init: |_config| Ok({ count: 0.I64 }),
    handleMessage: |state, message|
        match message {
            "increment" => {
                nextCount = state.count + 1.I64
                Ok({ state: { count: nextCount }, response: "Counter incremented to ${Str.inspect(nextCount)}" })
            }
            "decrement" => {
                nextCount = state.count - 1.I64
                Ok({ state: { count: nextCount }, response: "Counter decremented to ${Str.inspect(nextCount)}" })
            }
            "get" => Ok({ state, response: "Current count is ${Str.inspect(state.count)}" })
            _ => Ok({ state, response: "Unknown command '${message}'" })
        },
    handleToolCall: |state, toolCall|
        match toolCall.name {
            "get_count" => Ok({
                state,
                result: { id: toolCall.id, success: True, output: Json.encode({ count: state.count }) },
            })
            _ => Ok({
                state,
                result: { id: toolCall.id, success: False, output: "Unknown tool ${toolCall.name}" },
            })
        },
    metadata: {
        name: "counter-agent",
        version: "1.0.0",
        description: "A durable Roc counter agent running on Golem Cloud",
        tools: [{ name: "get_count", description: "Returns current counter value", parameters: [] }],
    },
    serializeState: |state| Json.encode(state),
    deserializeState: |stateJson| Ok(Json.decode_or(stateJson, { count: 0.I64 })),
})
```

#### After (28 lines):
```roc
app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, agent, reply, readOnly, tool]

State : { count : I64 }

getCountTool = tool({
    name: "get_count",
    description: "Returns current counter value",
    params: [],
    execute: |state, _args| Ok({ count: state.count }),
})

agent : Agent(State)
agent = Golem.agent({
    name: "counter-agent",
    initial: { count: 0.I64 },
    tools: [getCountTool],
    onMessage: |state, message|
        match message {
            "increment" => reply({ count: state.count + 1.I64 }, "Counter incremented to ${Str.inspect(state.count + 1.I64)}"),
            "decrement" => reply({ count: state.count - 1.I64 }, "Counter decremented to ${Str.inspect(state.count - 1.I64)}"),
            "get" => readOnly(state, "Current count is ${Str.inspect(state.count)}"),
            _ => readOnly(state, "Unknown command '${message}'. Available: increment, decrement, get"),
        },
})
```

---

## 4. Implementation Steps

1. **Implement Core Types & Combinators in `platform/Golem.roc`**:
   - Define `Tool(state)` and `StatelessTool`.
   - Implement `tool` and `statefulTool` constructors.
   - Implement `param` and `optionalParam` helpers.
   - Implement `reply`, `readOnly`, `stay`, `fail`, `replyJson`, `readOnlyJson`.
2. **Implement `Golem.agent` and `Golem.ephemeral` in `platform/Golem.roc`**:
   - Build underlying `ErasedAgent` mapping automatically.
   - Wire default state serialization and deserialization via `Json.encode` and `Json.decode_or`.
   - Dispatch tool calls with automatic envelope construction and ID tracking.
   - Support `stateChanged: False` to trigger oplog bypass in host.
3. **Add Automated Unit Tests (`tests/dx_primitives.roc`)**:
   - Test response constructors (`reply`, `readOnly`, `stay`).
   - Test tool dispatch and envelope generation.
   - Test state serialization defaults.
4. **Refactor Examples to Modern DX**:
   - Refactor `examples/counter/main.roc` to `Golem.agent`.
   - Refactor `examples/ai_tool/main.roc` to unified tools.
   - Refactor `examples/streaming_agent/main.roc`.
   - Ensure `examples/canary_release` continues to use composed `spec` cleanly.
5. **Full Multi-Layer Verification**:
   - Layer 1: `cargo fmt && cargo clippy && roc fmt --check && roc check`
   - Layer 2: `cargo test` and `roc test`
   - Layer 3: `./tooling/build.sh all` and `wasm-tools validate`
   - Layer 4: `./tooling/test_consumer.sh` and `./tooling/test_runtime.sh` (live Golem server)
6. **Documentation & Release Synchronization**:
   - Update `README.md` with modern concise examples.
   - Re-package and upload release assets if needed.

---

## 5. Verification Criteria

- [x] `tests/dx_primitives.roc` passes all expect tests (`roc test tests/dx_primitives.roc`).
- [x] `examples/counter/main.roc` compiles and runs cleanly with 0 errors under new `Golem.agent` syntax.
- [x] `examples/ai_tool/main.roc` compiles and runs cleanly with unified tool declarations.
- [x] `./tooling/test_consumer.sh` succeeds with packaged distribution.
- [x] `./tooling/test_runtime.sh` passes 100% on live Golem server across all suites (counter multi-turn, read-only bypass, crash recovery, canary rollout, AI tool, streaming).
- [x] `cargo test --manifest-path host/Cargo.toml --features stub-guest --lib` passes all 53 host tests.
- [x] `README.md` reflects clean, low-boilerplate agent definitions.
