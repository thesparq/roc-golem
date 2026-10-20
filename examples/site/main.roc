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

	handleToolCall!: |state, toolCall|
		{
			state,
			result: {
				id: toolCall.id,
				success: False,
				output: "file-site agent exposes no tools",
			},
		},

	metadata: {
		name: "file-site-agent",
		version: "1.0.0",
		description: "Serves files from its durable Golem filesystem over HTTP",
		tools: [],
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
		openapiProvider: NoOpenApi,
	},

	serializeState: |state|
		"{\"served\":${I64.to_str(state.served)}}",

	deserializeState: |stateJson|
		match Str.split_first(stateJson, "\"served\":") {
			Ok({ before: _, after }) =>
				match Str.split_first(after, "}") {
					Ok({ before, after: _ }) =>
						match I64.from_str(before) {
							Ok(served) => Ok({
								served: served,
							})
							Err(_) => Err("Failed to parse served integer")
						}

					Err(_) => Err("Invalid JSON format")
				}

			Err(_) => Err("Missing served field")
		},
})
