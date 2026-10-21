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

	handleRequest!: |state, _request|
		{
			state,
			response: { status: 404, headers: [], body: "this agent is not an HTTP router" },
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
						stdout: "",
						stderr: "",
					},
				}

			# `echo` is the example's tool with streams: whatever the caller pipes
			# in (`golem tool invoke --stdin`) comes back out on stdout, and stderr
			# reports how much of it arrived. Both are written once this hook has
			# returned (see the README's tool streams section).
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
						stdout: toolCall.stdin,
						stderr: "echo tool: read ${U64.to_str(Str.count_utf8_bytes(toolCall.stdin))} bytes of stdin",
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
						stdout: "",
						stderr: "",
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
				readOnly: True,
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
				readOnly: True,
			},
		],
		config: [],
		agentKind: Regular,
		httpMount: None,
		httpHandler: NoHandler,
		openapiProvider: NoOpenApi,
	},

	serializeState: |state|
		"{\"invocations\":${I64.to_str(state.invocations)},\"lastTool\":\"${state.lastTool}\"}",

	deserializeState: |stateJson| {
		invocations =
			match Str.split_first(stateJson, "\"invocations\":") {
				Ok({ before: _, after }) =>
					match Str.split_first(after, ",") {
						Ok({ before, after: _ }) => I64.from_str(before) ?? 0
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
