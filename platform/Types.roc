module [
	Result,
	Metadata,
	MethodDeclaration,
	AgentMode,
	ToolDefinition,
	ToolParameter,
	ToolCall,
	ToolResult,
	PersistenceMode,
	LogLevel,
	SecretHandle,
	SecretHeader,
	HttpMount,
]

## Compatibility alias for Try in modern Roc
Result(ok, err) : Try(ok, err)

## Types representing Golem Agent Metadata, Methods, and Tool specifications

SecretHandle : U32

SecretHeader : {
	name : Str,
	prefix : Str,
	handle : SecretHandle,
}

AgentMode : [
	Durable,
	Ephemeral,
]

HttpMount : {
	pathPrefix : Str,
	corsOrigins : List(Str),
	webhookSuffix : Str,
}

MethodDeclaration : {
	name : Str,
	description : Str,
	promptHint : Str,
	readOnly : Bool,
}

ToolParameter : {
	name : Str,
	description : Str,
	paramType : Str,
	required : Bool,
}

ToolDefinition : {
	name : Str,
	description : Str,
	parameters : List(ToolParameter),
}

Metadata : {
	name : Str,
	version : Str,
	description : Str,
	tools : List(ToolDefinition),
	methods : List(MethodDeclaration),
	mode : AgentMode,
	httpMount : [None, Some(HttpMount)],
}

ToolCall : {
	id : Str,
	name : Str,
	arguments : Str,
}

ToolResult : {
	id : Str,
	success : Bool,
	output : Str,
}

PersistenceMode : [
	PersistNothing,
	PersistStateOnly,
	PersistEverything,
]

LogLevel : [
	Trace,
	Debug,
	Info,
	Warn,
	Error,
]
