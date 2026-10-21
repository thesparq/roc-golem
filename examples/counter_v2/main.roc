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
		openapiProvider: NoOpenApi,
	},

	serializeState: |state|
		"{\"count\":${I64.to_str(state.count)},\"note\":\"${state.note}\"}",

	deserializeState: |stateJson|
		match Str.split_first(stateJson, "\"count\":") {
			Ok({ before: _, after }) =>
				match Str.split_first(after, ",") {
					# The pre-update format: `{"count":N}`, written by the older
					# revision and carried over by the snapshot. Migrating it here
					# is the app's job — the platform hands the state JSON across
					# unchanged.
					Err(_) =>
						match Str.split_first(after, "}") {
							Ok({ before: countStr, after: _ }) =>
								match I64.from_str(countStr) {
									Ok(c) => Ok({ count: c, note: "migrated" })
									Err(_) => Err("Failed to parse count integer")
								}

							Err(_) => Err("Invalid JSON format")
						}

					Ok({ before: countStr, after: rest }) =>
						match Str.split_first(rest, "\"note\":\"") {
							Ok({ before: _, after: noteRest }) =>
								match Str.split_first(noteRest, "\"") {
									Ok({ before: note, after: _ }) =>
										match I64.from_str(countStr) {
											Ok(c) => Ok({ count: c, note: note })
											Err(_) => Err("Failed to parse count integer")
										}

									Err(_) => Err("Invalid note format")
								}

							Err(_) => Err("Missing note field")
						}

					Err(_) => Err("Invalid JSON format")
				}

			Err(_) => Err("Missing count field")
		},
})
