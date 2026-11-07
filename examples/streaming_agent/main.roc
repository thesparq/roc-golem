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
			connected: False,
			handle: 0.U32,
			sent: 0.U64,
			received: 0.U64,
		}),

	handleMessage: |state, message|
		match message {
			"connect" =>
				Ok({
					state: { ..state, connected: True, handle: 1.U32 },
					response: "WebSocket connected (handle=1)",
				})

			"status" => {
				statusStr = if state.connected "connected" else "disconnected"
				Ok({
					state,
					response: "WebSocket is ${statusStr}. Sent ${Str.inspect(state.sent)}, Received ${Str.inspect(state.received)}",
				})
			}

			_ =>
				if state.connected {
					Ok({
						state: { ..state, sent: state.sent + 1.U64 },
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
						success: True,
						output: "{\"status\": \"stream_active\", \"sent_count\": ${Str.inspect(state.sent)}}",
					},
				})

			_ =>
				Ok({
					state,
					result: {
						id: toolCall.id,
						success: False,
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
						required: True,
					},
				],
			},
		],
	},

	serializeState: |state| {
		connStr = if state.connected "true" else "false"
		"{\"connected\":${connStr},\"handle\":${Str.inspect(state.handle)},\"sent\":${Str.inspect(state.sent)},\"received\":${Str.inspect(state.received)}}"
	},

	deserializeState: |stateJson| {
		connected =
			match Str.split_first(stateJson, "\"connected\":true") {
				Ok(_) => True
				Err(_) => False
			}
		handle : U32
		handle =
			match Str.split_first(stateJson, "\"handle\":") {
				Ok({ after, .. }) =>
					match Str.split_first(after, ",") {
						Ok({ before, .. }) => U32.from_str(Str.trim(before)) ?? 0.U32
						Err(_) => 0.U32
					}

				Err(_) => 0.U32
			}
		sent : U64
		sent =
			match Str.split_first(stateJson, "\"sent\":") {
				Ok({ after, .. }) =>
					match Str.split_first(after, ",") {
						Ok({ before, .. }) => U64.from_str(Str.trim(before)) ?? 0.U64
						Err(_) => 0.U64
					}

				Err(_) => 0.U64
			}
		received : U64
		received =
			match Str.split_first(stateJson, "\"received\":") {
				Ok({ after, .. }) => {
					cleaned = Str.trim(after)
					match Str.split_first(cleaned, "}") {
						Ok({ before, .. }) => U64.from_str(Str.trim(before)) ?? 0.U64
						Err(_) => 0.U64
					}
				}

				Err(_) => 0.U64
			}
		Ok({
			connected: connected,
			handle: handle,
			sent: sent,
			received: received,
		})
	},
})
