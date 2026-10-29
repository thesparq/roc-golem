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

	# The multimodal method's handler. This app declares `multimodal: NoMultimodal`,
	# so it registers no such method and this is never called; an app that wants
	# multimodal content declares `HandlesMultimodal` instead and answers with the
	# envelope it was handed (see the README's "Multimodal values").
	handleMultimodal!: |state, _parts|
		{
			state,
			parts: "[{\"tag\":\"text\",\"value\":{\"text\":\"this agent does not handle multimodal input\",\"language\":null}}]",
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
		methods: [],
		config: [],
		agentKind: Regular,
		httpMount: None,
		httpHandler: NoHandler,
		multimodal: NoMultimodal,
		openapiProvider: NoOpenApi,
	},

	serializeState: |state|
		Json.to_str({ invocations: state.invocations, lastTool: state.lastTool }),

	deserializeState: |stateJson| {
		parsed : Try(
			{ invocations : Try(I64, [Missing]), lastTool : Try(Str, [Missing]) },
			[InvalidJson(Str)],
		)
		parsed = Json.parse(stateJson)
		match parsed {
			Err(_) => Ok({ invocations: 0, lastTool: "none" })
			Ok(fields) => Ok({
				invocations: fields.invocations ?? 0,
				lastTool: fields.lastTool ?? "none",
			})
		}
	},
})
