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
                    # Format as JSON payload for host
                    Ok
                        """
                        {"state":${next_state_json},"response":"${response}"}
                        """

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
                    Ok
                        """
                        {"state":${next_state_json},"success":${success_str},"output":"${result.output}"}
                        """

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
    """
    {"name":"${meta.name}","version":"${meta.version}","description":"${meta.description}","tools":[${tools_json}]}
    """

format_tool_definition : ToolDefinition -> Str
format_tool_definition = |tool|
    params_json =
        tool.parameters
        |> List.map format_tool_parameter
        |> Str.joinWith ","
    """
    {"name":"${tool.name}","description":"${tool.description}","parameters":[${params_json}]}
    """

format_tool_parameter : ToolParameter -> Str
format_tool_parameter = |param|
    req_str = if param.required then "true" else "false"
    """
    {"name":"${param.name}","description":"${param.description}","type":"${param.paramType}","required":${req_str}}
    """

parse_tool_call : Str -> ToolCall
parse_tool_call = |_raw_json| {
    id: "call-1",
    name: "default",
    arguments: "{}",
}
