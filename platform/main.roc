platform "golem"
	requires {
		agent : _
	}
	exposes [
		Golem,
		Types,
	]
	packages {}
	provides {
		"main_init_for_host_1_exposed_generic": main_init_for_host!,
		"main_handle_message_for_host_1_exposed_generic": main_handle_message_for_host!,
		"main_handle_tool_call_for_host_1_exposed_generic": main_handle_tool_call_for_host!,
		"main_metadata_for_host_1_exposed_generic": main_metadata_for_host,
	}
	hosted {
		"rocFxLog": Host.rocFxLog!,
		"rocFxGetWorkerId": Host.rocFxGetWorkerId!,
		"rocFxRpcInvoke": Host.rocFxRpcInvoke!,
		"rocFxSetPersistence": Host.rocFxSetPersistence!,
		"rocFxHttpRequest": Host.rocFxHttpRequest!,
		"rocFxWsConnect": Host.rocFxWsConnect!,
		"rocFxWsSend": Host.rocFxWsSend!,
		"rocFxWsReceive": Host.rocFxWsReceive!,
		"rocFxWsClose": Host.rocFxWsClose!,
		"rocFxSleepMillis": Host.rocFxSleepMillis!,
		"rocFxNowMillis": Host.rocFxNowMillis!,
	}
	targets: {
		wasm32: {
			inputs: ["libhost.a", app],
			exports: ["golem:agent/guest@1.5.0#discover-agent-types", "cabi_post_golem:agent/guest@1.5.0#discover-agent-types", "golem:agent/guest@1.5.0#get-definition", "cabi_post_golem:agent/guest@1.5.0#get-definition", "golem:agent/guest@1.5.0#initialize", "cabi_post_golem:agent/guest@1.5.0#initialize", "golem:agent/guest@1.5.0#invoke", "cabi_post_golem:agent/guest@1.5.0#invoke", "cabi_realloc", "cabi_realloc_wit_bindgen_0_36_0"],
		},
	}

import Types exposing [
	ToolDefinition,
	ToolParameter,
	ToolCall,
]
import Golem
import Host

main_init_for_host! : Str => Try(Str, Str)
main_init_for_host! = |config_str| {
	initial_model = (agent.init!)(config_str)
	Ok((agent.serializeState)(initial_model))
}

main_handle_message_for_host! : Str, Str => Try(Str, Str)
main_handle_message_for_host! = |state_json, message_str| {
	match (agent.deserializeState)(state_json) {
		Err(err) =>
			Err("Failed to deserialize state: ${err}")

		Ok(current_model) => {
			res = (agent.handleMessage!)(current_model, message_str)
			next_state_json = (agent.serializeState)(res.state)

			# We'll just take the first reply for now, or join them
			joined_replies = Str.join_with(res.replies, "\n")
			escaped_response = escape_json_str(joined_replies)
			Ok("{\"state\":${next_state_json},\"response\":\"${escaped_response}\"}")
		}
	}
}

main_handle_tool_call_for_host! : Str, Str => Try(Str, Str)
main_handle_tool_call_for_host! = |state_json, tool_call_json| {
	match (agent.deserializeState)(state_json) {
		Err(err) =>
			Err("Failed to deserialize state: ${err}")

		Ok(current_model) => {
			parsed_call = parse_tool_call(tool_call_json)
			res = (agent.handleToolCall!)(current_model, parsed_call)
			next_state_json = (agent.serializeState)(res.state)
			success_str = match res.result.success {
				True => "true"
				False => "false"
			}
			escaped_output = escape_json_str(res.result.output)
			Ok("{\"state\":${next_state_json},\"success\":${success_str},\"output\":\"${escaped_output}\"}")
		}
	}
}

main_metadata_for_host : {} => Str
main_metadata_for_host = |_| {
	meta = agent.metadata

	formatted_tools = List.map(meta.tools, format_tool_definition)
	tools_json = Str.join_with(formatted_tools, ",")

	name_escaped = escape_json_str(meta.name)
	version_escaped = escape_json_str(meta.version)
	desc_escaped = escape_json_str(meta.description)

	"{\"name\":\"${name_escaped}\",\"version\":\"${version_escaped}\",\"description\":\"${desc_escaped}\",\"tools\":[${tools_json}]}"
}

format_tool_definition : ToolDefinition -> Str
format_tool_definition = |tool| {
	formatted_params = List.map(tool.parameters, format_tool_parameter)
	params_json = Str.join_with(formatted_params, ",")

	name_escaped = escape_json_str(tool.name)
	desc_escaped = escape_json_str(tool.description)
	"{\"name\":\"${name_escaped}\",\"description\":\"${desc_escaped}\",\"parameters\":[${params_json}]}"
}

format_tool_parameter : ToolParameter -> Str
format_tool_parameter = |param| {
	req_str = match param.required {
		True => "true"
		False => "false"
	}
	name_escaped = escape_json_str(param.name)
	desc_escaped = escape_json_str(param.description)
	type_escaped = escape_json_str(param.paramType)
	"{\"name\":\"${name_escaped}\",\"description\":\"${desc_escaped}\",\"type\":\"${type_escaped}\",\"required\":${req_str}}"
}

escape_json_str : Str -> Str
escape_json_str = |input| {
	s1 = Str.replace_each(input, "\\", "\\\\")
	s2 = Str.replace_each(s1, "\"", "\\\"")
	s3 = Str.replace_each(s2, "\n", "\\n")
	s4 = Str.replace_each(s3, "\r", "\\r")
	Str.replace_each(s4, "\t", "\\t")
}

parse_tool_call : Str -> ToolCall
parse_tool_call = |raw_json| {
	id = match extract_json_field(raw_json, "id") {
		Ok(v) => v
		Err(_) => "call-1"
	}
	name = match extract_json_field(raw_json, "name") {
		Ok(v) => v
		Err(_) => "default"
	}
	arguments = match extract_json_field(raw_json, "arguments") {
		Ok(v) => v
		Err(_) => "{}"
	}
	{ id, name, arguments }
}

extract_json_field : Str, Str -> Try(Str, [NotFound])
extract_json_field = |json, field_name| {
	target = "\"${field_name}\":\""
	match Str.split_first(json, target) {
		Ok({ before: _, after }) =>
			match Str.split_first(after, "\"") {
				Ok({ before, after: _ }) => Ok(before)
				Err(_) => Err(NotFound)
			}
		Err(_) => Err(NotFound)
	}
}
