app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, defineAgent]

# A file-site agent: it publishes files from its own durable filesystem over
# HTTP. `golem.yaml` provisions `/assets/hello.txt` and `/site/index.html` into
# the agent's filesystem, and the mount below serves them from
# `http://<deployment-domain>/site/<id>/...` without invoking the agent.
#
# The `{id}` capture in the mount path is the agent's constructor parameter, so
# the URL picks which instance's filesystem is served. That is why the mount
# lives in `metadata`: it is part of the agent type's public surface.

State : {
	served : I64,
}

agent : Agent(State)
agent = defineAgent({
	init!: |_config|
		{ served: 0 },

	handleMessage!: |state, message|
		match message {
			"ping" =>
				{
					state,
					replies: ["file-site agent is serving its mount"],
				}

			_ => {
				nextServed = state.served + 1
				{
					state: { served: nextServed },
					replies: [
						"file-site agent received '${message}' (message ${I64.to_str(nextServed)})",
					],
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
		{
			state,
			result: {
				id: toolCall.id,
				success: False,
				output: "file-site agent exposes no tools",
				stdout: "",
				stderr: "",
			},
		},

	metadata: {
		name: "file-site-agent",
		version: "1.0.0",
		description: "Serves files from its durable Golem filesystem over HTTP",
		tools: [],
		methods: [],
		config: [],
		agentKind: Regular,
		httpMount: Mount({
			pathPrefix: ["site", "{id}"],
			staticBindings: [],
			filesystemBindings: [
				Subtree({ publicPrefix: ["assets"], filesystemRoot: "/assets" }),
				Exact({ publicPath: ["index.html"], filePath: "/site/index.html" }),
			],
			fileResponseHeaders: [{ name: "referrer-policy", value: "no-referrer" }],
			corsAllowedPatterns: ["*"],
		}),
		httpHandler: NoHandler,
		multimodal: NoMultimodal,
		openapiProvider: NoOpenApi,
	},

	serializeState: |state|
		Json.to_str({ served: state.served }),

	deserializeState: |stateJson| {
		parsed : Try({ served : I64 }, [InvalidJson(Str), MissingRequiredField(Str)])
		parsed = Json.parse(stateJson)
		match parsed {
			Ok(fields) => Ok({ served: fields.served })
			Err(_) => Err("Invalid JSON format")
		}
	},
})
