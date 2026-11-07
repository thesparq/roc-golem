app [main] { pf: platform "../platform/main.roc" }

import pf.Golem exposing [Agent, defineAgent]

State : {}

escape_json_str : Str -> Str
escape_json_str = |input|
	input
		|> Str.replace_each("\\", "\\\\")
		|> Str.replace_each("\"", "\\\"")
		|> Str.replace_each("\n", "\\n")
		|> Str.replace_each("\r", "\\r")
		|> Str.replace_each("\t", "\\t")

format_reply_envelope : Str, Str -> Str
format_reply_envelope = |next_state_json, response| {
	escaped_state = escape_json_str(next_state_json)
	escaped_response = escape_json_str(response)
	"{\"state\":\"${escaped_state}\",\"response\":\"${escaped_response}\"}"
}

format_tool_envelope : Str, Bool, Str -> Str
format_tool_envelope = |next_state_json, success, output| {
	escaped_state = escape_json_str(next_state_json)
	escaped_output = escape_json_str(output)
	success_str = if success "true" else "false"
	"{\"state\":\"${escaped_state}\",\"success\":${success_str},\"output\":\"${escaped_output}\"}"
}

# 1. Basic JSON state escaping
expect
	format_reply_envelope("{\"count\":1}", "Counter incremented") == "{\"state\":\"{\\\"count\\\":1}\",\"response\":\"Counter incremented\"}"

# 2. Empty state (read-only zero-serialization bypass)
expect
	format_reply_envelope("", "Current count is 1") == "{\"state\":\"\",\"response\":\"Current count is 1\"}"

# 3. Complex state with quotes, backslashes, newlines, tabs, and unicode
expect
	format_reply_envelope("{\"path\":\"C:\\\\dir\",\"msg\":\"line1\\nline2\\t\\\"hello\\\" 🚀\"}", "ok") == "{\"state\":\"{\\\"path\\\":\\\"C:\\\\\\\\dir\\\",\\\"msg\\\":\\\"line1\\\\nline2\\\\t\\\\\\\"hello\\\\\\\" 🚀\\\"}\",\"response\":\"ok\"}"

# 4. Tool call envelope formatting
expect
	format_tool_envelope("{\"invocations\":3}", True, "{\"result\":42}") == "{\"state\":\"{\\\"invocations\\\":3}\",\"success\":true,\"output\":\"{\\\"result\\\":42}\"}"

agent : Agent({})
agent = defineAgent({
	init: |_| Ok({}),
	handleMessage: |st, _| Ok({ state: st, response: "ok" }),
	handleToolCall: |st, call| Ok({ state: st, result: { id: call.id, success: True, output: "{}" } }),
	metadata: {
		name: "envelope-test-agent",
		version: "1.0.0",
		description: "Test agent for reply envelope unit testing",
		tools: [],
	},
	serializeState: |_| "{}",
	deserializeState: |_| Ok({}),
})

main = "ok"
