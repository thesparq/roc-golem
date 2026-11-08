app [agent] { pf: platform "../../platform/main.roc" }

import pf.Json
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
						output: Json.encode({ status: "stream_active", sent_count: state.sent }),
					},
				})

			_ =>
				Ok({
					state,
					result: {
						id: toolCall.id,
						success: False,
						output: Json.encode({ error: "Unknown tool ${toolCall.name}" }),
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

	serializeState: |state|
		Json.encode(state),

	deserializeState: |stateJson| {
		defaultState : State
		defaultState = {
			connected: False,
			handle: 0.U32,
			sent: 0.U64,
			received: 0.U64,
		}
		res : State
		res = Json.decode_or(stateJson, defaultState)
		Ok(res)
	},
})
