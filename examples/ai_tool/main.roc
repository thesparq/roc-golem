app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, defineAgent]

# Agent State
State :: {
	invocations : I64,
	lastTool : Str,
}

agent : Agent(State)
agent = defineAgent({
	init!: |_config|
		{ invocations: 0, lastTool: "none" },

	handleMessage!: |state, _message|
		{
			state,
			replies: ["I am an AI assistant agent. You can invoke my tools (calculator, echo, memory)."],
		},

	handleToolCall!: |state, toolCall| {
		nextInvocations = state.invocations + 1
		match toolCall.name {
			"calculator" =>
				{
					state: { invocations: nextInvocations, lastTool: "calculator" },
					result: {
						id: toolCall.id,
						success: True,
						output: "{\"result\": 42}",
					},
				}

			"echo" => {
				escapedArgs =
					toolCall.arguments
						|> Str.replace_each("\\", "\\\\")
						|> Str.replace_each("\"", "\\\"")
				{
					state: { invocations: nextInvocations, lastTool: "echo" },
					result: {
						id: toolCall.id,
						success: True,
						output: "{\"echo\": \"${escapedArgs}\"}",
					},
				}
			}

			_ =>
				{
					state,
					result: {
						id: toolCall.id,
						success: False,
						output: "{\"error\": \"Tool not found\"}",
					},
				}
			}
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
						required: True,
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
						required: True,
					},
				],
			},
		],
	},

	serializeState: |state|
		"{\"invocations\":${I64.to_str(state.invocations)},\"lastTool\":\"${state.lastTool}\"}",

	deserializeState: |stateJson| {
		invocations =
			match Str.split_first(stateJson, "\"invocations\":") {
				Ok({ before: _, after }) =>
					match Str.split_first(after, ",") {
						Ok({ before, after: _ }) => I64.from_str(Str.trim(before)) ?? 0
						Err(_) => 0
					}

				Err(_) => 0
			}
		lastTool =
			match Str.split_first(stateJson, "\"lastTool\":\"") {
				Ok({ before: _, after }) =>
					match Str.split_first(after, "\"") {
						Ok({ before, after: _ }) => before
						Err(_) => "none"
					}

				Err(_) => "none"
			}
		Ok({ invocations, lastTool })
	},
})
