platform "golem"
	requires {
		[State : state] for agent : Golem.Agent(state)
	}
	exposes [
		Golem,
		Types,
	]
	packages {}
	provides {
		"roc__main_init_for_host_1_exposed_generic": main_init_for_host,
		"roc__main_handle_message_for_host_1_exposed_generic": main_handle_message_for_host,
		"roc__main_handle_tool_call_for_host_1_exposed_generic": main_handle_tool_call_for_host,
		"roc__main_metadata_for_host_1_exposed_generic": main_metadata_for_host,
	}

import Types exposing [
	ToolDefinition,
	ToolParameter,
	ToolCall,
	MethodDeclaration,
	AgentMode,
]
import Golem

## Entry point for agent initialization called by Rust Host
main_init_for_host : Str -> Result(Str, Str)
main_init_for_host = |envelope_or_config| {
	agent_type = extract_json_field(envelope_or_config, "agent_type") ?? ""
	config = extract_json_field(envelope_or_config, "config") ?? envelope_or_config
	match find_agent_by_name(agent.agents, agent_type) {
		Err(err) => Err(err),
		Ok(target_agent) => (target_agent.init)(config),
	}
}

## Entry point for handling a message or typed method call by Rust Host
main_handle_message_for_host : Str, Str -> Result(Str, Str)
main_handle_message_for_host = |state_json, envelope_or_msg| {
	agent_type = extract_json_field(envelope_or_msg, "agent_type") ?? ""
	method_name = extract_json_field(envelope_or_msg, "method") ?? ""
	input_str = extract_json_field(envelope_or_msg, "input") ?? envelope_or_msg

	match find_agent_by_name(agent.agents, agent_type) {
		Err(err) => Err(err),
		Ok(target_agent) =>
			if Str.is_empty(method_name) or method_name == "handle-message" or method_name == "message" {
				match (target_agent.handleMessage)(state_json, input_str) {
					Ok(res) => {
						escaped_response = escape_json_str(res.response)
						Ok("{\"state\":\"${res.nextStateJson}\",\"response\":\"${escaped_response}\"}")
					}
					Err(err) => Err(err),
				}
			} else {
				match (target_agent.invokeMethod)(method_name, state_json, input_str) {
					Ok(res) => {
						escaped_out = escape_json_str(res.out)
						Ok("{\"state\":\"${res.nextStateJson}\",\"response\":\"${escaped_out}\"}")
					}
					Err(err) => Err(err),
				}
			},
	}
}

## Entry point for handling a tool invocation called by Rust Host
main_handle_tool_call_for_host : Str, Str -> Result(Str, Str)
main_handle_tool_call_for_host = |state_json, tool_call_json| {
	agent_type = extract_json_field(tool_call_json, "agent_type") ?? ""
	parsed_call = parse_tool_call(tool_call_json)
	match find_agent_by_name(agent.agents, agent_type) {
		Err(err) => Err(err),
		Ok(target_agent) =>
			match (target_agent.handleToolCall)(state_json, parsed_call) {
				Ok(res) => {
					success_str = if res.result.success "true" else "false"
					escaped_output = escape_json_str(res.result.output)
					Ok("{\"state\":\"${res.nextStateJson}\",\"success\":${success_str},\"output\":\"${escaped_output}\"}")
				}
				Err(err) => Err(err),
			},
	}
}

## Expose metadata to Rust Host as a JSON string
main_metadata_for_host : Str
main_metadata_for_host = {
	agents_json =
		agent.agents
			|> List.map(format_agent_metadata)
			|> Str.join_with(",")
	"{\"agents\":[${agents_json}]}"
}

format_agent_metadata : Golem.ErasedAgent -> Str
format_agent_metadata = |erased| {
	meta = erased.metadata
	name_escaped = escape_json_str(meta.name)
	version_escaped = escape_json_str(meta.version)
	desc_escaped = escape_json_str(meta.description)
	mode_str =
		match meta.mode {
			Durable => "durable",
			Ephemeral => "ephemeral",
		}
	tools_json =
		meta.tools
			|> List.map(format_tool_definition)
			|> Str.join_with(",")
	methods_json =
		meta.methods
			|> List.map(format_method_declaration)
			|> Str.join_with(",")
	"{\"name\":\"${name_escaped}\",\"version\":\"${version_escaped}\",\"description\":\"${desc_escaped}\",\"mode\":\"${mode_str}\",\"methods\":[${methods_json}],\"tools\":[${tools_json}]}"
}

format_method_declaration : MethodDeclaration -> Str
format_method_declaration = |m| {
	name_escaped = escape_json_str(m.name)
	desc_escaped = escape_json_str(m.description)
	hint_escaped = escape_json_str(m.promptHint)
	ro_str = if m.readOnly "true" else "false"
	"{\"name\":\"${name_escaped}\",\"description\":\"${desc_escaped}\",\"prompt_hint\":\"${hint_escaped}\",\"read_only\":${ro_str}}"
}

format_tool_definition : ToolDefinition -> Str
format_tool_definition = |tool| {
	params_json =
		tool.parameters
			|> List.map(format_tool_parameter)
			|> Str.join_with(",")
	name_escaped = escape_json_str(tool.name)
	desc_escaped = escape_json_str(tool.description)
	"{\"name\":\"${name_escaped}\",\"description\":\"${desc_escaped}\",\"parameters\":[${params_json}]}"
}

format_tool_parameter : ToolParameter -> Str
format_tool_parameter = |param| {
	req_str = if param.required "true" else "false"
	name_escaped = escape_json_str(param.name)
	desc_escaped = escape_json_str(param.description)
	type_escaped = escape_json_str(param.paramType)
	"{\"name\":\"${name_escaped}\",\"description\":\"${desc_escaped}\",\"type\":\"${type_escaped}\",\"required\":${req_str}}"
}

escape_json_str : Str -> Str
escape_json_str = |input|
	input
		|> Str.replace_each("\\", "\\\\")
		|> Str.replace_each("\"", "\\\"")
		|> Str.replace_each("\n", "\\n")
		|> Str.replace_each("\r", "\\r")
		|> Str.replace_each("\t", "\\t")

parse_tool_call : Str -> ToolCall
parse_tool_call = |raw_json| {
	id: extract_json_field(raw_json, "id") ?? "call-1",
	name: extract_json_field(raw_json, "name") ?? "default",
	arguments: extract_json_field(raw_json, "arguments") ?? "{}",
}

extract_json_field : Str, Str -> Result(Str, [NotFound])
extract_json_field = |json, field_name| {
	target = "\"${field_name}\":\""
	match Str.split_first(json, target) {
		Ok({ after }) =>
			match Str.split_first(after, "\"") {
				Ok({ before }) => Ok(before),
				Err(_) => Err(NotFound),
			}

		Err(_) =>
			Err(NotFound),
		}
}

find_agent_by_name : List(Golem.ErasedAgent), Str -> Result(Golem.ErasedAgent, Str)
find_agent_by_name = |agents, target_name|
	if Str.is_empty(target_name) {
		match agents {
			[] => Err("No agents defined in component"),
			[first, ..] => Ok(first),
		}
	} else {
		match agents {
			[] => Err("Agent type '${target_name}' not found"),
			[first, .. as rest] =>
				if first.metadata.name == target_name {
					Ok(first)
				} else {
					find_agent_by_name(rest, target_name)
				},
		}
	}
