app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, defineAgent]

# A static-site router. Golem serves the uploaded files and the OpenAPI document
# itself; everything else is handed to `handleRequest!`.
#
# `staticBindings` serve files that `golem deploy` uploads from the manifest's
# `files:` list for this agent type — they are content-addressed, so responses
# carry an immutable ETag. `openapiProvider` is the document Golem merges into
# `/openapi.yaml` on the deployment's domain. `httpHandler: HandlesRequests`
# registers the dynamic handler, so paths the bindings do not match are routed
# to this app instead of answering 404. Because routers are parameterless and
# ephemeral, the mount path is literal-only and there is no filesystem to bind.

State : {}

agent : Agent(State)
agent = defineAgent({
	init!: |_config|
		{},

	# Never called: a router registers only its OpenAPI provider method and, when
	# `httpHandler: HandlesRequests`, the handler below.
	handleMessage!: |state, _message| { state, replies: [] },

	# Every request the static bindings do not match lands here, with the body
	# already buffered by the host. A router owns its statuses: 204 for the
	# bodyless case, 404 for a path this app does not serve, and a computed
	# answer for everything else.
	handleRequest!: |state, request|
		if request.path == "/static/ping" {
			# No body: the host skips the response stream write entirely.
			{ state, response: { status: 204, headers: [], body: "" } }
		} else if request.path == "/static/missing" {
			{ state, response: { status: 404, headers: [], body: "" } }
		} else {
			{
				state,
				response: {
					status: 200,
					headers: [{ name: "content-type", value: "text/plain" }],
					body: "roc-golem router: ${request.method} ${request.path} body='${request.body}'",
				},
			}
		},

	# The multimodal method's handler. A router cannot declare it: its registered
	# methods are the request handler above and the OpenAPI provider, and the host
	# refuses `HandlesMultimodal` here. The stub is still part of the agent record.
	handleMultimodal!: |state, _parts|
		{
			state,
			parts: "[{\"tag\":\"text\",\"value\":{\"text\":\"a router registers no multimodal method\",\"language\":null}}]",
		},

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
		httpHandler: HandlesRequests,
		multimodal: NoMultimodal,
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
