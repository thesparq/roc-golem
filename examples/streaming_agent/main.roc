app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [
    Agent,
    defineAgent,
    logInfo,
    websocketConnect,
    websocketSend,
    websocketReceive,
    websocketClose,
]
import pf.Types exposing [ToolCall, ToolResult]

# Agent State
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
                # In Golem, we can initiate websocket connection
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
        """
        {"connected":${connStr},"handle":${Num.toStr state.handle},"sent":${Num.toStr (List.len state.messagesSent)}}
        """,

    deserializeState: |_stateJson|
        Ok {
            connected: Bool.false,
            handle: 0u32,
            messagesSent: [],
            messagesReceived: [],
        },
}
