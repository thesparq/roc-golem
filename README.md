# Roc Platform for Golem Cloud

A durable, agent-native WebAssembly platform for writing [Golem Cloud](https://golem.cloud) stateful agents, AI tools, and streaming workflows in [Roc](https://www.roc-lang.org) with a pre-compiled **Rust Host** translation layer and the **Wasm Component Model**.

---

## 🌟 Highlights

- **Pure Functional Agents in Roc**: Write durable agents as pure state machines with idiomatic Roc syntax without needing Rust or WIT knowledge.
- **Universal Pass-Through Adapter**: The platform translation layer acts as a generic JSON/String bridge, letting your Roc application control strongly typed serialization and domain logic.
- **Pre-Compiled Host Architecture**: The Rust host is pre-compiled into a static library (`libhost.a` / `host.wasm`), allowing app developers to build and componentize directly.
- **WebSocket & Streaming Support**: Built-in Golem WebSocket Client API (`websocketConnect`, `websocketSend`, `websocketReceive`, `websocketClose`) for bi-directional live streaming.
- **Durable Scheduling & Timers**: Durable execution-aware sleep (`Golem.sleep`) and monotonic clock access (`Golem.now`).
- **Worker-to-Worker RPC & HTTP**: Call other Golem workers or external APIs directly from Roc effect functions.

---

## 📁 Repository Structure

```
roc-golem/
├── wit/
│   └── world.wit           # Golem Component Model WIT interface definitions
├── host/
│   ├── Cargo.toml          # Rust host crate configuration (wit-bindgen, serde)
│   └── src/
│       ├── lib.rs          # Universal WIT export/import & C-ABI translation layer
│       ├── roc_std.rs      # Wasm32 Roc ABI types (RocStr, RocList, Allocators)
│       └── guest_bridge.rs # Guest dispatch & fallback bridge
├── platform/
│   ├── main.roc            # Roc Platform definition & host entry points
│   ├── Golem.roc           # Idiomatic Roc Golem SDK (Agent, WebSocket, Timers, RPC, HTTP)
│   ├── Types.roc           # Core data types (Metadata, ToolDefinition, ToolCall)
│   └── Effect.roc          # Low-level host effect declarations
├── examples/
│   ├── counter/
│   │   └── main.roc        # Stateful counter agent example
│   ├── ai_tool/
│   │   └── main.roc        # AI agent with tool declarations example
│   └── streaming_agent/
│       └── main.roc        # WebSocket streaming agent example
├── tooling/
│   └── build.sh            # Platform pre-compilation & app componentization script
└── golem.yaml              # Golem Cloud application manifest
```

---

## 🚀 Writing Agents in Roc

### 1. Streaming Agent with WebSockets (`examples/streaming_agent/main.roc`)

```roc
app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [
    Agent,
    defineAgent,
    websocketConnect,
    websocketSend,
    websocketReceive,
    websocketClose,
]
import pf.Types exposing [ToolCall, ToolResult]

State : {
    connected : Bool,
    handle : U32,
    messagesSent : List Str,
    messagesReceived : List Str,
}

agent : Agent State
agent = defineAgent {
    init: |_config|
        Ok {
            connected: Bool.false,
            handle: 0u32,
            messagesSent: [],
            messagesReceived: [],
        },

    handleMessage: |state, message|
        when message is
            "connect" ->
                Ok {
                    state: { state & connected: Bool.true, handle: 1u32 },
                    response: "WebSocket connected (handle=1)",
                }

            "status" ->
                statusStr = if state.connected then "connected" else "disconnected"
                Ok {
                    state,
                    response: "WebSocket is ${statusStr}. Sent ${Num.toStr (List.len state.messagesSent)}, Received ${Num.toStr (List.len state.messagesReceived)}",
                }

            _ ->
                if state.connected then
                    nextSent = List.append state.messagesSent message
                    Ok {
                        state: { state & messagesSent: nextSent },
                        response: "Queued message for WebSocket: ${message}",
                    }
                else
                    Ok {
                        state,
                        response: "Not connected. Send 'connect' first.",
                    },

    handleToolCall: |state, toolCall|
        when toolCall.name is
            "stream_message" ->
                Ok {
                    state,
                    result: {
                        id: toolCall.id,
                        success: Bool.true,
                        output: "{\"status\": \"stream_active\", \"sent_count\": ${Num.toStr (List.len state.messagesSent)}}",
                    },
                }

            _ ->
                Ok {
                    state,
                    result: {
                        id: toolCall.id,
                        success: Bool.false,
                        output: "{\"error\": \"Unknown tool ${toolCall.name}\"}",
                    },
                },

    metadata: {
        name: "streaming-agent",
        version: "1.0.0",
        description: "Durable WebSocket streaming agent running on Golem Cloud",
        tools: [
            {
                name: "stream_message",
                description: "Streams message via WebSocket",
                parameters: [
                    {
                        name: "content",
                        description: "Message to stream",
                        paramType: "string",
                        required: Bool.true,
                    },
                ],
            },
        ],
    },

    serializeState: |state|
        connStr = if state.connected then "true" else "false"
        "{\"connected\":${connStr},\"handle\":${Num.toStr state.handle},\"sent\":${Num.toStr (List.len state.messagesSent)}}",

    deserializeState: |stateJson|
        connected =
            when Str.splitFirst stateJson "\"connected\":true" is
                Ok _ -> Bool.true
                Err _ -> Bool.false
        handle =
            when Str.splitFirst stateJson "\"handle\":" is
                Ok { after } ->
                    when Str.splitFirst after "," is
                        Ok { before } -> Str.toU32 (Str.trim before) |> Result.withDefault 0u32
                        Err _ -> 0u32

                Err _ -> 0u32
        Ok {
            connected,
            handle,
            messagesSent: [],
            messagesReceived: [],
        },
}
```

---

## 🔨 Build & Deployment Workflow

### 1. Pre-compile Platform Host (Once)
```bash
./tooling/build.sh platform
```
Produces `build/libhost.a` and `build/host.wasm`.

### 2. Build Any Roc Agent
```bash
# Build streaming agent
./tooling/build.sh streaming_agent

# Or build from path
./tooling/build.sh app examples/counter/main.roc
```

### 3. Deploy to Golem Cloud
```bash
golem component add --component-name streaming-agent build/streaming_agent_agent.wasm
golem worker add --component-name streaming-agent --worker-name stream-worker-1
golem worker invoke-and-await --component-name streaming-agent --worker-name stream-worker-1 \
  --function "golem:agent-platform/agent-api.{handle-message}" \
  --args '["connect"]'
```
