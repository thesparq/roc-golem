app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, defineAgent]

# Agent State
State :: {
	connected : Bool,
	handle : U32,
	sent : U64,
	received : U64,
}

agent : Agent(State)
agent = defineAgent({
	init!: |_config|
		{
			connected: False,
			handle: 0,
			sent: 0,
			received: 0,
		},

	handleMessage!: |state, message|
		match message {
			"connect" =>
				{
					state: { ..state, connected: True, handle: 1 },
					replies: ["WebSocket connected (handle=1)"],
				}

			"status" => {
				statusStr = if state.connected "connected" else "disconnected"
				{
					state,
					replies: ["WebSocket is ${statusStr}. Sent ${U64.to_str(state.sent)}, Received ${U64.to_str(state.received)}"],
				}
			}

			_ =>
				if state.connected {
					{
						state: { ..state, sent: state.sent + 1 },
						replies: ["Queued message for WebSocket: ${message}"],
					}
				} else {
					{
						state,
						replies: ["Not connected. Send 'connect' first."],
					}
				}
			},

	handleToolCall!: |state, toolCall|
		match toolCall.name {
			"stream_message" =>
				{
					state,
					result: {
						id: toolCall.id,
						success: True,
						output: "{\"status\": \"stream_active\", \"sent_count\": ${U64.to_str(state.sent)}}",
					},
				}

			_ =>
				{
					state,
					result: {
						id: toolCall.id,
						success: False,
						output: "{\"error\": \"Unknown tool ${toolCall.name}\"}",
					},
				}
			},

	metadata: {
		name: "streaming-agent",
		version: "1.0.0",
		description: "Durable WebSocket streaming agent running on Golem Cloud",
		tools: [
			{
				name: "stream_message",
				description: "Streams message via WebSocket",
				parameters: [
					{
						name: "content",
						description: "Message to stream",
						paramType: "string",
						required: True,
					},
				],
			},
		],
	},

	serializeState: |state| {
		connStr = if state.connected "true" else "false"
		"{\"connected\":${connStr},\"handle\":${U32.to_str(state.handle)},\"sent\":${U64.to_str(state.sent)},\"received\":${U64.to_str(state.received)}}"
	},

	deserializeState: |stateJson| {
		connected =
			match Str.split_first(stateJson, "\"connected\":true") {
				Ok(_) => True
				Err(_) => False
			}
		handle =
			match Str.split_first(stateJson, "\"handle\":") {
				Ok({ before: _, after }) =>
					match Str.split_first(after, ",") {
						Ok({ before, after: _ }) => U32.from_str(Str.trim(before)) ?? 0
						Err(_) => 0
					}

				Err(_) => 0
			}
		sent =
			match Str.split_first(stateJson, "\"sent\":") {
				Ok({ before: _, after }) =>
					match Str.split_first(after, ",") {
						Ok({ before, after: _ }) => U64.from_str(Str.trim(before)) ?? 0
						Err(_) => 0
					}

				Err(_) => 0
			}
		received =
			match Str.split_first(stateJson, "\"received\":") {
				Ok({ before: _, after }) => {
					cleaned = Str.trim(after)
					match Str.split_first(cleaned, "}") {
						Ok({ before, after: _ }) => U64.from_str(Str.trim(before)) ?? 0
						Err(_) => 0
					}
				}

				Err(_) => 0
			}
		Ok({
			connected,
			handle,
			sent,
			received,
		})
	},
})
