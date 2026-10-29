app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, defineAgent]
import pf.Golem exposing [logInfo!, websocketClose!, websocketConnect!, websocketReceive!, websocketSend!]

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
				statusStr = if (state.connected) "connected" else "disconnected"
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

	handleRequest!: |state, _request|
		{
			state,
			response: { status: 404, headers: [], body: "this agent is not an HTTP router" },
		},

	# The multimodal method's handler. This app declares `multimodal: NoMultimodal`,
	# so it registers no such method and this is never called; an app that wants
	# multimodal content declares `HandlesMultimodal` instead and answers with the
	# envelope it was handed (see the README's "Multimodal values").
	handleMultimodal!: |state, _parts|
		{
			state,
			parts: "[{\"tag\":\"text\",\"value\":{\"text\":\"this agent does not handle multimodal input\",\"language\":null}}]",
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
						stdout: "",
						stderr: "",
					},
				}

			_ =>
				{
					state,
					result: {
						id: toolCall.id,
						success: False,
						output: "{\"error\": \"Unknown tool ${toolCall.name}\"}",
						stdout: "",
						stderr: "",
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
				readOnly: False,
			},
		],
		methods: [],
		config: [],
		agentKind: Regular,
		httpMount: None,
		httpHandler: NoHandler,
		multimodal: NoMultimodal,
		openapiProvider: NoOpenApi,
	},

	serializeState: |state|
		Json.to_str({
			connected: state.connected,
			handle: state.handle,
			sent: state.sent,
			received: state.received,
		}),

	deserializeState: |stateJson| {
		parsed : Try(
			{
				connected : Bool,
				handle : U32,
				sent : U64,
				received : U64,
			},
			[InvalidJson(Str), MissingRequiredField(Str)],
		)
		parsed = Json.parse(stateJson)
		match parsed {
			Ok(fields) => Ok({
				connected: fields.connected,
				handle: fields.handle,
				sent: fields.sent,
				received: fields.received,
			})
			Err(_) => Err("Invalid JSON format")
		}
	},
})
