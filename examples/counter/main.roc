app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, agent, reply, readOnly, tool]

# Agent State
State : {
	count : I64,
}

getCountTool = tool({
	name: "get_count",
	description: "Returns current counter value",
	params: [],
	execute: |state, _args| Ok("{\"count\":${Str.inspect(state.count)}}"),
})

agent : Agent(State)
agent = Golem.agent({
	name: "counter-agent",
	initial: { count: 0.I64 },
	tools: [getCountTool],
	onMessage: |state, message|
		match message {
			"increment" => {
				nextCount = state.count + 1.I64
				reply({ count: nextCount }, "Counter incremented to ${Str.inspect(nextCount)}")
			}

			"decrement" => {
				nextCount = state.count - 1.I64
				reply({ count: nextCount }, "Counter decremented to ${Str.inspect(nextCount)}")
			}

			"get" =>
				readOnly(state, "Current count is ${Str.inspect(state.count)}")

			_ =>
				readOnly(state, "Unknown command '${message}'. Available: increment, decrement, get")
		},
})
