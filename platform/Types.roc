module [
    Metadata,
    ToolDefinition,
    ToolParameter,
    ToolCall,
    ToolResult,
    PersistenceMode,
    LogLevel,
]

## Types representing Golem Agent Metadata and Tool specifications

ToolParameter : {
    name : Str,
    description : Str,
    paramType : Str,
    required : Bool,
}

ToolDefinition : {
    name : Str,
    description : Str,
    parameters : List ToolParameter,
}

Metadata : {
    name : Str,
    version : Str,
    description : Str,
    tools : List ToolDefinition,
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
