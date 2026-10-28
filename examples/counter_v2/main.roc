app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, defineAgent, getConfig!]

State :: {
	count : I64,
	note : Str,
}

agent : Agent(State)
agent = defineAgent({
	init!: |_config|
		{ count: 0, note: "v2" },

	handleMessage!: |state, message|
		match message {
			"increment" => {
				nextCount = state.count + 1
				{
					state: { count: nextCount, note: state.note },
					replies: ["Counter incremented to ${I64.to_str(nextCount)}"],
				}
			}

			"decrement" => {
				nextCount = state.count - 1
				{
					state: { count: nextCount, note: state.note },
					replies: ["Counter decremented to ${I64.to_str(nextCount)}"],
				}
			}

			"config" =>
				match getConfig!("greeting") {
					Ok(greeting) =>
						{
							state,
							replies: ["Config greeting: ${greeting}"],
						}

					Err(err) =>
						{
							state,
							replies: ["Config error: ${err}"],
						}
				}

			"note" =>
				{
					state,
					replies: ["Note is ${state.note}"],
				}

			"get" =>
				{
					state,
					replies: ["Current count is ${I64.to_str(state.count)}"],
				}

			_ =>
				{
					state,
					replies: ["Unknown command '${message}'. Available: increment, decrement, get"],
				}
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

	handleToolCall!: |state, toolCall|
		match toolCall.name {
			"set_note" =>
				{
					state: { count: state.count, note: toolCall.arguments },
					result: {
						id: toolCall.id,
						success: True,
						output: "note set",
						stdout: "",
						stderr: "",
					},
				}

			"get_count" =>
				{
					state,
					result: {
						id: toolCall.id,
						success: True,
						output: "{\"count\": ${I64.to_str(state.count)}}",
						stdout: "",
						stderr: "",
					},
				}

			_ =>
				{
					state,
					result: {
						id: toolCall.id,
						success: False,
						output: "Unknown tool ${toolCall.name}",
						stdout: "",
						stderr: "",
					},
				}
		},

	metadata: {
		name: "counter-agent",
		version: "2.0.0",
		description: "A durable Roc counter agent running on Golem Cloud",
		tools: [
			{
				name: "get_count",
				description: "Returns current counter value",
				parameters: [],
				readOnly: True,
			},
			{
				name: "set_note",
				description: "Stores a note next to the counter",
				parameters: [],
				readOnly: False,
			},
		],
		config: [
			{
				name: "greeting",
				secret: False,
			},
		],
		agentKind: Regular,
		httpMount: None,
		httpHandler: NoHandler,
		multimodal: NoMultimodal,
		openapiProvider: NoOpenApi,
	},

	serializeState: |state|
		Json.to_str({ count: state.count, note: state.note }),

	deserializeState: |stateJson| {
		# The pre-update format is `{"count":N}`, written by the older
		# revision and carried over by the snapshot; a missing `note` is that
		# format, and migrating it here is the app's job — the platform hands
		# the state JSON across unchanged.
		parsed : Try(
			{ count : I64, note : Try(Str, [Missing]) },
			[InvalidJson(Str), MissingRequiredField(Str)],
		)
		parsed = Json.parse(stateJson)
		match parsed {
			Ok(fields) => Ok({ count: fields.count, note: fields.note ?? "migrated" })
			Err(_) => Err("Missing count field")
		}
	},
})
