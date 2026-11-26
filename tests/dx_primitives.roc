app [main] { pf: platform "../platform/main.roc" }

import pf.Types exposing [Result]
import pf.Golem exposing [
	Agent,
	StatelessTool,
	Tool,
	agent,
	ephemeral,
	fail,
	optionalParam,
	param,
	readOnly,
	readOnlyJson,
	reply,
	replyJson,
	simpleAgent,
	simpleEphemeral,
	statefulTool,
	statelessToTool,
	statelessTool,
	statelessToolJson,
	stay,
	tool,
	toolJson,
]

State : { count : I64, name : Str }

# ==============================================================================
# 1. Response Primitives Tests
# ==============================================================================

# reply: sets stateChanged = True, response = text
expect {
	res = reply({ count: 2.I64, name: "test" }, "incremented")
	res == Ok({ state: { count: 2.I64, name: "test" }, stateChanged: True, response: "incremented" })
}

# readOnly: sets stateChanged = False, response = text
expect {
	st = { count: 1.I64, name: "test" }
	res = readOnly(st, "current count is 1")
	res == Ok({ state: st, stateChanged: False, response: "current count is 1" })
}

# stay: sets stateChanged = False, response = ""
expect {
	st = { count: 1.I64, name: "test" }
	res = stay(st)
	res == Ok({ state: st, stateChanged: False, response: "" })
}

# fail: returns Err(errMsg)
expect {
	res : Result({ state : State, stateChanged : Bool, response : Str }, Str)
	res = fail("something broke")
	res == Err("something broke")
}

# replyJson: sets stateChanged = True, response = JSON serialized
expect {
	res = replyJson({ count: 2.I64, name: "test" }, { status: "ok", val: 42.I64 })
	res == Ok({ state: { count: 2.I64, name: "test" }, stateChanged: True, response: "{\"status\":\"ok\",\"val\":42}" })
}

# readOnlyJson: sets stateChanged = False, response = JSON serialized
expect {
	st = { count: 5.I64, name: "alice" }
	res = readOnlyJson(st, { count: st.count })
	res == Ok({ state: st, stateChanged: False, response: "{\"count\":5}" })
}

# ==============================================================================
# 2. Schema Parameter Helpers Tests
# ==============================================================================

# param: required string parameter
expect {
	p = param("user_id", "The user identifier")
	p == { name: "user_id", description: "The user identifier", paramType: "string", required: True }
}

# optionalParam: optional string parameter
expect {
	p = optionalParam("filter", "Optional filter string")
	p == { name: "filter", description: "Optional filter string", paramType: "string", required: False }
}

# ==============================================================================
# 3. Unified Tool Constructors & Execution Tests
# ==============================================================================

# tool: read-only state query tool (stateChanged = False)
expect {
	exec : State, Str -> Result(Str, Str)
	exec = |st, _args| Ok(st.name)
	queryTool : Tool(State)
	queryTool = tool({
		name: "get_name",
		description: "Returns agent name",
		params: [],
		execute: exec,
	})
	isMatch =
		match exec({ count: 0.I64, name: "Golem" }, "") {
			Ok(out) => out == "Golem"
			Err(_) => False
		}
	queryTool.definition.name == "get_name" and isMatch
}

# toolJson: read-only tool returning JSON value
expect {
	exec = |st, _args| Ok({ active: "True", count: st.count })
	calcTool : Tool(State)
	calcTool = toolJson({
		name: "metrics",
		description: "Returns count metrics",
		params: [],
		execute: exec,
	})
	calcTool.definition.name == "metrics"
}

# statefulTool: mutates state, stateChanged = True
expect {
	exec : State, Str -> Result({ state : State, stateChanged : Bool, output : Str }, Str)
	exec = |st, _args|
		Ok({
			state: { count: st.count + 5.I64, name: st.name },
			stateChanged: True,
			output: "Added 5",
		})
	incrementTool : Tool(State)
	incrementTool = statefulTool({
		name: "add",
		description: "Adds amount to count",
		params: [param("amount", "Amount to add")],
		execute: exec,
	})
	isMatch =
		match exec({ count: 10.I64, name: "calc" }, "") {
			Ok(res) => res.stateChanged == True and res.state.count == 15.I64 and res.output == "Added 5"
			Err(_) => False
		}
	incrementTool.definition.name == "add" and isMatch
}

# statelessTool: purely stateless execution
expect {
	exec = |args| Ok("echo: ${args}")
	echoTool = statelessTool({
		name: "echo",
		description: "Echoes input",
		params: [param("text", "Text to echo")],
		execute: exec,
	})
	echoTool.definition.name == "echo" and exec("hello") == Ok("echo: hello")
}

# statelessToolJson: stateless returning JSON
expect {
	exec = |_| Ok({ healthy: "True" })
	healthTool = statelessToolJson({
		name: "health",
		description: "Healthcheck",
		params: [],
		execute: exec,
	})
	healthTool.definition.name == "health"
}

# statelessToTool: lifts StatelessTool into Tool(state)
expect {
	echoTool = statelessTool({
		name: "echo",
		description: "Echoes input",
		params: [param("text", "Text to echo")],
		execute: |args| Ok("echo: ${args}"),
	})
	lifted : Tool(State)
	lifted = statelessToTool(echoTool)
	lifted.definition.name == "echo"
}

# ==============================================================================
# 4. Golem.agent Integration & Tool Auto-Dispatch Tests
# ==============================================================================

serializeState : State -> Str
serializeState = |s| Json.to_str(s)

testTools : List(Tool(State))
testTools = [
	tool({
		name: "get_count",
		description: "Returns count as string",
		params: [],
		execute: |st, _| Ok(Str.inspect(st.count)),
	}),
	statefulTool({
		name: "increment_by",
		description: "Increments count by 1",
		params: [],
		execute: |st, _|
			Ok({
				state: { count: st.count + 1.I64, name: st.name },
				stateChanged: True,
				output: "Incremented",
			}),
	}),
]

testOnMessage : State, Str -> Result({ state : State, stateChanged : Bool, response : Str }, Str)
testOnMessage = |st, msg|
	match msg {
		"inc" => reply({ count: st.count + 1.I64, name: st.name }, "incremented")
		"get" => readOnly(st, "count is ${Str.inspect(st.count)}")
		_ => fail("unknown command")
	}

testAgent : Agent(State)
testAgent = agent({
	name: "test-counter",
	initial: { count: 0.I64, name: "test-agent" },
	tools: testTools,
	onMessage: |st, msg| testOnMessage(st, msg),
})

# Verify erased metadata has tools registered automatically (no double-declaration)
expect {
	firstAgent =
		match testAgent.agents {
			[first, ..] => first
			_ => crash ("empty agents")
		}
	firstAgent.metadata.name == "test-counter"
		and List.len(firstAgent.metadata.tools) == 2
}

# Verify tool definitions in agent metadata
expect {
	firstAgent =
		match testAgent.agents {
			[first, ..] => first
			_ => crash ("empty agents")
		}
	match firstAgent.metadata.tools {
		[t1, t2] => t1.name == "get_count" and t2.name == "increment_by"
		_ => False
	}
}

# Verify readOnly onMessage triggers safe zero-serialization bypass (stateChanged = False)
expect {
	initialState = { count: 10.I64, name: "test-agent" }
	res = testOnMessage(initialState, "get")
	match res {
		Ok(r) =>
			r.stateChanged == False
				and r.response == "count is 10"
		Err(_) => False
	}
}

# Verify mutating onMessage updates state and sets stateChanged = True
expect {
	initialState = { count: 10.I64, name: "test-agent" }
	res = testOnMessage(initialState, "inc")
	match res {
		Ok(r) =>
			r.stateChanged == True
				and r.state.count == 11.I64
				and r.response == "incremented"
		Err(_) => False
	}
}

# ==============================================================================
# 5. Golem.ephemeral Tests
# ==============================================================================

diagTools : List(StatelessTool)
diagTools = [
	statelessTool({
		name: "ping",
		description: "Pings worker",
		params: [],
		execute: |_| Ok("pong"),
	}),
]

testDiagWorker : Agent({})
testDiagWorker = ephemeral({
	name: "diag-worker",
	tools: diagTools,
	onMessage: |msg| Ok("processed: ${msg}"),
})

expect {
	diagAgent =
		match testDiagWorker.agents {
			[first, ..] => first
			_ => crash ("empty agents")
		}
	diagAgent.metadata.mode == Ephemeral
		and match diagAgent.metadata.tools {
			[t] => t.name == "ping"
			_ => False
		}
}

# Simple agent without tools
testSimple : Agent(State)
testSimple = simpleAgent({
	name: "simple-counter",
	initial: { count: 0.I64, name: "simple" },
	onMessage: |st, _| reply(st, "ok"),
})

expect {
	simple =
		match testSimple.agents {
			[first, ..] => first
			_ => crash ("empty agents")
		}
	List.is_empty(simple.metadata.tools)
}

main = testAgent
