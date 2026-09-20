app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, defineAgent]

# Agent State
State : {
    connected : Bool,
    handle : U32,
    sent : U64,
    received : U64,
}

agent : Agent State
agent = defineAgent {
    init: |_config|
        Ok {
            connected: Bool.false,
            handle: 0u32,
            sent: 0u64,
            received: 0u64,
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
                    response: "WebSocket is ${statusStr}. Sent ${Num.to_str state.sent}, Received ${Num.to_str state.received}",
                }

            _ ->
                if state.connected then
                    Ok {
                        state: { state & sent: state.sent + 1u64 },
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
                        output: "{\"status\": \"stream_active\", \"sent_count\": ${Num.to_str state.sent}}",
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
        "{\"connected\":${connStr},\"handle\":${Num.to_str state.handle},\"sent\":${Num.to_str state.sent},\"received\":${Num.to_str state.received}}",

    deserializeState: |stateJson|
        connected =
            when Str.split_first stateJson "\"connected\":true" is
                Ok _ -> Bool.true
                Err _ -> Bool.false
        handle =
            when Str.split_first stateJson "\"handle\":" is
                Ok { after } ->
                    when Str.split_first after "," is
                        Ok { before } -> Str.to_u32 (Str.trim before) |> Result.with_default 0u32
                        Err _ -> 0u32

                Err _ -> 0u32
        sent =
            when Str.split_first stateJson "\"sent\":" is
                Ok { after } ->
                    when Str.split_first after "," is
                        Ok { before } -> Str.to_u64 (Str.trim before) |> Result.with_default 0u64
                        Err _ -> 0u64

                Err _ -> 0u64
        received =
            when Str.split_first stateJson "\"received\":" is
                Ok { after } ->
                    cleaned = Str.trim after
                    when Str.split_first cleaned "}" is
                        Ok { before } -> Str.to_u64 (Str.trim before) |> Result.with_default 0u64
                        Err _ -> 0u64

                Err _ -> 0u64
        Ok {
            connected,
            handle,
            sent,
            received,
        },
}

