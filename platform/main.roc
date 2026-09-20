platform "golem"
    requires { State } { agent : Golem.Agent State }
    exposes [
        Golem,
        Types,
    ]
    packages {}
    imports []
    provides [
        main_init_for_host,
        main_handle_message_for_host,
        main_handle_tool_call_for_host,
        main_metadata_for_host,
    ]

import Types exposing [
    Metadata,
    ToolDefinition,
    ToolParameter,
    ToolCall,
    ToolResult,
]
import Golem exposing [Agent]

## Entry point for agent initialization called by Rust Host
main_init_for_host : Str -> Result Str Str
main_init_for_host = |config_str|
    when agent.init config_str is
        Ok initial_model ->
            Ok (agent.serializeState initial_model)

        Err err ->
            Err err

## Entry point for handling a message called by Rust Host
main_handle_message_for_host : Str, Str -> Result Str Str
main_handle_message_for_host = |state_json, message_str|
    when agent.deserializeState state_json is
        Err err ->
            Err "Failed to deserialize state: ${err}"

        Ok current_model ->
            when agent.handleMessage current_model message_str is
                Ok { state: next_model, response } ->
                    next_state_json = agent.serializeState next_model
                    escaped_response = escape_json_str response
                    Ok "{\"state\":${next_state_json},\"response\":\"${escaped_response}\"}"

                Err err ->
                    Err err

## Entry point for handling a tool invocation called by Rust Host
main_handle_tool_call_for_host : Str, Str -> Result Str Str
main_handle_tool_call_for_host = |state_json, tool_call_json|
    when agent.deserializeState state_json is
        Err err ->
            Err "Failed to deserialize state: ${err}"

        Ok current_model ->
            parsed_call = parse_tool_call tool_call_json
            when agent.handleToolCall current_model parsed_call is
                Ok { state: next_model, result } ->
                    next_state_json = agent.serializeState next_model
                    success_str = if result.success then "true" else "false"
                    escaped_output = escape_json_str result.output
                    Ok "{\"state\":${next_state_json},\"success\":${success_str},\"output\":\"${escaped_output}\"}"

                Err err ->
                    Err err

## Expose metadata to Rust Host as a JSON string
main_metadata_for_host : Str
main_metadata_for_host =
    meta = agent.metadata
    tools_json =
        meta.tools
        |> List.map format_tool_definition
        |> Str.joinWith ","
    name_escaped = escape_json_str meta.name
    version_escaped = escape_json_str meta.version
    desc_escaped = escape_json_str meta.description
    "{\"name\":\"${name_escaped}\",\"version\":\"${version_escaped}\",\"description\":\"${desc_escaped}\",\"tools\":[${tools_json}]}"

format_tool_definition : ToolDefinition -> Str
format_tool_definition = |tool|
    params_json =
        tool.parameters
        |> List.map format_tool_parameter
        |> Str.joinWith ","
    name_escaped = escape_json_str tool.name
    desc_escaped = escape_json_str tool.description
    "{\"name\":\"${name_escaped}\",\"description\":\"${desc_escaped}\",\"parameters\":[${params_json}]}"

format_tool_parameter : ToolParameter -> Str
format_tool_parameter = |param|
    req_str = if param.required then "true" else "false"
    name_escaped = escape_json_str param.name
    desc_escaped = escape_json_str param.description
    type_escaped = escape_json_str param.paramType
    "{\"name\":\"${name_escaped}\",\"description\":\"${desc_escaped}\",\"type\":\"${type_escaped}\",\"required\":${req_str}}"

escape_json_str : Str -> Str
escape_json_str = |input|
    input
    |> Str.replaceEach "\\" "\\\\"
    |> Str.replaceEach "\"" "\\\""
    |> Str.replaceEach "\n" "\\n"
    |> Str.replaceEach "\r" "\\r"
    |> Str.replaceEach "\t" "\\t"

parse_tool_call : Str -> ToolCall
parse_tool_call = |raw_json| {
    id: extract_json_field raw_json "id" |> Result.withDefault "call-1",
    name: extract_json_field raw_json "name" |> Result.withDefault "default",
    arguments: extract_json_field raw_json "arguments" |> Result.withDefault "{}",
}

extract_json_field : Str, Str -> Result Str [NotFound]
extract_json_field = |json, field_name|
    target = "\"${field_name}\":\""
    when Str.splitFirst json target is
        Ok { after } ->
            when Str.splitFirst after "\"" is
                Ok { before } -> Ok before
                Err _ -> Err NotFound

        Err _ ->
            Err NotFound
