app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, defineAgent]

# Agent State
State : {
    invocations : I64,
    lastTool : Str,
}

agent : Agent State
agent = defineAgent {
    init: |_config|
        Ok { invocations: 0, lastTool: "none" },

    handleMessage: |state, _message|
        Ok {
            state,
            response: "I am an AI assistant agent. You can invoke my tools (calculator, echo, memory).",
        },

    handleToolCall: |state, toolCall|
        nextInvocations = state.invocations + 1
        when toolCall.name is
            "calculator" ->
                Ok {
                    state: { invocations: nextInvocations, lastTool: "calculator" },
                    result: {
                        id: toolCall.id,
                        success: Bool.true,
                        output: "{\"result\": 42}",
                    },
                }

            "echo" ->
                escapedArgs =
                    toolCall.arguments
                    |> Str.replace_each "\\" "\\\\"
                    |> Str.replace_each "\"" "\\\""
                Ok {
                    state: { invocations: nextInvocations, lastTool: "echo" },
                    result: {
                        id: toolCall.id,
                        success: Bool.true,
                        output: "{\"echo\": \"${escapedArgs}\"}",
                    },
                }

            _ ->
                Ok {
                    state,
                    result: {
                        id: toolCall.id,
                        success: Bool.false,
                        output: "{\"error\": \"Tool not found\"}",
                    },
                },

    metadata: {
        name: "ai-tool-agent",
        version: "1.0.0",
        description: "An AI Agent providing calculator and utility tools on Golem Cloud",
        tools: [
            {
                name: "calculator",
                description: "Evaluates mathematical expressions safely",
                parameters: [
                    {
                        name: "expression",
                        description: "Math expression to evaluate",
                        paramType: "string",
                        required: Bool.true,
                    },
                ],
            },
            {
                name: "echo",
                description: "Echoes back the input arguments",
                parameters: [
                    {
                        name: "text",
                        description: "Text to echo",
                        paramType: "string",
                        required: Bool.true,
                    },
                ],
            },
        ],
    },

    serializeState: |state|
        "{\"invocations\":${Num.to_str state.invocations},\"lastTool\":\"${state.lastTool}\"}",

    deserializeState: |stateJson|
        invocations =
            when Str.split_first stateJson "\"invocations\":" is
                Ok { after } ->
                    when Str.split_first after "," is
                        Ok { before } -> Str.to_i64 (Str.trim before) |> Result.with_default 0
                        Err _ -> 0

                Err _ -> 0
        lastTool =
            when Str.split_first stateJson "\"lastTool\":\"" is
                Ok { after } ->
                    when Str.split_first after "\"" is
                        Ok { before } -> before
                        Err _ -> "none"

                Err _ -> "none"
        Ok { invocations, lastTool },
}

