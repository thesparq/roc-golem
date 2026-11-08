app [agent] { pf: platform "../../platform/main.roc" }

import pf.Json
import pf.Golem exposing [Agent, defineAgent]

# Agent State
State : {
	invocations : I64,
	lastTool : Str,
}

agent : Agent(State)
agent = defineAgent({
	init: |_config|
		Ok({ invocations: 0.I64, lastTool: "none" }),

	handleMessage: |state, _message|
		Ok({
			state,
			response: "I am an AI assistant agent. You can invoke my tools (calculator, echo, memory).",
		}),

	handleToolCall: |state, toolCall| {
		nextInvocations = state.invocations + 1.I64
		match toolCall.name {
			"calculator" =>
				Ok({
					state: { invocations: nextInvocations, lastTool: "calculator" },
					result: {
						id: toolCall.id,
						success: True,
						output: Json.encode({ result: 42.I64 }),
					},
				})

			"echo" =>
				Ok({
					state: { invocations: nextInvocations, lastTool: "echo" },
					result: {
						id: toolCall.id,
						success: True,
						output: Json.encode({ echo: toolCall.arguments }),
					},
				})

			_ =>
				Ok({
					state,
					result: {
						id: toolCall.id,
						success: False,
						output: Json.encode({ error: "Tool not found" }),
					},
				})
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
		Json.encode(state),

	deserializeState: |stateJson|
		Ok(Json.decode_or(stateJson, { invocations: 0.I64, lastTool: "none" })),
})
