app [main] { pf: platform "../platform/main.roc" }

import pf.Types exposing [Result]
import pf.Golem exposing [
	Agent,
	Tool,
	StatelessTool,
	agent,
	simpleAgent,
	ephemeral,
	simpleEphemeral,
	tool,
	toolJson,
	statefulTool,
	statelessTool,
	statelessToolJson,
	statelessToTool,
	param,
	optionalParam,
	reply,
	readOnly,
	stay,
	fail,
	replyJson,
	readOnlyJson,
	dispatchTool,
	dispatchMessage,
	dispatchStatelessTool,
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
	queryTool : Tool(State)
	queryTool = tool({
		name: "get_name",
		description: "Returns agent name",
		params: [],
		execute: |st, _args| Ok(st.name),
	})
	isMatch =
		match (queryTool.execute)({ count: 0.I64, name: "Golem" }, "") {
			Ok(res) => res.stateChanged == False and res.output == "Golem" and res.state.count == 0.I64
			Err(_) => False
		}
	queryTool.definition.name == "get_name" and isMatch
}

# toolJson: read-only tool returning JSON value
expect {
	calcTool : Tool(State)
	calcTool = toolJson({
		name: "metrics",
		description: "Returns count metrics",
		params: [],
		execute: |st, _args| Ok({ active: "True", count: st.count }),
	})
	match (calcTool.execute)({ count: 10.I64, name: "calc" }, "") {
		Ok(res) => res.stateChanged == False and res.output == "{\"active\":\"True\",\"count\":10}"
		Err(_) => False
	}
}

# statefulTool: mutates state, stateChanged = True
expect {
	incrementTool : Tool(State)
	incrementTool = statefulTool({
		name: "add",
		description: "Adds amount to count",
		params: [param("amount", "Amount to add")],
		execute: |st, _args|
			Ok({
				state: { count: st.count + 5.I64, name: st.name },
				stateChanged: True,
				output: "Added 5",
			}),
	})
	match (incrementTool.execute)({ count: 10.I64, name: "calc" }, "") {
		Ok(res) => res.stateChanged == True and res.state.count == 15.I64 and res.output == "Added 5"
		Err(_) => False
	}
}

# statelessTool: purely stateless execution
expect {
	echoTool = statelessTool({
		name: "echo",
		description: "Echoes input",
		params: [param("text", "Text to echo")],
		execute: |args| Ok("echo: ${args}"),
	})
	(echoTool.execute)("hello") == Ok("echo: hello")
}

# statelessToolJson: stateless returning JSON
expect {
	healthTool = statelessToolJson({
		name: "health",
		description: "Healthcheck",
		params: [],
		execute: |_| Ok({ healthy: "True" }),
	})
	(healthTool.execute)("") == Ok("{\"healthy\":\"True\"}")
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
	match (lifted.execute)({ count: 99.I64, name: "bot" }, "ping") {
		Ok(res) => res.stateChanged == False and res.state.count == 99.I64 and res.output == "echo: ping"
		Err(_) => False
	}
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
	onMessage: testOnMessage,
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

# Verify tool call auto-dispatch, ID forwarding, and envelope construction
expect {
	initialState = { count: 42.I64, name: "test-agent" }
	res = dispatchTool(
		testTools,
		initialState,
		{ id: "call-123", name: "get_count", arguments: "{}" },
		serializeState,
	)
	match res {
		Ok(r) =>
			r.result.id == "call-123"
				and r.result.success == True
					and r.result.output == "42"
						and r.stateChanged == False
							and r.nextStateJson == ""
		Err(_) => False
	}
}

# Verify stateful tool call persists state and sets stateChanged = True
expect {
	initialState = { count: 42.I64, name: "test-agent" }
	res = dispatchTool(
		testTools,
		initialState,
		{ id: "call-456", name: "increment_by", arguments: "{}" },
		serializeState,
	)
	match res {
		Ok(r) =>
			r.result.id == "call-456"
				and r.result.success == True
					and r.result.output == "Incremented"
						and r.stateChanged == True
							and r.nextStateJson == "{\"count\":43,\"name\":\"test-agent\"}"
		Err(_) => False
	}
}

# Verify unknown tool call returns success = False with descriptive error and doesn't mutate state
expect {
	initialState = { count: 42.I64, name: "test-agent" }
	res = dispatchTool(
		testTools,
		initialState,
		{ id: "call-999", name: "non_existent", arguments: "{}" },
		serializeState,
	)
	match res {
		Ok(r) =>
			r.result.id == "call-999"
				and r.result.success == False
					and r.result.output == "Unknown tool 'non_existent'"
						and r.stateChanged == False
		Err(_) => False
	}
}

# Verify readOnly onMessage triggers safe zero-serialization bypass (stateChanged = False)
expect {
	initialState = { count: 10.I64, name: "test-agent" }
	res = dispatchMessage(
		testOnMessage,
		initialState,
		"get",
		serializeState,
	)
	match res {
		Ok(r) =>
			r.stateChanged == False
				and r.nextStateJson == ""
					and r.response == "count is 10"
		Err(_) => False
	}
}

# Verify mutating onMessage updates state and sets stateChanged = True
expect {
	initialState = { count: 10.I64, name: "test-agent" }
	res = dispatchMessage(
		testOnMessage,
		initialState,
		"inc",
		serializeState,
	)
	match res {
		Ok(r) =>
			r.stateChanged == True
				and r.nextStateJson == "{\"count\":11,\"name\":\"test-agent\"}"
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
	toolRes = dispatchStatelessTool(diagTools, { id: "c-1", name: "ping", arguments: "" })
	toolOk =
		match toolRes {
			Ok(r) => r.result.id == "c-1" and r.result.success == True and r.result.output == "pong" and r.stateChanged == False
			Err(_) => False
		}
	diagAgent.metadata.mode == Ephemeral and toolOk
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
