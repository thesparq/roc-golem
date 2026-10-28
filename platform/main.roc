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
		"main_handle_multimodal_for_host_1_exposed_generic": main_handle_multimodal_for_host!,
		"main_handle_request_for_host_1_exposed_generic": main_handle_request_for_host!,
		"main_handle_tool_call_for_host_1_exposed_generic": main_handle_tool_call_for_host!,
		"main_init_for_host_1_exposed_generic": main_init_for_host!,
		"main_metadata_for_host_1_exposed_generic": main_metadata_for_host,
	}
	hosted {
		"rocFxAppendFile": Host.rocFxAppendFile!,
		"rocFxAwaitWebhook": Host.rocFxAwaitWebhook!,
		"rocFxBlobCreateContainer": Host.rocFxBlobCreateContainer!,
		"rocFxBlobDelete": Host.rocFxBlobDelete!,
		"rocFxBlobGet": Host.rocFxBlobGet!,
		"rocFxBlobHas": Host.rocFxBlobHas!,
		"rocFxBlobList": Host.rocFxBlobList!,
		"rocFxBlobPut": Host.rocFxBlobPut!,
		"rocFxCreateWebhook": Host.rocFxCreateWebhook!,
		"rocFxDbBegin": Host.rocFxDbBegin!,
		"rocFxDbCommit": Host.rocFxDbCommit!,
		"rocFxDbExecute": Host.rocFxDbExecute!,
		"rocFxDbOpen": Host.rocFxDbOpen!,
		"rocFxDbOpenIgnite": Host.rocFxDbOpenIgnite!,
		"rocFxDbOpenMysql": Host.rocFxDbOpenMysql!,
		"rocFxDbQuery": Host.rocFxDbQuery!,
		"rocFxDbRollback": Host.rocFxDbRollback!,
		"rocFxDeleteFile": Host.rocFxDeleteFile!,
		"rocFxDurableStreamAppend": Host.rocFxDurableStreamAppend!,
		"rocFxDurableStreamRead": Host.rocFxDurableStreamRead!,
		"rocFxDurableStreamReader": Host.rocFxDurableStreamReader!,
		"rocFxDurableStreamRelease": Host.rocFxDurableStreamRelease!,
		"rocFxDurableStreamWriter": Host.rocFxDurableStreamWriter!,
		"rocFxFileExists": Host.rocFxFileExists!,
		"rocFxFileSize": Host.rocFxFileSize!,
		"rocFxFork": Host.rocFxFork!,
		"rocFxGetConfig": Host.rocFxGetConfig!,
		"rocFxGetPrincipal": Host.rocFxGetPrincipal!,
		"rocFxGetRetryPolicies": Host.rocFxGetRetryPolicies!,
		"rocFxGetRetryPolicy": Host.rocFxGetRetryPolicy!,
		"rocFxGetWorkerId": Host.rocFxGetWorkerId!,
		"rocFxHttpRequest": Host.rocFxHttpRequest!,
		"rocFxIdempotencyKey": Host.rocFxIdempotencyKey!,
		"rocFxKvDelete": Host.rocFxKvDelete!,
		"rocFxKvExists": Host.rocFxKvExists!,
		"rocFxKvGet": Host.rocFxKvGet!,
		"rocFxKvSet": Host.rocFxKvSet!,
		"rocFxListDir": Host.rocFxListDir!,
		"rocFxLog": Host.rocFxLog!,
		"rocFxMakeAgentId": Host.rocFxMakeAgentId!,
		"rocFxMakeDir": Host.rocFxMakeDir!,
		"rocFxNewQuotaToken": Host.rocFxNewQuotaToken!,
		"rocFxNextRetryDelay": Host.rocFxNextRetryDelay!,
		"rocFxNowMillis": Host.rocFxNowMillis!,
		"rocFxOplogCommit": Host.rocFxOplogCommit!,
		"rocFxParseAgentId": Host.rocFxParseAgentId!,
		"rocFxPermissionDerive": Host.rocFxPermissionDerive!,
		"rocFxPermissionDeriveFromWallet": Host.rocFxPermissionDeriveFromWallet!,
		"rocFxPermissionDeriveScope": Host.rocFxPermissionDeriveScope!,
		"rocFxPermissionExpiresAt": Host.rocFxPermissionExpiresAt!,
		"rocFxPermissionId": Host.rocFxPermissionId!,
		"rocFxPermissionInspect": Host.rocFxPermissionInspect!,
		"rocFxPermissionInstall": Host.rocFxPermissionInstall!,
		"rocFxPermissionListModules": Host.rocFxPermissionListModules!,
		"rocFxPermissionParents": Host.rocFxPermissionParents!,
		"rocFxPermissionRelease": Host.rocFxPermissionRelease!,
		"rocFxPermissionRevoke": Host.rocFxPermissionRevoke!,
		"rocFxPermissionValidateGrant": Host.rocFxPermissionValidateGrant!,
		"rocFxPermissionsWallet": Host.rocFxPermissionsWallet!,
		"rocFxPermissionsWalletVersion": Host.rocFxPermissionsWalletVersion!,
		"rocFxQuotaCommit": Host.rocFxQuotaCommit!,
		"rocFxQuotaMerge": Host.rocFxQuotaMerge!,
		"rocFxQuotaReserve": Host.rocFxQuotaReserve!,
		"rocFxQuotaSplit": Host.rocFxQuotaSplit!,
		"rocFxReadFile": Host.rocFxReadFile!,
		"rocFxRemoveRetryPolicy": Host.rocFxRemoveRetryPolicy!,
		"rocFxResolveAgentId": Host.rocFxResolveAgentId!,
		"rocFxResolveComponentId": Host.rocFxResolveComponentId!,
		"rocFxResolveRetryPolicy": Host.rocFxResolveRetryPolicy!,
		"rocFxRpcCancel": Host.rocFxRpcCancel!,
		"rocFxRpcFireAndForget": Host.rocFxRpcFireAndForget!,
		"rocFxRpcInvoke": Host.rocFxRpcInvoke!,
		"rocFxRpcSchedule": Host.rocFxRpcSchedule!,
		"rocFxRpcScheduleCancelable": Host.rocFxRpcScheduleCancelable!,
		"rocFxSetPersistence": Host.rocFxSetPersistence!,
		"rocFxSetRetryPolicy": Host.rocFxSetRetryPolicy!,
		"rocFxSleepMillis": Host.rocFxSleepMillis!,
		"rocFxTrap": Host.rocFxTrap!,
		"rocFxWriteFile": Host.rocFxWriteFile!,
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
	HttpHandlerOption,
	HttpHeader,
	HttpMountOption,
	HttpResponse,
	MultimodalOption,
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
			# The tool's stdout/stderr travel next to the payload: the host writes
			# them through the streams the caller attached (`golem tool invoke
			# --stdout` / `--stderr`).
			escaped_stdout = escape_json_str(res.result.stdout)
			escaped_stderr = escape_json_str(res.result.stderr)
			Ok("{\"state\":${next_state_json},\"success\":${success_str},\"output\":\"${escaped_output}\",\"stdout\":\"${escaped_stdout}\",\"stderr\":\"${escaped_stderr}\"}")
		}
	}
}

main_handle_multimodal_for_host! : Str, Str => Try(Str, Str)
main_handle_multimodal_for_host! = |state_json, parts_json| {
	match (agent.deserializeState)(state_json) {
		Err(err) =>
			Err("Failed to deserialize state: ${err}")

		Ok(current_model) => {
			res = (agent.handleMultimodal!)(current_model, parts_json)
			next_state_json = (agent.serializeState)(res.state)
			Ok("{\"state\":${next_state_json},\"parts\":${format_json_string(res.parts)}}")
		}
	}
}

main_handle_request_for_host! : Str, Str => Try(Str, Str)
main_handle_request_for_host! = |state_json, request_json| {
	match (agent.deserializeState)(state_json) {
		Err(err) =>
			Err("Failed to deserialize state: ${err}")

		Ok(current_model) => {
			request = parse_http_request(request_json)
			res = (agent.handleRequest!)(current_model, request)
			next_state_json = (agent.serializeState)(res.state)
			headers_json = format_http_headers(res.response.headers)
			body_escaped = escape_json_str(res.response.body)
			Ok("{\"state\":${next_state_json},\"status\":${U16.to_str(res.response.status)},\"headers\":[${headers_json}],\"body\":\"${body_escaped}\"}")
		}
	}
}

# The host builds this from the request Golem handed it; the header list stays a
# string so the app can read it verbatim (or ignore it). The built-in `Json`
# codec parses the flat object of JSON strings: every field is optional
# (`Try(Str, [Missing])`), so an absent field — or a payload that is not that
# object at all — reads as `""`, the same fallback these callers have always used.
parse_http_request : Str -> Types.HttpRequest
parse_http_request = |raw_json| {
	parsed : Try(
		{
			method : Try(Str, [Missing]),
			scheme : Try(Str, [Missing]),
			authority : Try(Str, [Missing]),
			path : Try(Str, [Missing]),
			query : Try(Str, [Missing]),
			headersJson : Try(Str, [Missing]),
			body : Try(Str, [Missing]),
		},
		[InvalidJson(Str)],
	)
	parsed = Json.parse(raw_json)
	match parsed {
		Err(_) => {
			method: "",
			scheme: "",
			authority: "",
			path: "",
			query: "",
			headersJson: "",
			body: "",
		}
		Ok(fields) => {
			method: fields.method ?? "",
			scheme: fields.scheme ?? "",
			authority: fields.authority ?? "",
			path: fields.path ?? "",
			query: fields.query ?? "",
			headersJson: fields.headersJson ?? "",
			body: fields.body ?? "",
		}
	}
}

format_http_headers : List(HttpHeader) -> Str
format_http_headers = |headers| {
	formatted = List.map(headers, format_http_header)
	Str.join_with(formatted, ",")
}

format_http_header : HttpHeader -> Str
format_http_header = |header| "{\"name\":${format_json_string(header.name)},\"value\":${format_json_string(header.value)}}"

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
	handler_json = format_http_handler(meta.httpHandler)
	multimodal_json = format_multimodal(meta.multimodal)
	openapi_json = format_openapi_provider(meta.openapiProvider)

	"{\"name\":\"${name_escaped}\",\"version\":\"${version_escaped}\",\"description\":\"${desc_escaped}\",\"tools\":[${tools_json}],\"config\":[${config_json}],\"agentKind\":${kind_json},\"httpMount\":${mount_json},\"httpHandler\":${handler_json},\"multimodal\":${multimodal_json},\"openapiProvider\":${openapi_json}}"
}

format_multimodal : MultimodalOption -> Str
format_multimodal = |multimodal|
	match multimodal {
		NoMultimodal => "null"
		HandlesMultimodal => "\"multimodal\""
	}

format_http_handler : HttpHandlerOption -> Str
format_http_handler = |handler|
	match handler {
		NoHandler => "null"
		HandlesRequests => "\"handler\""
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

# The control characters below 0x20 with no short escape, as `(byte, escape)`
# pairs. `\n`, `\r` and `\t` are replaced directly above; `\b` and `\f` are here
# as their `\u` forms so that no control character has to appear in this source
# file. A byte 0x80 or above is never escaped: it is part of a UTF-8 sequence,
# which JSON carries as-is.
control_escapes : List((U8, Str))
control_escapes = [
	(0, "\\u0000"),
	(1, "\\u0001"),
	(2, "\\u0002"),
	(3, "\\u0003"),
	(4, "\\u0004"),
	(5, "\\u0005"),
	(6, "\\u0006"),
	(7, "\\u0007"),
	(8, "\\u0008"),
	(11, "\\u000b"),
	(12, "\\u000c"),
	(14, "\\u000e"),
	(15, "\\u000f"),
	(16, "\\u0010"),
	(17, "\\u0011"),
	(18, "\\u0012"),
	(19, "\\u0013"),
	(20, "\\u0014"),
	(21, "\\u0015"),
	(22, "\\u0016"),
	(23, "\\u0017"),
	(24, "\\u0018"),
	(25, "\\u0019"),
	(26, "\\u001a"),
	(27, "\\u001b"),
	(28, "\\u001c"),
	(29, "\\u001d"),
	(30, "\\u001e"),
	(31, "\\u001f"),
]

# Escapes a string for the JSON this platform hands the host. Every control
# character below 0x20 has to be escaped: the host parses these payloads with
# `serde_json`, which refuses a raw one, so an answer that carried one — a reply,
# a response body, a tool's stdout — was rejected as malformed JSON and the
# invocation's output was dropped. The built-in `Json` codec writes the same
# `\u00XX` form, so what this writes is what `Json.parse` reads back.
escape_json_str : Str -> Str
escape_json_str = |input| {
	s1 = Str.replace_each(input, "\\", "\\\\")
	s2 = Str.replace_each(s1, "\"", "\\\"")
	s3 = Str.replace_each(s2, "\n", "\\n")
	s4 = Str.replace_each(s3, "\r", "\\r")
	s5 = Str.replace_each(s4, "\t", "\\t")
	apply_escapes(s5, control_escapes)
}

# Applies the `(byte, escape)` pairs of `control_escapes` in turn.
apply_escapes : Str, List((U8, Str)) -> Str
apply_escapes = |text, pairs|
	apply_escapes_at(text, pairs, 0)

apply_escapes_at : Str, List((U8, Str)), U64 -> Str
apply_escapes_at = |text, pairs, index|
	match List.get(pairs, index) {
		Err(_) => text
		Ok((byte, escape)) =>
			apply_escapes_at(Str.replace_each(text, Str.from_utf8_lossy([byte]), escape), pairs, index + 1)
	}

# The emitter and the reader agree: what `escape_json_str` writes for a control
# character is what `Json.parse` reads back.
vertical_tab = Str.from_utf8_lossy([11])

backspace = Str.from_utf8_lossy([8])

expect escape_json_str(vertical_tab) == "\\u000b"

expect escape_json_str(backspace) == "\\u0008"

expect {
	round_trip : Try({ a : Str }, [InvalidJson(Str), MissingRequiredField(Str)])
	round_trip = Json.parse("{\"a\":\"${escape_json_str(vertical_tab)}\"}")
	round_trip == Ok({ a: vertical_tab })
}

expect {
	round_trip : Try({ a : Str }, [InvalidJson(Str), MissingRequiredField(Str)])
	round_trip = Json.parse("{\"a\":\"a${escape_json_str(backspace)}b\"}")
	round_trip == Ok({ a: Str.concat("a", Str.concat(backspace, "b")) })
}

# A UTF-8 sequence is carried as-is, not escaped byte by byte.
expect escape_json_str("é") == "é"

# The short escapes are what they were.
expect escape_json_str("a\"b\\c\nd\re\tf") == "a\\\"b\\\\c\\nd\\re\\tf"

# The host writes the tool call as a flat object of JSON strings. The built-in
# `Json` codec parses it; every field is optional, so an absent one — or a
# payload that is not that object — takes the same default as before.
parse_tool_call : Str -> ToolCall
parse_tool_call = |raw_json| {
	call : Try(
		{
			id : Try(Str, [Missing]),
			name : Try(Str, [Missing]),
			arguments : Try(Str, [Missing]),
			stdin : Try(Str, [Missing]),
		},
		[InvalidJson(Str)],
	)
	call = Json.parse(raw_json)
	match call {
		Err(_) => { id: "call-1", name: "default", arguments: "{}", stdin: "" }
		Ok(fields) => {
			id = fields.id ?? "call-1"
			name = fields.name ?? "default"
			arguments = fields.arguments ?? "{}"
			stdin = fields.stdin ?? ""
			{ id, name, arguments, stdin }
		}
	}
}

# The reader is the built-in `Json` codec. These pin the mapping from the host's
# payloads onto `parse_tool_call` / `parse_http_request`: a present field is
# decoded (including the escapes the host's emitter writes), an absent one takes
# the default, and a value that contains another field's text is still read as a
# value — `Json.parse` reads member names as names.

expect parse_tool_call("{\"id\":\"c1\",\"name\":\"echo\",\"arguments\":\"{}\",\"stdin\":\"s\"}") == { id: "c1", name: "echo", arguments: "{}", stdin: "s" }

expect parse_tool_call("{}") == { id: "call-1", name: "default", arguments: "{}", stdin: "" }

expect parse_tool_call("not json") == { id: "call-1", name: "default", arguments: "{}", stdin: "" }

# A value carrying the text of a later field is not mistaken for that field.
expect parse_tool_call("{\"name\":\"decoy \\\"arguments\\\":\\\"FAKE\\\"\",\"arguments\":\"real\"}") == { id: "call-1", name: "decoy \"arguments\":\"FAKE\"", arguments: "real", stdin: "" }

# The escapes the host writes are decoded.
expect parse_tool_call("{\"name\":\"he said \\\"hi\\\"\",\"stdin\":\"line1\\nline2\"}") == { id: "call-1", name: "he said \"hi\"", arguments: "{}", stdin: "line1\nline2" }

expect parse_tool_call("{\"stdin\":\"caf\\u00e9 \\ud83d\\ude00\"}") == { id: "call-1", name: "default", arguments: "{}", stdin: "café 😀" }

# A control character survives the emitter/reader round trip.
expect parse_tool_call("{\"stdin\":\"${escape_json_str(vertical_tab)}\"}") == { id: "call-1", name: "default", arguments: "{}", stdin: vertical_tab }

expect parse_http_request("{\"method\":\"GET\",\"scheme\":\"http\",\"authority\":\"h\",\"path\":\"/p\",\"query\":\"q=1\",\"headersJson\":\"[]\",\"body\":\"b\"}") == { method: "GET", scheme: "http", authority: "h", path: "/p", query: "q=1", headersJson: "[]", body: "b" }

# Only the fields the host sent are set; the rest take their default.
expect parse_http_request("{\"method\":\"GET\"}") == { method: "GET", scheme: "", authority: "", path: "", query: "", headersJson: "", body: "" }

expect parse_http_request("{}") == { method: "", scheme: "", authority: "", path: "", query: "", headersJson: "", body: "" }
