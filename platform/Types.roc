## Types representing Golem Agent Metadata and Tool specifications

Types := {}.{
	ToolParameter : { name : Str, description : Str, paramType : Str, required : Bool }
	# `readOnly` maps to the 1.6 read-only method flag: the runtime may cache the
	# result and traps writes made from the call.
	ToolDefinition : { name : Str, description : Str, parameters : List(ToolParameter), readOnly : Bool }
	Metadata : {
		name : Str,
		version : Str,
		description : Str,
		tools : List(ToolDefinition),
		config : List(ConfigDeclaration),
		agentKind : AgentKind,
		httpMount : HttpMountOption,
		httpHandler : HttpHandlerOption,
		openapiProvider : OpenApiProvider,
	}
	# `HttpRouter` agent types are served by Golem itself: they are parameterless,
	# ephemeral, have no durable filesystem and register only their OpenAPI
	# provider method (and their request handler, when declared). Use `Regular`
	# for an agent with methods, state and (optionally) a filesystem mount.
	AgentKind : [Regular, HttpRouter]
	# `HandlesRequests` registers the router's `handle` method, so every request
	# its static bindings do not match is handed to `handleRequest!`. `NoHandler`
	# keeps a router purely static (and lets unmatched paths answer 404 without
	# waking anything).
	HttpHandlerOption : [NoHandler, HandlesRequests]
	# One request routed to a router's mount. `headersJson` is the raw header list
	# (`[{"name":...,"value":...}]`, in request order) and `query` is empty when
	# the URL had none.
	HttpRequest : {
		method : Str,
		scheme : Str,
		authority : Str,
		path : Str,
		query : Str,
		headersJson : Str,
		body : Str,
	}
	HttpHeader : { name : Str, value : Str }
	# What `handleRequest!` answers with: a status, response headers and a body.
	HttpResponse : { status : U16, headers : List(HttpHeader), body : Str }
	# The OpenAPI document a router publishes at the deployment's
	# `/openapi.yaml`. `NoOpenApi` leaves the route to Golem's generated spec.
	OpenApiProvider : [NoOpenApi, OpenApi(Str)]
	# An HTTP mount publishes the agent type on its deployment's domain. `None`
	# keeps the agent reachable only through the CLI/RPC surfaces.
	HttpMountOption : [None, Mount(HttpMount)]
	# `pathPrefix` segments are URL literals; a segment written as `{name}`
	# captures a value, and that name is the agent's constructor (identity)
	# parameter, so the URL selects which instance serves the request. Golem
	# requires every constructor parameter to be captured exactly once, and
	# router mounts must be literal-only (they have no constructor).
	#
	# `filesystemBindings` (regular agents) map public paths onto files in the
	# agent's own durable filesystem. `staticBindings` (routers) map them onto
	# files uploaded at deploy time from the manifest's `files:` list, which the
	# runtime serves itself with an immutable ETag. Either way matching GET/HEAD
	# requests never invoke the agent.
	HttpMount : {
		pathPrefix : List(Str),
		staticBindings : List(FileBinding),
		filesystemBindings : List(FileBinding),
		fileResponseHeaders : List(FileResponseHeader),
		corsAllowedPatterns : List(Str),
	}
	# `Exact` serves one public path from one file. `Subtree` serves every path
	# under a public prefix from a filesystem root, preserving the rest of the
	# path; the root is a directory inside the agent's filesystem.
	FileBinding : [
		Exact({ publicPath : List(Str), filePath : Str }),
		Subtree({ publicPrefix : List(Str), filesystemRoot : Str }),
	]
	# Extra headers added to responses served from a file binding. Golem
	# rejects headers it owns (content-length, cache-control, etag, ...).
	FileResponseHeader : { name : Str, value : Str }
	# A tool invocation. `arguments` is the single string input the platform
	# declares for every tool; `stdin` is the text the caller piped into the tool
	# (`golem tool invoke --stdin …`), empty when the caller attached nothing.
	ToolCall : { id : Str, name : Str, arguments : Str, stdin : Str }
	# The app's answer. `stdout` and `stderr` are the text the tool writes on the
	# streams the caller attached (`golem tool invoke --stdout` / `--stderr`); an
	# empty string writes nothing. Both are written after `handleToolCall!`
	# returns, so a tool cannot emit output while it runs yet (see the README).
	ToolResult : { id : Str, success : Bool, output : Str, stdout : Str, stderr : Str }
	# A config key the agent reads with `getConfig!`. `secret` marks it as coming
	# from the environment's secret store instead of the manifest's local config.
	ConfigDeclaration : { name : Str, secret : Bool }
	PersistenceMode : [PersistNothing, PersistStateOnly, PersistEverything]
	LogLevel : [Trace, Debug, Info, Warn, Error]
}
