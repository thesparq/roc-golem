app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, defineAgent]

# Agent State
State : {
	connected : Bool,
	handle : U32,
	sent : U64,
	received : U64,
}

agent : Agent(State)
agent = defineAgent({
	init: |_config|
		Ok({
			connected: Bool.false,
			handle: 0,
			sent: 0,
			received: 0,
		}),

	handleMessage: |state, message|
		match message {
			"connect" =>
				Ok({
					state: { ..state, connected: Bool.true, handle: 1 },
					response: "WebSocket connected (handle=1)",
				})

			"status" => {
				statusStr = if state.connected "connected" else "disconnected"
				Ok({
					state,
					response: "WebSocket is ${statusStr}. Sent ${Num.to_str(state.sent)}, Received ${Num.to_str(state.received)}",
				})
			}

			_ =>
				if state.connected {
					Ok({
						state: { ..state, sent: state.sent + 1 },
						response: "Queued message for WebSocket: ${message}",
					})
				} else {
					Ok({
						state,
						response: "Not connected. Send 'connect' first.",
					})
				}
			},

	handleToolCall: |state, toolCall|
		match toolCall.name {
			"stream_message" =>
				Ok({
					state,
					result: {
						id: toolCall.id,
						success: Bool.true,
						output: "{\"status\": \"stream_active\", \"sent_count\": ${Num.to_str(state.sent)}}",
					},
				})

			_ =>
				Ok({
					state,
					result: {
						id: toolCall.id,
						success: Bool.false,
						output: "{\"error\": \"Unknown tool ${toolCall.name}\"}",
					},
				})
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
						required: Bool.true,
					},
				],
			},
		],
	},

	serializeState: |state| {
		connStr = if state.connected "true" else "false"
		"{\"connected\":${connStr},\"handle\":${Num.to_str(state.handle)},\"sent\":${Num.to_str(state.sent)},\"received\":${Num.to_str(state.received)}}"
	},

	deserializeState: |stateJson| {
		connected =
			match Str.split_first(stateJson, "\"connected\":true") {
				Ok(_) => Bool.true
				Err(_) => Bool.false
			}
		handle =
			match Str.split_first(stateJson, "\"handle\":") {
				Ok({ after }) =>
					match Str.split_first(after, ",") {
						Ok({ before }) => Str.to_u32(Str.trim(before)) ?? 0
						Err(_) => 0
					}

				Err(_) => 0
			}
		sent =
			match Str.split_first(stateJson, "\"sent\":") {
				Ok({ after }) =>
					match Str.split_first(after, ",") {
						Ok({ before }) => Str.to_u64(Str.trim(before)) ?? 0
						Err(_) => 0
					}

				Err(_) => 0
			}
		received =
			match Str.split_first(stateJson, "\"received\":") {
				Ok({ after }) => {
					cleaned = Str.trim(after)
					match Str.split_first(cleaned, "}") {
						Ok({ before }) => Str.to_u64(Str.trim(before)) ?? 0
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
