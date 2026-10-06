app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, defineAgent]
import pf.Golem exposing [http!, rpc!, logInfo!]

# Agent State
#
# `baseUrl` is taken from the agent config when it looks like a URL, so a test
# harness can hand the agent an address to call; otherwise it falls back to a
# local default.
State :: {
	baseUrl : Str,
	lastStatus : Str,
	lastBody : Str,
	lastRpc : Str,
}

agent : Agent(State)
agent = defineAgent({
	init!: |config|
		{
			baseUrl: if config_starts_with_http(config) config else "http://127.0.0.1:18080/",
			lastStatus: "none",
			lastBody: "",
			lastRpc: "none",
		},

	handleMessage!: |state, message|
		match message {
			"fetch" => {
				request = "{\"url\":\"${state.baseUrl}\",\"method\":\"GET\"}"
				match http!(request) {
					Ok(responseJson) => {
						status = extract_json_str(responseJson, "status") ?? "unknown"
						body = extract_json_str(responseJson, "body") ?? ""
						logInfo!("http call returned status ${status}")
						{
							state: { ..state, lastStatus: status, lastBody: quote_free(body) },
							# The platform hands the whole response over as JSON
							# (`{status, headers, body}`); parsing the body properly is
							# the app's business, so this example only reports its size.
							replies: ["HTTP ${status} (${U64.to_str(Str.count_utf8_bytes(responseJson))} byte response)"],
						}
					}

					Err(err) =>
						{
							state,
							replies: ["HTTP failed: ${err}"],
						}
					}
			}

			"rpc" =>
			# The target is an agent id: <agent-type>(<constructor params>).
			# Cross-component calls also need the target declared as an agent-type
			# dependency, which this platform does not emit yet.
				match rpc!("counter-agent(\"{}\")", "handle-message", "increment") {
					Ok(reply) =>
						{
							state: { ..state, lastRpc: reply },
							replies: ["RPC reply: ${reply}"],
						}

					Err(err) =>
						{
							state,
							replies: ["RPC failed: ${err}"],
						}
					}

			_ =>
				{
					state,
					replies: ["Commands: fetch, rpc"],
				}
			},

	handleToolCall!: |state, toolCall|
		{
			state,
			result: {
				id: toolCall.id,
				success: False,
				output: "{\"error\": \"Unknown tool ${toolCall.name}\"}",
			},
		},

	metadata: {
		name: "effects-agent",
		version: "1.0.0",
		description: "Calls HTTP endpoints and other agents from a Golem Cloud agent",
		tools: [],
	},

	serializeState: |state|
		"{\"baseUrl\":\"${state.baseUrl}\",\"lastStatus\":\"${state.lastStatus}\",\"lastBody\":\"${state.lastBody}\",\"lastRpc\":\"${state.lastRpc}\"}",

	deserializeState: |stateJson|
		Ok({
			baseUrl: extract_json_str(stateJson, "baseUrl") ?? "http://127.0.0.1:18080/",
			lastStatus: extract_json_str(stateJson, "lastStatus") ?? "none",
			lastBody: extract_json_str(stateJson, "lastBody") ?? "",
			lastRpc: extract_json_str(stateJson, "lastRpc") ?? "none",
		}),
})

## The state is embedded in JSON without an escaper, so keep stored strings free
## of characters that would break it. The reply itself is escaped by the
## platform.
quote_free : Str -> Str
quote_free = |text| {
	no_quotes = Str.replace_each(text, "\"", "'")
	Str.replace_each(no_quotes, "\\", "/")
}

config_starts_with_http : Str -> Bool
config_starts_with_http = |config|
	match Str.split_first(config, "http") {
		Ok(_) => True
		Err(_) => False
	}

## Minimal JSON field reader, matching the style used by the platform's own
## hooks: it looks for `"field":"` first and takes everything up to the closing
## quote, and falls back to reading a bare (numeric) value up to `,` or `}`,
## which is what the HTTP response's `status` is.
extract_json_str : Str, Str -> Try(Str, [NotFound])
extract_json_str = |json, field|
	match Str.split_first(json, "\"${field}\":\"") {
		Ok({ before: _, after }) =>
			match Str.split_first(after, "\"") {
				Ok({ before, after: _ }) => Ok(before)
				Err(_) => Err(NotFound)
			}
		Err(_) =>
			match Str.split_first(json, "\"${field}\":") {
				Ok({ before: _, after }) =>
					match Str.split_first(after, ",") {
						Ok({ before, after: _ }) => Ok(Str.trim(before))
						Err(_) =>
							match Str.split_first(after, "}") {
								Ok({ before, after: _ }) => Ok(Str.trim(before))
								Err(_) => Ok(Str.trim(after))
							}
						}
				Err(_) => Err(NotFound)
			}
		}
