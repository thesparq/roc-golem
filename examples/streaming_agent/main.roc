app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, defineAgent]
import pf.Golem exposing [logInfo!, websocketConnect!, websocketSend!, websocketReceive!, websocketClose!]

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
				match websocketConnect!("wss://echo.websocket.events") {
					Ok(handle) => {
						logInfo!("websocket connected, handle=${U32.to_str(handle)}")
						{
							state: { ..state, connected: True, handle },
							replies: ["Connected with handle ${U32.to_str(handle)}"],
						}
					}

					Err(err) =>
						{
							state,
							replies: ["Connect failed: ${err}"],
						}
					}

			"receive" =>
				if state.connected {
					match websocketReceive!(state.handle) {
						Ok(text) =>
							{
								state: { ..state, received: state.received + 1 },
								replies: ["Received: ${text}"],
							}

						Err(err) =>
							{
								state,
								replies: ["Receive failed: ${err}"],
							}
						}
				} else {
					{
						state,
						replies: ["Not connected. Send 'connect' first."],
					}
				}

			"close" =>
				if state.connected {
					match websocketClose!(state.handle) {
						Ok(_) =>
							{
								state: { ..state, connected: False, handle: 0 },
								replies: ["Connection closed"],
							}

						Err(err) =>
							{
								state,
								replies: ["Close failed: ${err}"],
							}
						}
				} else {
					{
						state,
						replies: ["Not connected. Send 'connect' first."],
					}
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
					match websocketSend!(state.handle, message) {
						Ok(_) =>
							{
								state: { ..state, sent: state.sent + 1 },
								replies: ["Sent: ${message}"],
							}

						Err(err) =>
							{
								state,
								replies: ["Send failed: ${err}"],
							}
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
