platform "golem"
	requires {
		[State : state] for agent : Golem.Agent(state)
	}
	exposes [
		Golem,
		Types,
		Json,
	]
	packages {}
	provides {
		"main_init_for_host": main_init_for_host,
		"main_handle_message_for_host": main_handle_message_for_host!,
		"main_handle_tool_call_for_host": main_handle_tool_call_for_host!,
		"main_metadata_for_host": main_metadata_for_host,
	}
	hosted {
		"rocFxLog": Effect.rocFxLog!,
		"rocFxGetWorkerId": Effect.rocFxGetWorkerId!,
		"rocFxRpcInvoke": Effect.rocFxRpcInvoke!,
		"rocFxRpcSchedule": Effect.rocFxRpcSchedule!,
		"rocFxRpcFireAndForget": Effect.rocFxRpcFireAndForget!,
		"rocFxSetPersistence": Effect.rocFxSetPersistence!,
		"rocFxHttpRequest": Effect.rocFxHttpRequest!,
		"rocFxWsConnect": Effect.rocFxWsConnect!,
		"rocFxWsSend": Effect.rocFxWsSend!,
		"rocFxWsReceive": Effect.rocFxWsReceive!,
		"rocFxWsClose": Effect.rocFxWsClose!,
		"rocFxSleepMillis": Effect.rocFxSleepMillis!,
		"rocFxNowMillis": Effect.rocFxNowMillis!,
		"rocFxReadFileBytes": Effect.rocFxReadFileBytes!,
		"rocFxWriteFileBytes": Effect.rocFxWriteFileBytes!,
		"rocFxAppendFileBytes": Effect.rocFxAppendFileBytes!,
		"rocFxReadFile": Effect.rocFxReadFile!,
		"rocFxWriteFile": Effect.rocFxWriteFile!,
		"rocFxKvGetBytes": Effect.rocFxKvGetBytes!,
		"rocFxKvSetBytes": Effect.rocFxKvSetBytes!,
		"rocFxKvGet": Effect.rocFxKvGet!,
		"rocFxKvSet": Effect.rocFxKvSet!,
		"rocFxBlobGetBytes": Effect.rocFxBlobGetBytes!,
		"rocFxBlobPutBytes": Effect.rocFxBlobPutBytes!,
		"rocFxBlobGet": Effect.rocFxBlobGet!,
		"rocFxBlobPut": Effect.rocFxBlobPut!,
		"rocFxHttpRequestBytes": Effect.rocFxHttpRequestBytes!,
		"rocFxGetSecretHandle": Effect.rocFxGetSecretHandle!,
		"rocFxRevealSecret": Effect.rocFxRevealSecret!,
		"rocFxGenerateIdempotencyKey": Effect.rocFxGenerateIdempotencyKey!,
		"rocFxGetOplogIndex": Effect.rocFxGetOplogIndex!,
		"rocFxOplogCommit": Effect.rocFxOplogCommit!,
		"rocFxCreatePromise": Effect.rocFxCreatePromise!,
		"rocFxCompletePromise": Effect.rocFxCompletePromise!,
		"rocFxGetPromise": Effect.rocFxGetPromise!,
		"rocFxCreateWebhook": Effect.rocFxCreateWebhook!,
		"rocFxConfigGet": Effect.rocFxConfigGet!,
		"rocFxTraceId": Effect.rocFxTraceId!,
		"rocFxSpanId": Effect.rocFxSpanId!,
		"rocFxTraceHeaders": Effect.rocFxTraceHeaders!,
		"rocFxGetIdempotenceMode": Effect.rocFxGetIdempotenceMode!,
		"rocFxSetIdempotenceMode": Effect.rocFxSetIdempotenceMode!,
		"rocFxRevertAgent": Effect.rocFxRevertAgent!,
		"rocFxDbExecute": Effect.rocFxDbExecute!,
		"rocFxDbExecuteParams": Effect.rocFxDbExecuteParams!,
		"rocFxDbQuery": Effect.rocFxDbQuery!,
		"rocFxDbQueryParams": Effect.rocFxDbQueryParams!,
		"rocFxRpcInvokeBytes": Effect.rocFxRpcInvokeBytes!,
		"rocFxRpcScheduleBytes": Effect.rocFxRpcScheduleBytes!,
		"rocFxRpcFireAndForgetBytes": Effect.rocFxRpcFireAndForgetBytes!,
		"rocFxHttpStreamOpen": Effect.rocFxHttpStreamOpen!,
		"rocFxHttpStreamRead": Effect.rocFxHttpStreamRead!,
		"rocFxHttpStreamClose": Effect.rocFxHttpStreamClose!,
	}
	targets: {
		inputs_dir: "targets/",
		wasm32: {
			inputs: ["libroc_golem_host.a", app],
			output: Shared,
			exports: ["main_init_for_host", "main_handle_message_for_host", "main_handle_tool_call_for_host", "main_metadata_for_host", "golem:agent/guest@2.0.0#discover-agent-types", "golem:agent/guest@2.0.0#get-definition", "golem:agent/guest@2.0.0#initialize", "golem:agent/guest@2.0.0#invoke", "cabi_post_golem:agent/guest@2.0.0#discover-agent-types", "cabi_post_golem:agent/guest@2.0.0#get-definition", "cabi_post_golem:agent/guest@2.0.0#initialize", "cabi_post_golem:agent/guest@2.0.0#invoke", "cabi_realloc"],
		},
	}

import Types exposing [
	Result,
	ToolDefinition,
	ToolParameter,
	ToolCall,
	MethodDeclaration,
	AgentMode,
	HttpMount,
]
import Golem
import Effect
import Json exposing [encode, decode, decode_or, escape]

## Entry point for agent initialization called by Rust Host
main_init_for_host : Str -> Result(Str, Str)
main_init_for_host = |envelope_or_config| {
	agent_type = extract_json_field(envelope_or_config, "agent_type") ?? ""
	config = extract_json_field(envelope_or_config, "config") ?? envelope_or_config
	match find_agent_by_name(agent.agents, agent_type) {
		Err(err) => Err(err)
		Ok(target_agent) => (target_agent.init)(config)
	}
}

## Entry point for handling a message or typed method call by Rust Host
main_handle_message_for_host! : Str, Str => Result(Str, Str)
main_handle_message_for_host! = |state_json, envelope_or_msg| {
	agent_type = extract_json_field(envelope_or_msg, "agent_type") ?? ""
	method_name = extract_json_field(envelope_or_msg, "method") ?? ""
	input_str = extract_json_field(envelope_or_msg, "input") ?? envelope_or_msg

	match find_agent_by_name(agent.agents, agent_type) {
		Err(err) => Err(err)
		Ok(target_agent) =>
			if Str.is_empty(method_name) or method_name == "handle-message" or method_name == "message" {
				match (target_agent.handleMessage)(state_json, input_str) {
					Ok(res) => Ok(format_reply_envelope(res.nextStateJson, res.response))
					Err(err) => Err(err)
				}
			} else {
				match (target_agent.invokeMethod)(method_name, state_json, input_str) {
					Ok(res) => Ok(format_reply_envelope(res.nextStateJson, res.out))
					Err(err) => Err(err)
				}
			}
	}
}

## Entry point for handling a tool invocation called by Rust Host
main_handle_tool_call_for_host! : Str, Str => Result(Str, Str)
main_handle_tool_call_for_host! = |state_json, tool_call_json| {
	agent_type = extract_json_field(tool_call_json, "agent_type") ?? ""
	parsed_call = parse_tool_call(tool_call_json)
	match find_agent_by_name(agent.agents, agent_type) {
		Err(err) => Err(err)
		Ok(target_agent) =>
			match (target_agent.handleToolCall)(state_json, parsed_call) {
				Ok(res) => Ok(format_tool_envelope(res.nextStateJson, res.result.success, res.result.output))
				Err(err) => Err(err)
			}
	}
}

## Expose metadata to Rust Host as a JSON string
main_metadata_for_host : {} -> Str
main_metadata_for_host = |{}| {
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
			Durable => "durable"
			Ephemeral => "ephemeral"
		}
	tools_json =
		meta.tools
			|> List.map(format_tool_definition)
			|> Str.join_with(",")
	methods_json =
		meta.methods
			|> List.map(format_method_declaration)
			|> Str.join_with(",")
	http_mount_field =
		match meta.httpMount {
			None => ""
			Some(mount) => {
				prefix_escaped = escape_json_str(mount.pathPrefix)
				suffix_escaped = escape_json_str(mount.webhookSuffix)
				cors_list =
					mount.corsOrigins
						|> List.map(|o| "\"${escape_json_str(o)}\"")
						|> Str.join_with(",")
				",\"http_mount\":{\"path_prefix\":\"${prefix_escaped}\",\"cors_origins\":[${cors_list}],\"webhook_suffix\":\"${suffix_escaped}\"}"
			}
		}
	"{\"name\":\"${name_escaped}\",\"version\":\"${version_escaped}\",\"description\":\"${desc_escaped}\",\"mode\":\"${mode_str}\",\"methods\":[${methods_json}],\"tools\":[${tools_json}]${http_mount_field}}"
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
	escape(input)

format_reply_envelope : Str, Str -> Str
format_reply_envelope = |next_state_json, response| {
	escaped_state = escape_json_str(next_state_json)
	escaped_response = escape_json_str(response)
	"{\"state\":\"${escaped_state}\",\"response\":\"${escaped_response}\"}"
}

format_tool_envelope : Str, Bool, Str -> Str
format_tool_envelope = |next_state_json, success, output| {
	escaped_state = escape_json_str(next_state_json)
	escaped_output = escape_json_str(output)
	success_str = if success "true" else "false"
	"{\"state\":\"${escaped_state}\",\"success\":${success_str},\"output\":\"${escaped_output}\"}"
}

parse_tool_call : Str -> ToolCall
parse_tool_call = |raw_json|
	decode_or(
		raw_json,
		{
			id: "call-1",
			name: "default",
			arguments: "{}",
		},
	)

extract_json_field : Str, Str -> Result(Str, [NotFound])
extract_json_field = |json, field_name| {
	target = "\"${field_name}\":\""
	match Str.split_first(json, target) {
		Ok({ after, .. }) =>
			match Str.split_first(after, "\"") {
				Ok({ before, .. }) => Ok(before)
				Err(_) => Err(NotFound)
			}

		Err(_) =>
			Err(NotFound)
	}
}

find_agent_by_name : List(Golem.ErasedAgent), Str -> Result(Golem.ErasedAgent, Str)
find_agent_by_name = |agents, target_name|
	if Str.is_empty(target_name) {
		match agents {
			[] => Err("No agents defined in component")
			[first, ..] => Ok(first)
		}
	} else {
		match agents {
			[] => Err("Agent type '${target_name}' not found")
			[first, .. as rest] =>
				if first.metadata.name == target_name {
					Ok(first)
				} else {
					find_agent_by_name(rest, target_name)
				}
		}
	}
