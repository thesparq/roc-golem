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
		"main_handle_message_for_host_1_exposed_generic": main_handle_message_for_host!,
		"main_handle_tool_call_for_host_1_exposed_generic": main_handle_tool_call_for_host!,
		"main_init_for_host_1_exposed_generic": main_init_for_host!,
		"main_metadata_for_host_1_exposed_generic": main_metadata_for_host,
	}
	hosted {
		"rocFxAwaitWebhook": Host.rocFxAwaitWebhook!,
		"rocFxCreateWebhook": Host.rocFxCreateWebhook!,
		"rocFxGetConfig": Host.rocFxGetConfig!,
		"rocFxGetRetryPolicies": Host.rocFxGetRetryPolicies!,
		"rocFxGetRetryPolicy": Host.rocFxGetRetryPolicy!,
		"rocFxGetWorkerId": Host.rocFxGetWorkerId!,
		"rocFxHttpRequest": Host.rocFxHttpRequest!,
		"rocFxLog": Host.rocFxLog!,
		"rocFxMakeAgentId": Host.rocFxMakeAgentId!,
		"rocFxNewQuotaToken": Host.rocFxNewQuotaToken!,
		"rocFxNextRetryDelay": Host.rocFxNextRetryDelay!,
		"rocFxNowMillis": Host.rocFxNowMillis!,
		"rocFxParseAgentId": Host.rocFxParseAgentId!,
		"rocFxQuotaCommit": Host.rocFxQuotaCommit!,
		"rocFxQuotaMerge": Host.rocFxQuotaMerge!,
		"rocFxQuotaReserve": Host.rocFxQuotaReserve!,
		"rocFxQuotaSplit": Host.rocFxQuotaSplit!,
		"rocFxRemoveRetryPolicy": Host.rocFxRemoveRetryPolicy!,
		"rocFxResolveRetryPolicy": Host.rocFxResolveRetryPolicy!,
		"rocFxRpcCancel": Host.rocFxRpcCancel!,
		"rocFxRpcFireAndForget": Host.rocFxRpcFireAndForget!,
		"rocFxRpcInvoke": Host.rocFxRpcInvoke!,
		"rocFxRpcSchedule": Host.rocFxRpcSchedule!,
		"rocFxRpcScheduleCancelable": Host.rocFxRpcScheduleCancelable!,
		"rocFxSetPersistence": Host.rocFxSetPersistence!,
		"rocFxSetRetryPolicy": Host.rocFxSetRetryPolicy!,
		"rocFxSleepMillis": Host.rocFxSleepMillis!,
		"rocFxWsClose": Host.rocFxWsClose!,
		"rocFxWsConnect": Host.rocFxWsConnect!,
		"rocFxWsReceive": Host.rocFxWsReceive!,
		"rocFxWsSend": Host.rocFxWsSend!,
	}
	targets: {
		wasm32: {
			inputs: ["libhost.a", app],
			exports: ["[async-lift]golem:agent/guest@2.0.0#initialize", "[callback][async-lift]golem:agent/guest@2.0.0#initialize", "[async-lift]golem:agent/guest@2.0.0#invoke", "[callback][async-lift]golem:agent/guest@2.0.0#invoke", "golem:agent/guest@2.0.0#get-definition", "cabi_post_golem:agent/guest@2.0.0#get-definition", "golem:agent/guest@2.0.0#discover-agent-types", "cabi_post_golem:agent/guest@2.0.0#discover-agent-types", "[async-lift]golem:tool/guest@0.1.0#invoke", "[callback][async-lift]golem:tool/guest@0.1.0#invoke", "golem:tool/guest@0.1.0#discover-tools", "cabi_post_golem:tool/guest@0.1.0#discover-tools", "golem:tool/guest@0.1.0#get-tool", "cabi_post_golem:tool/guest@0.1.0#get-tool", "[async-lift]golem:api/save-snapshot@1.5.0#save", "[callback][async-lift]golem:api/save-snapshot@1.5.0#save", "[async-lift]golem:api/load-snapshot@1.5.0#load", "[callback][async-lift]golem:api/load-snapshot@1.5.0#load", "cabi_realloc"],
		},
	}

import Types exposing [
	AgentKind,
	ConfigDeclaration,
	FileBinding,
	FileResponseHeader,
	HttpMountOption,
	OpenApiProvider,
	ToolCall,
	ToolDefinition,
	ToolParameter,
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

	formatted_config = List.map(meta.config, format_config_declaration)
	config_json = Str.join_with(formatted_config, ",")

	mount_json = format_http_mount(meta.httpMount)
	kind_json = format_agent_kind(meta.agentKind)
	openapi_json = format_openapi_provider(meta.openapiProvider)

	"{\"name\":\"${name_escaped}\",\"version\":\"${version_escaped}\",\"description\":\"${desc_escaped}\",\"tools\":[${tools_json}],\"config\":[${config_json}],\"agentKind\":${kind_json},\"httpMount\":${mount_json},\"openapiProvider\":${openapi_json}}"
}

format_agent_kind : AgentKind -> Str
format_agent_kind = |kind|
	match kind {
		Regular => "\"regular\""
		HttpRouter => "\"http-router\""
	}

format_openapi_provider : OpenApiProvider -> Str
format_openapi_provider = |provider|
	match provider {
		NoOpenApi => "null"
		OpenApi(document) => format_json_string(document)
	}

format_http_mount : HttpMountOption -> Str
format_http_mount = |mount|
	match mount {
		None => "null"

		Mount(m) => {
			prefix_json = format_string_list(m.pathPrefix)
			static_json = format_file_bindings(m.staticBindings)
			bindings_json = format_file_bindings(m.filesystemBindings)
			headers_json = format_file_response_headers(m.fileResponseHeaders)
			cors_json = format_string_list(m.corsAllowedPatterns)
			"{\"pathPrefix\":${prefix_json},\"staticBindings\":${static_json},\"filesystemBindings\":${bindings_json},\"fileResponseHeaders\":${headers_json},\"corsAllowedPatterns\":${cors_json}}"
		}
	}

format_string_list : List(Str) -> Str
format_string_list = |items| {
	escaped = List.map(items, format_json_string)
	"[${Str.join_with(escaped, ",")}]"
}

format_json_string : Str -> Str
format_json_string = |item| "\"${escape_json_str(item)}\""

format_file_bindings : List(FileBinding) -> Str
format_file_bindings = |bindings| {
	formatted = List.map(bindings, format_file_binding)
	"[${Str.join_with(formatted, ",")}]"
}

format_file_binding : FileBinding -> Str
format_file_binding = |binding|
	match binding {
		Exact({ publicPath, filePath }) => "{\"exact\":{\"publicPath\":${format_string_list(publicPath)},\"filePath\":${format_json_string(filePath)}}}"

		Subtree({ publicPrefix, filesystemRoot }) => "{\"subtree\":{\"publicPrefix\":${format_string_list(publicPrefix)},\"filesystemRoot\":${format_json_string(filesystemRoot)}}}"
	}

format_file_response_headers : List(FileResponseHeader) -> Str
format_file_response_headers = |headers| {
	formatted = List.map(headers, format_file_response_header)
	"[${Str.join_with(formatted, ",")}]"
}

format_file_response_header : FileResponseHeader -> Str
format_file_response_header = |header| "{\"name\":${format_json_string(header.name)},\"value\":${format_json_string(header.value)}}"

format_config_declaration : ConfigDeclaration -> Str
format_config_declaration = |decl| {
	name_escaped = escape_json_str(decl.name)
	secret_str = match decl.secret {
		True => "true"
		False => "false"
	}
	"{\"name\":\"${name_escaped}\",\"secret\":${secret_str}}"
}

format_tool_definition : ToolDefinition -> Str
format_tool_definition = |tool| {
	formatted_params = List.map(tool.parameters, format_tool_parameter)
	params_json = Str.join_with(formatted_params, ",")

	name_escaped = escape_json_str(tool.name)
	desc_escaped = escape_json_str(tool.description)
	read_only_str = match tool.readOnly {
		True => "true"
		False => "false"
	}
	"{\"name\":\"${name_escaped}\",\"description\":\"${desc_escaped}\",\"parameters\":[${params_json}],\"readOnly\":${read_only_str}}"
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
