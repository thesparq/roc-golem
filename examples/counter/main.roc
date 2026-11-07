app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, defineAgent]

# Agent State
State : {
	count : I64,
}

agent : Agent(State)
agent = defineAgent({
	init: |_config|
		Ok({ count: 0.I64 }),

	handleMessage: |state, message|
		match message {
			"increment" => {
				nextCount = state.count + 1.I64
				Ok({
					state: { count: nextCount },
					response: "Counter incremented to ${Str.inspect(nextCount)}",
				})
			}

			"decrement" => {
				nextCount = state.count - 1.I64
				Ok({
					state: { count: nextCount },
					response: "Counter decremented to ${Str.inspect(nextCount)}",
				})
			}

			"get" =>
				Ok({
					state,
					response: "Current count is ${Str.inspect(state.count)}",
				})

			_ =>
				Ok({
					state,
					response: "Unknown command '${message}'. Available: increment, decrement, get",
				})
		},

	handleToolCall: |state, toolCall|
		match toolCall.name {
			"get_count" =>
				Ok({
					state,
					result: {
						id: toolCall.id,
						success: True,
						output: "{\"count\": ${Str.inspect(state.count)}}",
					},
				})

			_ =>
				Ok({
					state,
					result: {
						id: toolCall.id,
						success: False,
						output: "Unknown tool ${toolCall.name}",
					},
				})
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
		"{\"count\":${Str.inspect(state.count)}}",

	deserializeState: |stateJson| {
		count : I64
		count =
			match Str.split_first(stateJson, "\"count\"") {
				Ok({ after, .. }) =>
					match Str.split_first(after, ":") {
						Ok({ after: afterColon, .. }) => {
							cleaned = Str.trim(afterColon)
							numStr =
								match Str.split_first(cleaned, ",") {
									Ok({ before, .. }) => before
									Err(_) =>
										match Str.split_first(cleaned, "}") {
											Ok({ before, .. }) => before
											Err(_) => cleaned
										}
								}
							I64.from_str(Str.trim(numStr)) ?? 0.I64
						}

						Err(_) => 0.I64
					}

				Err(_) => 0.I64
			}
		Ok({ count: count })
	},
})
