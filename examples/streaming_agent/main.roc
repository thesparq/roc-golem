app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, agent, reply, readOnly, toolJson, param]

# Agent State
State : {
	connected : Bool,
	handle : U32,
	sent : U64,
	received : U64,
}

streamMessageTool = toolJson({
	name: "stream_message",
	description: "Streams message via WebSocket",
	params: [param("content", "Message to stream")],
	execute: |state, _args|
		Ok({ status: "stream_active", sent_count: state.sent }),
})

agent : Agent(State)
agent = Golem.agent({
	name: "streaming-agent",
	initial: {
		connected: False,
		handle: 0.U32,
		sent: 0.U64,
		received: 0.U64,
	},
	tools: [streamMessageTool],
	onMessage: |state, message|
		match message {
			"connect" =>
			# NOTE: Simplified example. In an effectful method/workflow, use Golem.websocketConnect.
				reply(
					{ ..state, connected: True, handle: 1.U32 },
					"WebSocket connected (handle=1)",
				)

			"status" => {
				statusStr = if state.connected "connected" else "disconnected"
				readOnly(
					state,
					"WebSocket is ${statusStr}. Sent ${Str.inspect(state.sent)}, Received ${Str.inspect(state.received)}",
				)
			}

			_ =>
				if state.connected {
					reply(
						{ ..state, sent: state.sent + 1.U64 },
						"Queued message for WebSocket: ${message}",
					)
				} else {
					readOnly(state, "Not connected. Send 'connect' first.")
				}
		},
})
