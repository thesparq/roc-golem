## Types representing Golem Agent Metadata and Tool specifications

Types := {}.{
	ToolParameter : { name : Str, description : Str, paramType : Str, required : Bool }
	# `readOnly` maps to the 1.6 read-only method flag: the runtime may cache the
	# result and traps writes made from the call.
	ToolDefinition : { name : Str, description : Str, parameters : List(ToolParameter), readOnly : Bool }
	Metadata : { name : Str, version : Str, description : Str, tools : List(ToolDefinition) }
	ToolCall : { id : Str, name : Str, arguments : Str }
	ToolResult : { id : Str, success : Bool, output : Str }
	PersistenceMode : [PersistNothing, PersistStateOnly, PersistEverything]
	LogLevel : [Trace, Debug, Info, Warn, Error]
}
