# Specification 20: Effectful High-Level Agent & Tool API

## Status
- **Date**: 2026-11-16
- **Status**: Accepted & In Implementation
- **Target Release**: `v1.4.0`

## 1. Context & Problem Statement
During real-world development of multi-agent systems on `roc-golem`, agent authors encountered a friction point when using the high-level Developer Experience (DX) API (`Golem.agent`, `Golem.agentWith`, `Golem.ephemeral`, and `tool` / `statefulTool`):

Host effects in `roc-golem` (`now`, `rpcSchedule`, `httpRequest`, `createPromise`, `getSecretHandle`, etc.) produce values of type `Effect.Effect(a)`. Executing an effect requires invoking `Effect.run!`, which gives the calling closure an effectful fat arrow (`=>`).

However, in `platform/Golem.roc`:
1. `Tool(state)` declared its `execute` function as `state, Str -> Result({ state, stateChanged, output }, Str)` (pure thin arrow).
2. `StatelessTool` declared its `execute` function as `Str -> Result(Str, Str)` (pure thin arrow).
3. `agentWith` and `agent` declared `onMessage` as `state, Str -> Result({ state, stateChanged, response }, Str)` (pure thin arrow).
4. `ephemeral` and `simpleEphemeral` declared `onMessage` as `Str -> Result(Str, Str)` (pure thin arrow).

Because an effectful closure (`=>`) cannot be coerced to a pure function (`->`), any attempt to call host effects inside a unified tool or `onMessage` failed with a compiler arrow mismatch error, forcing developers to abandon the high-level API and revert to the verbose low-level builder pattern (`define |> registerEffectful |> build`).

## 2. Root Cause & Architectural Insight
The underlying platform and runtime are already completely effectful:
- `ErasedAgent` declares `invokeMethod`, `handleToolCall`, and `handleMessage` using fat arrows (`=>`).
- `platform/main.roc` hosts effectful handlers `main_handle_message_for_host!` and `main_handle_tool_call_for_host!`.
- The Rust host executes guest calls via async WebAssembly component exports.

The thin arrow restriction was solely an artificial constraint in the high-level API type annotations. In Roc's type system, pure functions (`->`) are subtypes of effectful closures (`=>`), meaning upgrading these signatures to `=>` provides **100% backward compatibility** for all existing pure agents while enabling full effectful execution.

## 3. Detailed Design

### 3.1 Type Signature Updates in `platform/Golem.roc`
1. `Tool(state)`:
   ```roc
   Tool(state) : {
       definition : ToolDefinition,
       execute : state, Str => Result({ state : state, stateChanged : Bool, output : Str }, Str),
   }
   ```
2. `StatelessTool`:
   ```roc
   StatelessTool : {
       definition : ToolDefinition,
       execute : Str => Result(Str, Str),
   }
   ```
3. `dispatchTool`:
   ```roc
   dispatchTool :
       List(Tool(state)), state, ToolCall, (state -> Str) => Result({ nextStateJson : Str, stateChanged : Bool, result : ToolResult }, Str)
   ```
4. `dispatchStatelessTool`:
   ```roc
   dispatchStatelessTool :
       List(StatelessTool), ToolCall => Result({ nextStateJson : Str, stateChanged : Bool, result : ToolResult }, Str)
   ```
5. `dispatchMessage`:
   ```roc
   dispatchMessage :
       (state, Str => Result({ state : state, stateChanged : Bool, response : Str }, Str)), state, Str, (state -> Str) => Result({ nextStateJson : Str, stateChanged : Bool, response : Str }, Str)
   ```
6. `agentWith`, `agent`, `simpleAgent`, `ephemeral`, `simpleEphemeral`:
   `onMessage` signatures upgraded from `->` to `=>`.

### 3.2 New Tool Constructors
- `effectfulTool`:
  ```roc
  effectfulTool :
      {
          name : Str,
          description : Str,
          params : List(ToolParameter),
          execute : state, Str -> Effect.Effect(Result({ state : state, stateChanged : Bool, output : Str }, Str)),
      } -> Tool(state)
  ```
- `effectfulToolJson`:
  ```roc
  effectfulToolJson :
      {
          name : Str,
          description : Str,
          params : List(ToolParameter),
          execute : state, Str -> Effect.Effect(Result({ state : state, stateChanged : Bool, output : a }, Str)),
      } -> Tool(state)
          where [a.Json.Encodable([])]
  ```
- `effectfulStatelessTool`:
  ```roc
  effectfulStatelessTool :
      {
          name : Str,
          description : Str,
          params : List(ToolParameter),
          execute : Str -> Effect.Effect(Result(Str, Str)),
      } -> StatelessTool
  ```
- `effectfulStatelessToolJson`:
  ```roc
  effectfulStatelessToolJson :
      {
          name : Str,
          description : Str,
          params : List(ToolParameter),
          execute : Str -> Effect.Effect(Result(a, Str)),
      } -> StatelessTool
          where [a.Json.Encodable([])]
  ```

### 3.3 Imperative Effect Execution: `runEffect!`
Export `runEffect!` and `Effect` directly from `pf.Golem`:
```roc
Effect(a) : Effect.Effect(a)

runEffect! : Effect.Effect(a) => a
runEffect! = |eff| Effect.run!(eff)
```

This allows synchronous, imperative effect execution inside any `=>` closure:
```roc
agent = Golem.agent({
    name: "booking-coordinator",
    initial: { activeBookings: 0.U64 },
    tools: [scheduleTool],
    onMessage: |state, message|
        match message {
            "schedule" => {
                timestamp = Golem.runEffect!(Golem.now)
                reply({ state & activeBookings: state.activeBookings + 1 }, "Scheduled at ${Str.inspect(timestamp)}")
            }
            "ping" =>
                readOnly(state, "pong")
        },
})
```

## 4. Verification Plan
- Unit tests in `tests/effectful_agent.roc` testing `runEffect!` in `onMessage`, `effectfulTool`, `effectfulStatelessTool`, and pure/effectful tool mixing.
- Full regression verification across all 6 existing Roc test suites and 56 host tests.
- Release packaging and consumer verification via `./tooling/test_consumer.sh`.
