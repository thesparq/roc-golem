app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, defineAgent]
import pf.Types exposing [ToolCall, ToolResult]

# Agent State
State : {
    count : I64,
    history : List Str,
}

agent : Agent State
agent = defineAgent {
    init: |_config|
        Ok { count: 0, history: [] },

    handleMessage: |state, message|
        when message is
            "increment" ->
                nextCount = state.count + 1
                nextState = {
                    count: nextCount,
                    history: List.append state.history "increment",
                }
                Ok {
                    state: nextState,
                    response: "Counter incremented to ${Num.toStr nextCount}",
                }

            "decrement" ->
                nextCount = state.count - 1
                nextState = {
                    count: nextCount,
                    history: List.append state.history "decrement",
                }
                Ok {
                    state: nextState,
                    response: "Counter decremented to ${Num.toStr nextCount}",
                }

            "get" ->
                Ok {
                    state,
                    response: "Current count is ${Num.toStr state.count}",
                }

            _ ->
                Ok {
                    state,
                    response: "Unknown command '${message}'. Available: increment, decrement, get",
                },

    handleToolCall: |state, toolCall|
        when toolCall.name is
            "get_count" ->
                Ok {
                    state,
                    result: {
                        id: toolCall.id,
                        success: Bool.true,
                        output: "{\"count\": ${Num.toStr state.count}}",
                    },
                }

            _ ->
                Ok {
                    state,
                    result: {
                        id: toolCall.id,
                        success: Bool.false,
                        output: "Unknown tool ${toolCall.name}",
                    },
                },

    metadata: {
        name: "counter-agent",
        version: "1.0.0",
        description: "A durable Roc counter agent running on Golem Cloud",
        tools: [
            {
                name: "get_count",
                description: "Returns current counter value",
                parameters: [],
            },
        ],
    },

    serializeState: |state|
        """
        {"count":${Num.toStr state.count}}
        """,

    deserializeState: |_stateJson|
        Ok { count: 0, history: [] },
}
