app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, defineAgent]

State :: {
	count : I64,
}

agent : Agent(State)
agent = defineAgent({
	init!: |_config|
		{ count: 0 },

	handleMessage!: |state, message|
		match message {
			"increment" => {
				nextCount = state.count + 1
				{
					state: { count: nextCount },
					replies: ["Counter incremented to ${I64.to_str(nextCount)}"],
				}
			}

			"decrement" => {
				nextCount = state.count - 1
				{
					state: { count: nextCount },
					replies: ["Counter decremented to ${I64.to_str(nextCount)}"],
				}
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

	handleToolCall!: |state, toolCall|
		match toolCall.name {
			"get_count" =>
				{
					state,
					result: {
						id: toolCall.id,
						success: True,
						output: "{\"count\": ${I64.to_str(state.count)}}",
					},
				}

			_ =>
				{
					state,
					result: {
						id: toolCall.id,
						success: False,
						output: "Unknown tool ${toolCall.name}",
					},
				}
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
		"{\"count\":${I64.to_str(state.count)}}",

	deserializeState: |stateJson|
		match Str.split_first(stateJson, "\"count\":") {
			Ok({ before: _, after }) =>
				match Str.split_first(after, "}") {
					Ok({ before, after: _ }) =>
						match I64.from_str(Str.trim(before)) {
							Ok(c) => Ok({ count: c })
							Err(_) => Err("Failed to parse count integer")
						}

					Err(_) => Err("Invalid JSON format")
				}

			Err(_) => Err("Missing count field")
		},
})
