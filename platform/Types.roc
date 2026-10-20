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
		httpMount : HttpMountOption,
	}
	# An HTTP mount publishes the agent type on its deployment's domain. `None`
	# keeps the agent reachable only through the CLI/RPC surfaces.
	HttpMountOption : [None, Mount(HttpMount)]
	# `pathPrefix` segments are URL literals; a segment written as `{name}`
	# captures a value, and that name is the agent's constructor (identity)
	# parameter, so the URL selects which instance serves the request. Golem
	# requires every constructor parameter to be captured exactly once.
	# `filesystemBindings` map public paths onto files in the agent's own
	# durable filesystem; the runtime serves matching GET/HEAD requests without
	# invoking the agent.
	HttpMount : {
		pathPrefix : List(Str),
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
	# Extra headers added to responses served from `filesystemBindings`. Golem
	# rejects headers it owns (content-length, cache-control, etag, ...).
	FileResponseHeader : { name : Str, value : Str }
	ToolCall : { id : Str, name : Str, arguments : Str }
	ToolResult : { id : Str, success : Bool, output : Str }
	# A config key the agent reads with `getConfig!`. `secret` marks it as coming
	# from the environment's secret store instead of the manifest's local config.
	ConfigDeclaration : { name : Str, secret : Bool }
	PersistenceMode : [PersistNothing, PersistStateOnly, PersistEverything]
	LogLevel : [Trace, Debug, Info, Warn, Error]
}
