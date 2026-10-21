app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, defineAgent]

# A static-site router: Golem serves everything itself, so no request ever
# reaches this app.
#
# `staticBindings` serve files that `golem deploy` uploads from the manifest's
# `files:` list for this agent type — they are content-addressed, so responses
# carry an immutable ETag. `openapiProvider` is the document Golem returns from
# `/openapi.yaml` on the deployment's domain, and it is this agent type's only
# method. Because routers are parameterless and ephemeral, the mount path is
# literal-only and there is no filesystem to bind.

State : {}

agent : Agent(State)
agent = defineAgent({
	init!: |_config|
		{},

	# Never called: a router registers only its OpenAPI provider method.
	handleMessage!: |state, _message|
		{ state, replies: [] },

	handleToolCall!: |state, toolCall|
		{
			state,
			result: {
				id: toolCall.id,
				success: False,
				output: "static-site-agent exposes no tools",
				stdout: "",
				stderr: "",
			},
		},

	metadata: {
		name: "static-site-agent",
		version: "1.0.0",
		description: "Serves an uploaded static site and an OpenAPI document from a router mount",
		tools: [],
		config: [],
		agentKind: HttpRouter,
		httpMount: Mount({
			pathPrefix: ["static"],
			staticBindings: [
				Subtree({ publicPrefix: ["assets"], filesystemRoot: "/assets" }),
				Exact({ publicPath: ["index.html"], filePath: "/index.html" }),
			],
			filesystemBindings: [],
			fileResponseHeaders: [{ name: "referrer-policy", value: "no-referrer" }],
			corsAllowedPatterns: ["*"],
		}),
		# The provider's paths are relative to the mount prefix: `/index.html`
		# here is `/static/index.html` on the deployment's domain.
		openapiProvider: OpenApi(
			"{\"openapi\":\"3.1.0\",\"info\":{\"title\":\"roc-golem static site\",\"version\":\"1.0.0\"},\"paths\":{\"/index.html\":{\"get\":{\"summary\":\"The uploaded index page\",\"responses\":{\"200\":{\"description\":\"The static index\",\"content\":{\"text/html\":{}}}}}}}}",
		),
	},

	serializeState: |_state|
		"{}",

	deserializeState: |_stateJson|
		Ok({}),
})
