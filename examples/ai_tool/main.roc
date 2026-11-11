app [agent] { pf: platform "../../platform/main.roc" }

import pf.Json exposing [escape]
import pf.Golem exposing [Agent, agent, readOnly, statefulTool, param]

# Agent State
State : {
	invocations : I64,
	lastTool : Str,
}

calculatorTool = statefulTool({
	name: "calculator",
	description: "Evaluates mathematical expressions safely",
	params: [param("expression", "Math expression to evaluate")],
	execute: |state, _args|
		Ok({
			state: { invocations: state.invocations + 1.I64, lastTool: "calculator" },
			stateChanged: True,
			output: "{\"result\":42}",
		}),
})

echoTool = statefulTool({
	name: "echo",
	description: "Echoes back the input arguments",
	params: [param("text", "Text to echo")],
	execute: |state, args|
		Ok({
			state: { invocations: state.invocations + 1.I64, lastTool: "echo" },
			stateChanged: True,
			output: "{\"echo\":\"${escape(args)}\"}",
		}),
})

agent : Agent(State)
agent = Golem.agent({
	name: "ai-tool-agent",
	initial: { invocations: 0.I64, lastTool: "none" },
	tools: [calculatorTool, echoTool],
	onMessage: |state, _message|
		readOnly(state, "I am an AI assistant agent. You can invoke my tools (calculator, echo, memory)."),
})
