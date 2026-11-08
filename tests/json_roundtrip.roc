app [main] { pf: platform "../platform/main.roc" }

import pf.Json
import pf.Types exposing [Result]
import pf.Golem exposing [Agent, defineAgent]

# Dummy state for platform requirement
State : {}

# --- State and data types for testing ---

CounterState : {
	count : I64,
}

AiToolState : {
	invocations : I64,
	lastTool : Str,
}

StreamingState : {
	connected : Bool,
	handle : U32,
	sent : U64,
	received : U64,
}

CanaryStatus : {
	service : Str,
	activeVersion : Str,
	canaryVersion : Str,
	weight : U64,
	region : Str,
	pendingTimerHandle : U32,
	approvalWebhookUrl : Str,
	events : List(Str),
}

RollbackInput : {
	reason : Str,
}

LogBatch : {
	service : Str,
	lines : Str,
}

DiagnosticReport : {
	service : Str,
	faults : U64,
	healthy : Bool,
	summary : Str,
}

ToolCallEnvelope : {
	id : Str,
	name : Str,
	arguments : Str,
}

# --- 1. Primitives: Negative numbers, zero, booleans, strings with quotes, newlines, tabs, unicode, and emojis ---

# Primitive integers
expect {
	val : { a : I64, b : I64, c : I64 }
	val = { a: -100.I64, b: 0.I64, c: 999999.I64 }
	encoded = Json.encode(val)
	decoded = Json.decode(encoded)
	decoded == Ok(val)
}

# Primitive booleans
expect {
	val : { t : Bool, f : Bool }
	val = { t: True, f: False }
	encoded = Json.encode(val)
	decoded = Json.decode(encoded)
	decoded == Ok(val)
}

# Strings with quotes, newlines, tabs, unicode, and emojis
expect {
	specialStr = "Line 1\nLine 2\t\"Quotes\" and \\backslashes\\ and \r carriage return: 🚀 \u(1f389) 🎉"
	val : { text : Str }
	val = { text: specialStr }
	encoded = Json.encode(val)
	decoded = Json.decode(encoded)
	decoded == Ok(val)
}

# --- 2. Whitespace invariance: spaces around colons, indentation, newlines ---

expect {
	# Spaces around colon
	rawJson = "{\"count\" : 42}"
	res : Result(CounterState, Str)
	res = Json.decode(rawJson)
	res == Ok({ count: 42.I64 })
}

expect {
	# Indented with newlines and multiple spaces
	rawJson = "{\n  \"count\":   42\n}"
	res : Result(CounterState, Str)
	res = Json.decode(rawJson)
	res == Ok({ count: 42.I64 })
}

expect {
	# Complex whitespace with tabs and spaces
	rawJson = "\t{\n\t\t\"invocations\"\t:\t  7,\n\t\t\"lastTool\" : \"calculator\"\n\t}\n"
	res : Result(AiToolState, Str)
	res = Json.decode(rawJson)
	res == Ok({ invocations: 7.I64, lastTool: "calculator" })
}

# --- 3. Round-trip tests for every example's state type ---

# Counter: { count : I64 }
expect {
	initialState : CounterState
	initialState = { count: -42.I64 }
	encoded = Json.encode(initialState)
	decoded = Json.decode(encoded)
	decoded == Ok(initialState)
}

expect {
	initialState : CounterState
	initialState = { count: 0.I64 }
	encoded = Json.encode(initialState)
	decoded = Json.decode(encoded)
	decoded == Ok(initialState)
}

# AI Tool: { invocations : I64, lastTool : Str } with special characters, quotes, and newlines in lastTool
expect {
	initialState : AiToolState
	initialState = {
		invocations: 12.I64,
		lastTool: "echo\n\"quotes\" and \\slashes\\\twith\r\nlines and 🤖 emojis",
	}
	encoded = Json.encode(initialState)
	decoded = Json.decode(encoded)
	decoded == Ok(initialState)
}

# Streaming: { connected : Bool, handle : U32, sent : U64, received : U64 }
expect {
	initialState : StreamingState
	initialState = {
		connected: True,
		handle: 42.U32,
		sent: 1000.U64,
		received: 2000.U64,
	}
	encoded = Json.encode(initialState)
	decoded = Json.decode(encoded)
	decoded == Ok(initialState)
}

expect {
	initialState : StreamingState
	initialState = {
		connected: False,
		handle: 0.U32,
		sent: 0.U64,
		received: 0.U64,
	}
	encoded = Json.encode(initialState)
	decoded = Json.decode(encoded)
	decoded == Ok(initialState)
}

# Canary Release: Status, RollbackInput, LogBatch, DiagnosticReport
expect {
	status : CanaryStatus
	status = {
		service: "payments-service",
		activeVersion: "v1.2.0",
		canaryVersion: "v1.3.0-rc1",
		weight: 25.U64,
		region: "us-west-2",
		pendingTimerHandle: 99.U32,
		approvalWebhookUrl: "https://golem.cloud/webhooks/canary/payments-service/gate-approve",
		events: [
			"coordinator_initialized",
			"canary_started: v1.3.0-rc1 at 25%",
			"health_check_passed: latency < 50ms\nerror_rate = 0.00%",
		],
	}
	encoded = Json.encode(status)
	decoded = Json.decode(encoded)
	decoded == Ok(status)
}

expect {
	rollback : RollbackInput
	rollback = {
		reason: "Error spike > 5% on canary pods: \"500 Internal Error\"\npanic: nil pointer",
	}
	encoded = Json.encode(rollback)
	decoded = Json.decode(encoded)
	decoded == Ok(rollback)
}

expect {
	batch : LogBatch
	batch = {
		service: "auth-gateway",
		lines: "2026-11-08T10:00:00Z [INFO] Started\n2026-11-08T10:00:01Z [WARN] Retrying connection \"db-1\"",
	}
	encoded = Json.encode(batch)
	decoded = Json.decode(encoded)
	decoded == Ok(batch)
}

expect {
	diag : DiagnosticReport
	diag = {
		service: "billing-api",
		faults: 3.U64,
		healthy: False,
		summary: "Found 2 PANIC lines and 1 OOM in logs",
	}
	encoded = Json.encode(diag)
	decoded = Json.decode(encoded)
	decoded == Ok(diag)
}

# --- 4. Tool call envelope parsing resilience (spaces after colons) ---

expect {
	rawToolJson = "{\n  \"id\": \"call-42\",\n  \"name\": \"calculator\",\n  \"arguments\": \"{\\\"expr\\\": \\\"2+2\\\"}\"\n}"
	res : Result(ToolCallEnvelope, Str)
	res = Json.decode(rawToolJson)
	res == Ok({ id: "call-42", name: "calculator", arguments: "{\"expr\": \"2+2\"}" })
}

expect {
	rawToolJson = "{\"id\" : \"call-99\" , \"name\" : \"echo\" , \"arguments\" : \"hello world\"}"
	res : Result(ToolCallEnvelope, Str)
	res = Json.decode(rawToolJson)
	res == Ok({ id: "call-99", name: "echo", arguments: "hello world" })
}

# --- 5. Malformed JSON handling ---

expect {
	res : Result(CounterState, Str)
	res = Json.decode("{not valid json")
	Result.is_err(res)
}

expect {
	res : Result(CounterState, Str)
	res = Json.decode("")
	Result.is_err(res)
}

expect {
	res : Result(CounterState, Str)
	res = Json.decode("{\"count\": \"not a number\"}")
	Result.is_err(res)
}

# decode_or fallback
expect {
	val = Json.decode_or("{\"invalid\": true}", { count: 123.I64 })
	val == { count: 123.I64 }
}

expect {
	val = Json.decode_or("{\"count\": 999}", { count: 123.I64 })
	val == { count: 999.I64 }
}

# --- 6. String escaping helper ---

expect Json.escape("hello \"world\"\nline2\r\ttab\\backslash") == "hello \\\"world\\\"\\nline2\\r\\ttab\\\\backslash"

# Dummy agent to satisfy platform requirements
agent : Agent({})
agent = defineAgent({
	init: |_| Ok({}),
	handleMessage: |st, _| Ok({ state: st, response: "ok" }),
	handleToolCall: |st, call| Ok({ state: st, result: { id: call.id, success: True, output: "{}" } }),
	metadata: {
		name: "json-roundtrip-test-agent",
		version: "1.0.0",
		description: "Test agent for JSON roundtrip tests",
		tools: [],
	},
	serializeState: |_| "{}",
	deserializeState: |_| Ok({}),
})

main = "ok"
