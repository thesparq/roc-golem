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
	}
	ToolCall : { id : Str, name : Str, arguments : Str }
	ToolResult : { id : Str, success : Bool, output : Str }
	# A config key the agent reads with `getConfig!`. `secret` marks it as coming
	# from the environment's secret store instead of the manifest's local config.
	ConfigDeclaration : { name : Str, secret : Bool }
	PersistenceMode : [PersistNothing, PersistStateOnly, PersistEverything]
	LogLevel : [Trace, Debug, Info, Warn, Error]
}
