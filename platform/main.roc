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
# string so the app can read it verbatim (or ignore it).
parse_http_request : Str -> Types.HttpRequest
parse_http_request = |raw_json| {
	field = |name|
		match extract_json_field(raw_json, name) {
			Ok(value) => value
			Err(_) => ""
		}

	{
		method: field("method"),
		scheme: field("scheme"),
		authority: field("authority"),
		path: field("path"),
		query: field("query"),
		headersJson: field("headersJson"),
		body: field("body"),
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
# invocation's output was dropped. `extract_json_field` decodes `\u00XX`, so the
# two sides of this round trip now agree.
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
# character is what `extract_json_field` reads back.
vertical_tab = Str.from_utf8_lossy([11])

backspace = Str.from_utf8_lossy([8])

expect escape_json_str(vertical_tab) == "\\u000b"

expect escape_json_str(backspace) == "\\u0008"

expect extract_json_field("{\"a\":\"${escape_json_str(vertical_tab)}\"}", "a") == Ok(vertical_tab)

expect extract_json_field("{\"a\":\"a${escape_json_str(backspace)}b\"}", "a") == Ok(Str.concat("a", Str.concat(backspace, "b")))

# A UTF-8 sequence is carried as-is, not escaped byte by byte.
expect escape_json_str("é") == "é"

# The short escapes are what they were.
expect escape_json_str("a\"b\\c\nd\re\tf") == "a\\\"b\\\\c\\nd\\re\\tf"

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
	stdin = match extract_json_field(raw_json, "stdin") {
		Ok(v) => v
		Err(_) => ""
	}
	{ id, name, arguments, stdin }
}

# Reads one field out of the flat JSON object the host builds for a hook call:
# a compact object of string fields (`{"method":"GET","body":"..."}`), written
# by `serde_json`'s `to_string()`. Every field this platform reads — a tool
# call's `id`, `name`, `arguments` and `stdin`, a router request's `method`,
# `scheme`, `authority`, `path`, `query`, `headersJson` and `body` — is one JSON
# string.
#
# This walks the object's members the way JSON is written instead of searching
# the text for a `"name":"` substring:
#
#   - a value ends at its first *unescaped* quote, so a quote inside it (which
#     the host writes as `\"`) does not end it early, and a value that itself
#     contains the text `"name":"` cannot be mistaken for the field,
#   - the escapes the host's emitter writes are decoded back to the text they
#     stood for, and
#   - a missing field, a malformed escape, or an object that is not the flat
#     shape above all read the same to a caller: `NotFound`.
#
# The value scan splits at quotes rather than walking bytes, so a value with no
# escapes — the common case, and the 100 KB router body the e2e posts — costs a
# pass, not a copy per byte.
extract_json_field : Str, Str -> Try(Str, [NotFound])
extract_json_field = |json, field_name| {
	match Str.split_first(json, "{") {
		Ok({ before: "", after: body }) => scan_members(body, field_name)
		_ => Err(NotFound)
	}
}

# Walks the object's members from the first key: `"key":"value"`, repeated with
# a `,` between them. The walk reads each member's key and value whole, so a
# value's own text can never be read as a key. It returns at the key it was
# asked for; the end of the object without that key is `NotFound`.
scan_members : Str, Str -> Try(Str, [NotFound])
scan_members = |text, field_name| {
	if Str.starts_with(text, "}") {
		Err(NotFound)
	} else {
		match read_json_string(text) {
			Err(_) => Err(NotFound)
			Ok({ value: key, rest: after_key }) =>
				match after_separator(after_key, ":") {
					Err(_) => Err(NotFound)
					Ok(after_colon) =>
						match read_json_string(after_colon) {
							Err(_) => Err(NotFound)
							Ok({ value, rest: after_value }) =>
								if key == field_name {
									Ok(value)
								} else {
									match after_separator(after_value, ",") {
										Ok(next) => scan_members(next, field_name)
										Err(_) => Err(NotFound)
									}
								}
						}
				}
		}
	}
}

# The text after a separator that has to be at the current position.
after_separator : Str, Str -> Try(Str, [NotFound])
after_separator = |text, separator| {
	match Str.split_first(text, separator) {
		Ok({ before: "", after }) => Ok(after)
		_ => Err(NotFound)
	}
}

# Reads the JSON string at the start of `text` (which has to begin with its
# opening quote) and returns the text it stood for, plus the text after its
# closing quote.
read_json_string : Str -> Try({ value : Str, rest : Str }, [NotFound])
read_json_string = |text| {
	match Str.split_first(text, "\"") {
		Ok({ before: "", after: body }) => read_string_body(body)
		_ => Err(NotFound)
	}
}

# `body` starts at the string's first character, just past its opening quote.
read_string_body : Str -> Try({ value : Str, rest : Str }, [NotFound])
read_string_body = |body| {
	segments = Str.split_on(body, "\"")
	match string_end(segments) {
		Err(_) => Err(NotFound)
		Ok(index) =>
			match decode_json_escapes(Str.join_with(List.take_first(segments, index), "\"")) {
				Err(_) => Err(NotFound)
				Ok(value) => Ok({ value, rest: Str.join_with(List.drop_first(segments, index), "\"") })
			}
	}
}

# The index into `segments` of the piece that follows the string's *unescaped*
# closing quote. `segments` is the body split at every `"`, so the quote before
# `segments[j]` is escaped exactly when `segments[j - 1]` ends with an odd run of
# backslashes; the first piece whose predecessor does not is the closing quote.
# A string whose every quote is escaped (or which has none) never closes.
string_end : List(Str) -> Try(U64, [NotFound])
string_end = |segments| {
	quotes = List.len(segments) - 1
	escaped = escaped_quote_run(segments, 0, quotes)
	if escaped < quotes {
		Ok(escaped + 1)
	} else {
		Err(NotFound)
	}
}

# How many of the pieces before the first `limit` quotes end in an odd run of
# backslashes, i.e. how many leading quotes are escaped.
escaped_quote_run : List(Str), U64, U64 -> U64
escaped_quote_run = |segments, index, limit| {
	if index >= limit {
		index
	} else if ends_with_odd_backslashes(List.get(segments, index) ?? "") {
		escaped_quote_run(segments, index + 1, limit)
	} else {
		index
	}
}

# Whether `text`'s trailing run of backslashes is odd, which is what makes the
# quote after it escaped rather than the string's end.
ends_with_odd_backslashes : Str -> Bool
ends_with_odd_backslashes = |text| {
	if Str.ends_with(text, "\\") {
		match Str.split_last(text, "\\") {
			Ok({ before, after: _ }) => !ends_with_odd_backslashes(before)
			Err(_) => False
		}
	} else {
		False
	}
}

# Decodes the escapes `serde_json`'s compact emitter writes: `\"`, `\\`, `\/`,
# `\b`, `\f`, `\n`, `\r`, `\t`, and `\uXXXX` (any BMP code point, one to three
# UTF-8 bytes; a surrogate is paired with the `\uYYYY` after it, and an unpaired
# one becomes U+FFFD so the result is always valid UTF-8). A malformed escape is
# `NotFound` rather than a trap.
#
# A raw body splits at `\\` pairs first — one literal backslash each — so the
# runs that are left hold only single escapes and each decodes on its own. A
# value with no backslashes at all is one run of that split and costs one pass.
decode_json_escapes : Str -> Try(Str, [NotFound])
decode_json_escapes = |raw| {
	runs = List.map(Str.split_on(raw, "\\\\"), decode_run_escapes)
	if List.contains(List.map(runs, result_ok), False) {
		Err(NotFound)
	} else {
		Ok(Str.join_with(List.map(runs, |result| result ?? ""), "\\"))
	}
}

# The escapes in one run between `\\` pairs. Every `\` in it starts an escape
# and no two are adjacent, so it splits at each `\` into a literal head followed
# by one piece per escape.
decode_run_escapes : Str -> Try(Str, [NotFound])
decode_run_escapes = |run| {
	parts = Str.split_on(run, "\\")
	decoded = List.map_with_index(parts, |part, index| decode_escaped_part(parts, part, index))
	if List.contains(List.map(decoded, result_ok), False) {
		Err(NotFound)
	} else {
		Ok(Str.join_with(List.map(decoded, |result| result ?? ""), ""))
	}
}

result_ok : Try(a, e) -> Bool
result_ok = |result| match result {
	Ok(_) => True
	Err(_) => False
}

# One piece of a string body split at backslashes. Piece 0 is the raw text
# before the first backslash; every other piece begins with the character that
# backslash escaped, followed by more raw text. `parts` is the whole split, so a
# `\uXXXX` high surrogate can reach the piece that may hold its low half.
decode_escaped_part : List(Str), Str, U64 -> Try(Str, [NotFound])
decode_escaped_part = |parts, part, index| {
	if index == 0 {
		Ok(part)
	} else {
		bytes = Str.to_utf8(part)
		code = Str.from_utf8_lossy(List.take_first(bytes, 1))
		tail = Str.from_utf8_lossy(List.drop_first(bytes, 1))
		if code == "n" {
			Ok(Str.concat(char_of_code_point(10), tail))
		} else if code == "t" {
			Ok(Str.concat(char_of_code_point(9), tail))
		} else if code == "r" {
			Ok(Str.concat(char_of_code_point(13), tail))
		} else if code == "b" {
			Ok(Str.concat(char_of_code_point(8), tail))
		} else if code == "f" {
			Ok(Str.concat(char_of_code_point(12), tail))
		} else if code == "\"" {
			Ok(Str.concat("\"", tail))
		} else if code == "\\" {
			Ok(Str.concat("\\", tail))
		} else if code == "/" {
			Ok(Str.concat("/", tail))
		} else if code == "u" {
			decode_unicode_part(parts, index)
		} else {
			Err(NotFound)
		}
	}
}

# A `\uXXXX` piece: the code point it names and the raw text after its four
# digits. A high surrogate takes the following piece's low surrogate as its
# pair; a low surrogate that no high half claimed is U+FFFD on its own.
decode_unicode_part : List(Str), U64 -> Try(Str, [NotFound])
decode_unicode_part = |parts, index| {
	match unicode_part(parts, index) {
		Err(_) => Err(NotFound)
		Ok({ point, rest }) =>
			if is_high_surrogate(point) {
				match unicode_part(parts, index + 1) {
					Ok({ point: low, rest: low_rest }) =>
						if is_low_surrogate(low) {
							Ok(Str.concat(Str.concat(astral_char(point, low), rest), low_rest))
						} else {
							Ok(Str.concat(replacement_char, rest))
						}
					Err(_) => Ok(Str.concat(replacement_char, rest))
				}
			} else if is_low_surrogate(point) {
				if high_surrogate_before(parts, index) {
					# Already written with the high half that claimed it.
					Ok("")
				} else {
					Ok(Str.concat(replacement_char, rest))
				}
			} else {
				Ok(Str.concat(char_of_code_point(point), rest))
			}
	}
}

# The code point `\uXXXX` names and the raw text after its four digits, or
# `NotFound` when the piece is not four hex digits after a `u`.
unicode_part : List(Str), U64 -> Try({ point : U64, rest : Str }, [NotFound])
unicode_part = |parts, index| {
	bytes = Str.to_utf8(List.get(parts, index) ?? "")
	rest_bytes = List.drop_first(bytes, 1)
	if Str.from_utf8_lossy(List.take_first(bytes, 1)) == "u" and List.len(rest_bytes) >= 4 {
		match hex_code_point(List.take_first(rest_bytes, 4), 0, 0) {
			Err(_) => Err(NotFound)
			Ok(point) => Ok({ point, rest: Str.from_utf8_lossy(List.drop_first(rest_bytes, 4)) })
		}
	} else {
		Err(NotFound)
	}
}

# Whether the piece before `index` is a `\uXXXX` high surrogate, which means
# this piece's low half was already written with it.
high_surrogate_before : List(Str), U64 -> Bool
high_surrogate_before = |parts, index| {
	if index == 0 {
		False
	} else {
		match unicode_part(parts, index - 1) {
			Ok({ point, rest: _ }) => is_high_surrogate(point)
			Err(_) => False
		}
	}
}

# The value of four ASCII hex-digit bytes, read left to right.
hex_code_point : List(U8), U64, U64 -> Try(U64, [NotFound])
hex_code_point = |digits, index, value| {
	if index >= 4 {
		Ok(value)
	} else {
		match hex_digit(List.get(digits, index) ?? 255) {
			Err(_) => Err(NotFound)
			Ok(digit) => hex_code_point(digits, index + 1, value * 16 + digit)
		}
	}
}

hex_digit : U8 -> Try(U64, [NotFound])
hex_digit = |byte| {
	if byte >= 48 and byte <= 57 {
		Ok(U8.to_u64(byte - 48))
	} else if byte >= 97 and byte <= 102 {
		Ok(U8.to_u64(byte - 87))
	} else if byte >= 65 and byte <= 70 {
		Ok(U8.to_u64(byte - 55))
	} else {
		Err(NotFound)
	}
}

# A code point outside the surrogate range as its UTF-8 text: one to three bytes.
char_of_code_point : U64 -> Str
char_of_code_point = |point| {
	if point < 128 {
		Str.from_utf8_lossy([U64.to_u8_wrap(point)])
	} else if point < 2048 {
		Str.from_utf8_lossy([U64.to_u8_wrap(point // 64 + 192), U64.to_u8_wrap(point % 64 + 128)])
	} else {
		Str.from_utf8_lossy([U64.to_u8_wrap(point // 4096 + 224), U64.to_u8_wrap(point // 64 % 64 + 128), U64.to_u8_wrap(point % 64 + 128)])
	}
}

# The astral character a surrogate pair stands for: 0x10000 + (high - 0xD800) *
# 0x400 + (low - 0xDC00), as its four UTF-8 bytes.
astral_char : U64, U64 -> Str
astral_char = |high, low| {
	point = 65536 + (high - 55296) * 1024 + (low - 56320)
	Str.from_utf8_lossy([U64.to_u8_wrap(point // 262144 + 240), U64.to_u8_wrap(point // 4096 % 64 + 128), U64.to_u8_wrap(point // 64 % 64 + 128), U64.to_u8_wrap(point % 64 + 128)])
}

replacement_char : Str
replacement_char = char_of_code_point(65533)

is_high_surrogate : U64 -> Bool
is_high_surrogate = |point| point >= 55296 and point <= 56319

is_low_surrogate : U64 -> Bool
is_low_surrogate = |point| point >= 56320 and point <= 57343
# The reader's rules, pinned as tests (`roc test platform/main.roc`; CI runs it).
# The `\uXXXX` ones live here because nothing else can reach them: the host
# writes non-ASCII as raw UTF-8, and a control character in a value cannot
# survive the round trip through the platform's own output escaping (see the
# README's tool-streams note), so an end-to-end check cannot see the decode.

# The member scan: a key is read as a key, so a value carrying the text of the
# field — even before the real field — is never mistaken for it.
expect extract_json_field("{\"a\":\"x\"}", "a") == Ok("x")

expect extract_json_field("{\"a\":\"x\",\"b\":\"y\"}", "b") == Ok("y")

expect extract_json_field("{\"a\":\"decoy \\\"b\\\":\\\"FAKE\\\"\",\"b\":\"real\"}", "b") == Ok("real")

expect extract_json_field("{\"body\":\"{\\\"b\\\":\\\"1\\\"}\",\"b\":\"real\"}", "b") == Ok("real")

expect extract_json_field("{\"arguments\":\"{\\\"q\\\":\\\"hi\\\",\\\"p\\\":\\\"C:\\\\\\\\tmp\\\"}\",\"stdin\":\"s\"}", "arguments") == Ok("{\"q\":\"hi\",\"p\":\"C:\\\\tmp\"}")

# A value ends at its first *unescaped* quote, and the escapes are decoded.
expect extract_json_field("{\"a\":\"he said \\\"hi\\\"\"}", "a") == Ok("he said \"hi\"")

expect extract_json_field("{\"a\":\"C:\\\\tmp\"}", "a") == Ok("C:\\tmp")

expect extract_json_field("{\"a\":\"C:\\\\t\"}", "a") == Ok("C:\\t")

expect extract_json_field("{\"a\":\"a\\\\\\u0041\"}", "a") == Ok("a\\A")

expect extract_json_field("{\"a\":\"trailing backslash \\\\\"}", "a") == Ok("trailing backslash \\")

expect extract_json_field("{\"a\":\"\\\"\"}", "a") == Ok("\"")

expect extract_json_field("{\"a\":\"line1\\nline2\\ttab\\rcr\"}", "a") == Ok("line1\nline2\ttab\rcr")

expect extract_json_field("{\"a\":\"back\\bspace\"}", "a") == Ok("back${char_of_code_point(8)}space")

expect extract_json_field("{\"a\":\"form\\ffeed\\/slash\"}", "a") == Ok("form${char_of_code_point(12)}feed/slash")

# `\uXXXX`: the whole BMP, one to three UTF-8 bytes, and a surrogate pair as the
# astral character it stands for.
expect extract_json_field("{\"a\":\"\\u0041\\u007a\"}", "a") == Ok("Az")

expect extract_json_field("{\"a\":\"caf\\u00e9\"}", "a") == Ok("café")

expect extract_json_field("{\"a\":\"\\u4e2d\\u6587\"}", "a") == Ok("中文")

expect extract_json_field("{\"a\":\"\\ud83d\\ude00\"}", "a") == Ok("😀")

# A surrogate with no partner is U+FFFD, never invalid UTF-8.
expect extract_json_field("{\"a\":\"lone high \\ud83d end\"}", "a") == Ok("lone high � end")

expect extract_json_field("{\"a\":\"lone low \\ude00 end\"}", "a") == Ok("lone low � end")

expect extract_json_field("{\"a\":\"high then text \\ud83d x\"}", "a") == Ok("high then text � x")

# Empty values, and everything that cannot be read as this object is NotFound.
expect extract_json_field("{\"a\":\"\"}", "a") == Ok("")

expect extract_json_field("{\"a\":\"\",\"b\":\"y\"}", "b") == Ok("y")

expect extract_json_field("{\"a\":\"x\"}", "missing") == Err(NotFound)

expect extract_json_field("{\"a\":\"unterminated}", "a") == Err(NotFound)

expect extract_json_field("{\"a\":\"bad \\q esc\"}", "a") == Err(NotFound)

expect extract_json_field("{\"a\":1}", "a") == Err(NotFound)
