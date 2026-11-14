module [
	# --- Agent Builders ---
	Agent,
	ErasedAgent,
	Spec,
	agent,
	agentWith,
	simpleAgent,
	ephemeral,
	simpleEphemeral,
	define,
	defineEphemeral,
	defineAgent,
	build,
	bundle,
	bundle2,
	bundleAll,
	erase,

	# --- Method & Handler Registration ---
	Method,
	Handler,
	method,
	register,
	registerReadOnly,
	registerEphemeral,
	registerTyped,
	registerReadOnlyTyped,
	registerEphemeralTyped,
	registerEffectful,
	registerEffectfulTyped,
	withHttpMount,

	# --- Tool Helpers ---
	Tool,
	StatelessTool,
	tool,
	toolJson,
	statefulTool,
	statelessTool,
	statelessToolJson,
	statelessToTool,
	withTools,
	withToolHandler,
	withStatelessToolHandler,
	withEffectfulToolHandler,
	withEffectfulStatelessToolHandler,
	param,
	optionalParam,
	numberParam,
	boolParam,
	typedParam,
	dispatchTool,
	dispatchMessage,
	dispatchStatelessTool,

	# --- Response Primitives ---
	reply,
	readOnly,
	stay,
	fail,
	replyJson,
	readOnlyJson,

	# --- Logging ---
	logTrace,
	logDebug,
	logInfo,
	logWarn,
	logError,

	# --- Worker Identity ---
	workerId,

	# --- RPC ---
	rpc,
	rpcInstance,
	rpcSchedule,
	rpcScheduleInstance,
	rpcFireAndForget,
	rpcFireAndForgetInstance,
	rpcTyped,
	rpcTypedInstance,
	rpcScheduleTyped,
	rpcScheduleTypedInstance,
	rpcFireAndForgetTyped,
	rpcFireAndForgetTypedInstance,
	rpcBytes,
	rpcBytesInstance,
	rpcScheduleBytes,
	rpcScheduleBytesInstance,
	rpcFireAndForgetBytes,
	rpcFireAndForgetBytesInstance,

	# --- HTTP ---
	http,
	httpRequest,
	httpRequestBytes,
	httpStreamOpen,
	httpStreamReadChunk,
	httpStreamClose,

	# --- Storage: Filesystem ---
	readFile,
	writeFile,
	readFileBytes,
	writeFileBytes,
	appendFileBytes,

	# --- Storage: Key-Value ---
	kvGet,
	kvSet,
	kvGetBytes,
	kvSetBytes,

	# --- Storage: Blobstore ---
	blobGet,
	blobPut,
	blobGetBytes,
	blobPutBytes,

	# --- Secrets ---
	getSecretHandle,
	revealSecret,
	wrapSecretHandle,
	unwrapSecretHandle,

	# --- Database ---
	dbExecute,
	dbExecuteParams,
	dbQuery,
	dbQueryParams,

	# --- WebSocket ---
	websocketConnect,
	websocketSend,
	websocketReceive,
	websocketClose,

	# --- Concurrency & Time ---
	sleep,
	now,

	# --- Golem Primitives ---
	setPersistence,
	generateIdempotencyKey,
	getOplogIndex,
	oplogCommit,
	getIdempotenceMode,
	setIdempotenceMode,
	revertAgent,
	createPromise,
	completePromise,
	getPromise,
	createWebhook,

	# --- Observability ---
	configGet,
	traceId,
	spanId,
	traceHeaders,
]

import Types exposing [
	Result,
	Metadata,
	MethodDeclaration,
	AgentMode,
	ToolDefinition,
	ToolParameter,
	ToolCall,
	ToolResult,
	PersistenceMode,
	SecretHandle,
	SecretHeader,
	HttpRequest,
	HttpMount,
]
import Effect
import Json as PlatformJson exposing [escape]

## Phantom-typed Method declaration
Method(input, output) : {
	name : Str,
	description : Str,
	promptHint : Str,
}

## Construct a phantom-typed Method
method : { name : Str, description : Str, promptHint : Str } -> Method(_input, _output)
method = |cfg| {
	name: cfg.name,
	description: cfg.description,
	promptHint: cfg.promptHint,
}

## Method handler tracking whether state changed
Handler(state) : {
	declaration : MethodDeclaration,
	invoke : state, Str => Result({ state : state, stateChanged : Bool, out : Str }, Str),
}

## Specification builder for a strongly-typed agent
Spec(state) : {
	name : Str,
	version : Str,
	description : Str,
	mode : AgentMode,
	httpMount : [None, Some(HttpMount)],
	init : Str -> Result(state, Str),
	handlers : List(Handler(state)),
	tools : List(ToolDefinition),
	toolHandler : state, ToolCall => Result({ state : state, stateChanged : Bool, result : ToolResult }, Str),
	serializeState : state -> Str,
	deserializeState : Str -> Result(state, Str),
}

## State-erased agent closure representation
ErasedAgent : {
	metadata : Metadata,
	init : Str -> Result(Str, Str),
	invokeMethod : Str, Str, Str => Result({ nextStateJson : Str, stateChanged : Bool, out : Str }, Str),
	handleToolCall : Str, ToolCall => Result({ nextStateJson : Str, stateChanged : Bool, result : ToolResult }, Str),
	handleMessage : Str, Str => Result({ nextStateJson : Str, stateChanged : Bool, response : Str }, Str),
}

## Component Agent container (holds 1 or N bundled agents)
Agent(state) : {
	agents : List(ErasedAgent),
}

## Unified tool declaration with execution logic
Tool(state) : {
	definition : ToolDefinition,
	execute : state, Str -> Result({ state : state, stateChanged : Bool, output : Str }, Str),
}

## Stateless tool declaration
StatelessTool : {
	definition : ToolDefinition,
	execute : Str -> Result(Str, Str),
}

## Construct a read-only tool that queries state
tool :
	{
		name : Str,
		description : Str,
		params : List(ToolParameter),
		execute : state, Str -> Result(Str, Str),
	} -> Tool(state)
tool = |cfg| {
	definition: {
		name: cfg.name,
		description: cfg.description,
		parameters: cfg.params,
	},
	execute: |st, args|
		match (cfg.execute)(st, args) {
			Ok(output) => Ok({ state: st, stateChanged: False, output })
			Err(err) => Err(err)
		},
}

## Construct a read-only tool that returns JSON
toolJson :
	{
		name : Str,
		description : Str,
		params : List(ToolParameter),
		execute : state, Str -> Result(a, Str),
	} -> Tool(state)
		where [a.Json.Encodable([])]
toolJson = |cfg| {
	definition: {
		name: cfg.name,
		description: cfg.description,
		parameters: cfg.params,
	},
	execute: |st, args|
		match (cfg.execute)(st, args) {
			Ok(val) => Ok({ state: st, stateChanged: False, output: Json.to_str(val) })
			Err(err) => Err(err)
		},
}

## Construct a stateful tool that can mutate state
statefulTool :
	{
		name : Str,
		description : Str,
		params : List(ToolParameter),
		execute : state, Str -> Result({ state : state, stateChanged : Bool, output : Str }, Str),
	} -> Tool(state)
statefulTool = |cfg| {
	definition: {
		name: cfg.name,
		description: cfg.description,
		parameters: cfg.params,
	},
	execute: cfg.execute,
}

## Construct a stateless tool
statelessTool :
	{
		name : Str,
		description : Str,
		params : List(ToolParameter),
		execute : Str -> Result(Str, Str),
	} -> StatelessTool
statelessTool = |cfg| {
	definition: {
		name: cfg.name,
		description: cfg.description,
		parameters: cfg.params,
	},
	execute: cfg.execute,
}

## Construct a stateless tool that returns JSON
statelessToolJson :
	{
		name : Str,
		description : Str,
		params : List(ToolParameter),
		execute : Str -> Result(a, Str),
	} -> StatelessTool
		where [a.Json.Encodable([])]
statelessToolJson = |cfg| {
	definition: {
		name: cfg.name,
		description: cfg.description,
		parameters: cfg.params,
	},
	execute: |args|
		match (cfg.execute)(args) {
			Ok(val) => Ok(Json.to_str(val))
			Err(err) => Err(err)
		},
}

## Lift a StatelessTool into a Tool(state)
statelessToTool : StatelessTool -> Tool(state)
statelessToTool = |st| {
	definition: st.definition,
	execute: |state, args|
		match (st.execute)(args) {
			Ok(output) => Ok({ state, stateChanged: False, output })
			Err(err) => Err(err)
		},
}

## Shorthand constructor for required string tool parameter
param : Str, Str -> ToolParameter
param = |name, description| {
	name,
	description,
	paramType: "string",
	required: True,
}

## Shorthand constructor for optional string tool parameter
optionalParam : Str, Str -> ToolParameter
optionalParam = |name, description| {
	name,
	description,
	paramType: "string",
	required: False,
}

## Construct a numeric Tool parameter
numberParam : Str, Str -> ToolParameter
numberParam = |name, description| {
	name,
	description,
	paramType: "number",
	required: True,
}

## Construct a boolean Tool parameter
boolParam : Str, Str -> ToolParameter
boolParam = |name, description| {
	name,
	description,
	paramType: "boolean",
	required: True,
}

## Construct an arbitrary typed Tool parameter
typedParam : Str, Str, Str -> ToolParameter
typedParam = |name, description, paramType| {
	name,
	description,
	paramType,
	required: True,
}

## Response Primitives
reply : state, Str -> Result({ state : state, stateChanged : Bool, response : Str }, Str)
reply = |nextState, responseText|
	Ok({ state: nextState, stateChanged: True, response: responseText })

readOnly : state, Str -> Result({ state : state, stateChanged : Bool, response : Str }, Str)
readOnly = |currState, responseText|
	Ok({ state: currState, stateChanged: False, response: responseText })

stay : state -> Result({ state : state, stateChanged : Bool, response : Str }, Str)
stay = |currState|
	Ok({ state: currState, stateChanged: False, response: "" })

fail : Str -> Result({ state : state, stateChanged : Bool, response : Str }, Str)
fail = |errMsg|
	Err(errMsg)

replyJson : state, a -> Result({ state : state, stateChanged : Bool, response : Str }, Str) where [a.Json.Encodable([])]
replyJson = |nextState, val|
	Ok({ state: nextState, stateChanged: True, response: Json.to_str(val) })

readOnlyJson : state, a -> Result({ state : state, stateChanged : Bool, response : Str }, Str) where [a.Json.Encodable([])]
readOnlyJson = |currState, val|
	Ok({ state: currState, stateChanged: False, response: Json.to_str(val) })

findTool : List(Tool(state)), Str -> Result(Tool(state), [NotFound])
findTool = |toolList, targetName|
	match toolList {
		[] => Err(NotFound)
		[first, .. as rest] =>
			if first.definition.name == targetName {
				Ok(first)
			} else {
				findTool(rest, targetName)
			}
	}

findStatelessTool : List(StatelessTool), Str -> Result(StatelessTool, [NotFound])
findStatelessTool = |toolList, targetName|
	match toolList {
		[] => Err(NotFound)
		[first, .. as rest] =>
			if first.definition.name == targetName {
				Ok(first)
			} else {
				findStatelessTool(rest, targetName)
			}
	}

## Pure dispatcher for tool invocations
dispatchTool :
	List(Tool(state)), state, ToolCall, (state -> Str) -> Result({ nextStateJson : Str, stateChanged : Bool, result : ToolResult }, Str)
dispatchTool = |tools, st, call, serializeState|
	match findTool(tools, call.name) {
		Err(_) =>
			Ok({
				nextStateJson: "",
				stateChanged: False,
				result: {
					id: call.id,
					success: False,
					output: "Unknown tool '${call.name}'",
				},
			})
		Ok(targetTool) =>
			match (targetTool.execute)(st, call.arguments) {
				Ok(res) => {
					nextStateJson =
						if res.stateChanged {
							serializeState(res.state)
						} else {
							""
						}
					Ok({
						nextStateJson,
						stateChanged: res.stateChanged,
						result: {
							id: call.id,
							success: True,
							output: res.output,
						},
					})
				}
				Err(errMsg) =>
					Ok({
						nextStateJson: "",
						stateChanged: False,
						result: {
							id: call.id,
							success: False,
							output: errMsg,
						},
					})
			}
	}

## Pure dispatcher for message invocations
dispatchMessage :
	(state, Str -> Result({ state : state, stateChanged : Bool, response : Str }, Str)), state, Str, (state -> Str) -> Result({ nextStateJson : Str, stateChanged : Bool, response : Str }, Str)
dispatchMessage = |onMessage, st, msgStr, serializeState|
	match onMessage(st, msgStr) {
		Ok(res) => {
			nextStateJson =
				if res.stateChanged {
					serializeState(res.state)
				} else {
					""
				}
			Ok({
				nextStateJson,
				stateChanged: res.stateChanged,
				response: res.response,
			})
		}
		Err(err) => Err(err)
	}

## Pure dispatcher for stateless tool invocations
dispatchStatelessTool :
	List(StatelessTool), ToolCall -> Result({ nextStateJson : Str, stateChanged : Bool, result : ToolResult }, Str)
dispatchStatelessTool = |tools, call|
	match findStatelessTool(tools, call.name) {
		Err(_) =>
			Ok({
				nextStateJson: "",
				stateChanged: False,
				result: {
					id: call.id,
					success: False,
					output: "Unknown tool '${call.name}'",
				},
			})
		Ok(targetTool) =>
			match (targetTool.execute)(call.arguments) {
				Ok(output) =>
					Ok({
						nextStateJson: "",
						stateChanged: False,
						result: {
							id: call.id,
							success: True,
							output,
						},
					})
				Err(errMsg) =>
					Ok({
						nextStateJson: "",
						stateChanged: False,
						result: {
							id: call.id,
							success: False,
							output: errMsg,
						},
					})
			}
	}

## Canonical Stateful Agent constructor with custom codecs and metadata
agentWith :
	{
		name : Str,
		version : Str,
		description : Str,
		initial : state,
		onMessage : state, Str -> Result({ state : state, stateChanged : Bool, response : Str }, Str),
		tools : List(Tool(state)),
		serializeState : state -> Str,
		deserializeState : Str -> Result(state, Str),
	} -> Agent(state)
agentWith = |cfg| {
	erased : ErasedAgent
	erased = {
		metadata: {
			name: cfg.name,
			version: cfg.version,
			description: cfg.description,
			tools: List.map(cfg.tools, |t| t.definition),
			methods: [],
			mode: Durable,
			httpMount: None,
		},
		init: |configStr|
			if Str.is_empty(configStr) or configStr == "{}" {
				Ok((cfg.serializeState)(cfg.initial))
			} else {
				match (cfg.deserializeState)(configStr) {
					Ok(parsed) => Ok((cfg.serializeState)(parsed))
					Err(e) => Err("init failed to parse config: ${e}")
				}
			},
		invokeMethod: |methodName, stateJson, inputJson|
			match findTool(cfg.tools, methodName) {
				Ok(targetTool) =>
					match (cfg.deserializeState)(stateJson) {
						Err(e) => Err("Failed to deserialize state: ${e}")
						Ok(st) =>
							match (targetTool.execute)(st, inputJson) {
								Ok(res) => {
									nextStateJson =
										if res.stateChanged {
											(cfg.serializeState)(res.state)
										} else {
											""
										}
									Ok({
										nextStateJson,
										stateChanged: res.stateChanged,
										out: res.output,
									})
								}
								Err(err) => Err(err)
							}
					}
				Err(_) =>
					match (cfg.deserializeState)(stateJson) {
						Err(e) => Err("Failed to deserialize state: ${e}")
						Ok(st) =>
							match (cfg.onMessage)(st, inputJson) {
								Ok(res) => {
									nextStateJson =
										if res.stateChanged {
											(cfg.serializeState)(res.state)
										} else {
											""
										}
									Ok({
										nextStateJson,
										stateChanged: res.stateChanged,
										out: res.response,
									})
								}
								Err(err) => Err(err)
							}
					}
			},
		handleToolCall: |stateJson, call|
			match (cfg.deserializeState)(stateJson) {
				Err(e) => Err("Failed to deserialize state: ${e}")
				Ok(st) => dispatchTool(cfg.tools, st, call, cfg.serializeState)
			},
		handleMessage: |stateJson, msgStr|
			match (cfg.deserializeState)(stateJson) {
				Err(e) => Err("Failed to deserialize state: ${e}")
				Ok(st) => dispatchMessage(cfg.onMessage, st, msgStr, cfg.serializeState)
			},
	}
	{ agents: [erased] }
}

## Canonical Stateful Agent constructor with default JSON state serialization
agent :
	{
		name : Str,
		initial : state,
		onMessage : state, Str -> Result({ state : state, stateChanged : Bool, response : Str }, Str),
		tools : List(Tool(state)),
	} -> Agent(state)
		where [state.Json.Encodable([]), state.Json.Parseable([InvalidJson(Str), ..errs])]
agent = |cfg|
	agentWith({
		name: cfg.name,
		version: "1.0.0",
		description: "",
		initial: cfg.initial,
		onMessage: cfg.onMessage,
		tools: cfg.tools,
		serializeState: |st| Json.to_str(st),
		deserializeState: |str|
			match Json.parse(str) {
				Ok(val) => Ok(val)
				Err(InvalidJson(msg)) => Err("Invalid JSON: ${msg}")
				Err(_) => Ok(cfg.initial)
			},
	})

## Canonical Stateful Agent constructor without tools
simpleAgent :
	{
		name : Str,
		initial : state,
		onMessage : state, Str -> Result({ state : state, stateChanged : Bool, response : Str }, Str),
	} -> Agent(state)
		where [state.Json.Encodable([]), state.Json.Parseable([InvalidJson(Str), ..errs])]
simpleAgent = |cfg|
	agent({
		name: cfg.name,
		initial: cfg.initial,
		tools: [],
		onMessage: cfg.onMessage,
	})

## Canonical Ephemeral Agent constructor with tools
ephemeral :
	{
		name : Str,
		onMessage : Str -> Result(Str, Str),
		tools : List(StatelessTool),
	} -> Agent({})
ephemeral = |cfg| {
	erased : ErasedAgent
	erased = {
		metadata: {
			name: cfg.name,
			version: "1.0.0",
			description: "",
			tools: List.map(cfg.tools, |t| t.definition),
			methods: [],
			mode: Ephemeral,
			httpMount: None,
		},
		init: |_| Ok("{}"),
		invokeMethod: |methodName, _stateJson, inputJson|
			match findStatelessTool(cfg.tools, methodName) {
				Ok(targetTool) =>
					match (targetTool.execute)(inputJson) {
						Ok(out) => Ok({ nextStateJson: "", stateChanged: False, out })
						Err(err) => Err(err)
					}
				Err(_) =>
					match (cfg.onMessage)(inputJson) {
						Ok(out) => Ok({ nextStateJson: "", stateChanged: False, out })
						Err(err) => Err(err)
					}
			},
		handleToolCall: |_stateJson, call|
			dispatchStatelessTool(cfg.tools, call),
		handleMessage: |_stateJson, msgStr|
			match (cfg.onMessage)(msgStr) {
				Ok(response) => Ok({ nextStateJson: "", stateChanged: False, response })
				Err(err) => Err(err)
			},
	}
	{ agents: [erased] }
}

## Canonical Ephemeral Agent constructor without tools
simpleEphemeral :
	{
		name : Str,
		onMessage : Str -> Result(Str, Str),
	} -> Agent({})
simpleEphemeral = |cfg|
	ephemeral({
		name: cfg.name,
		onMessage: cfg.onMessage,
		tools: [],
	})

## Define a stateful (Durable) agent
define :
	{ name : Str, version : Str, description : Str }, (Str -> Result(state, Str)), (state -> Str), (Str -> Result(state, Str)) -> Spec(state)
define = |meta, initFn, serializeFn, deserializeFn| {
	name: meta.name,
	version: meta.version,
	description: meta.description,
	mode: Durable,
	httpMount: None,
	init: initFn,
	handlers: [],
	tools: [],
	toolHandler: |st, call|
		Ok({
			state: st,
			stateChanged: False,
			result: { id: call.id, success: False, output: "Tool not found: ${call.name}" },
		}),
	serializeState: serializeFn,
	deserializeState: deserializeFn,
}

## Define an ephemeral (stateless) agent
defineEphemeral :
	{ name : Str, version : Str, description : Str } -> Spec({})
defineEphemeral = |meta| {
	name: meta.name,
	version: meta.version,
	description: meta.description,
	mode: Ephemeral,
	httpMount: None,
	init: |_| Ok({}),
	handlers: [],
	tools: [],
	toolHandler: |st, call|
		Ok({
			state: st,
			stateChanged: False,
			result: { id: call.id, success: False, output: "Tool not found: ${call.name}" },
		}),
	serializeState: |_| "{}",
	deserializeState: |_| Ok({}),
}

## Attach HTTP mount configuration to agent spec
withHttpMount : Spec(state), HttpMount -> Spec(state)
withHttpMount = |spec, mount| { ..spec, httpMount: Some(mount) }

## Attach tool definitions to agent spec
withTools : Spec(state), List(ToolDefinition) -> Spec(state)
withTools = |spec, tools| { ..spec, tools }

## Attach stateful tool handler
withToolHandler : Spec(state), (state, ToolCall -> Result({ state : state, result : ToolResult }, Str)) -> Spec(state)
withToolHandler = |spec, handler| {
	..spec,
	toolHandler: |st, call|
		match handler(st, call) {
			Ok({ state: nextSt, result }) =>
				Ok({ state: nextSt, stateChanged: True, result })
			Err(e) => Err(e)
		},
}

## Attach stateless tool handler
withStatelessToolHandler : Spec(state), (ToolCall -> Result(ToolResult, Str)) -> Spec(state)
withStatelessToolHandler = |spec, handler| {
	..spec,
	toolHandler: |st, call|
		match handler(call) {
			Ok(result) =>
				Ok({ state: st, stateChanged: False, result })
			Err(e) => Err(e)
		},
}

## Attach effectful stateful tool handler
withEffectfulToolHandler : Spec(state), (state, ToolCall -> Effect.Effect(Result({ state : state, result : ToolResult }, Str))) -> Spec(state)
withEffectfulToolHandler = |spec, handler| {
	..spec,
	toolHandler: |st, call| {
		eff = handler(st, call)
		match Effect.run!(eff) {
			Ok({ state: nextSt, result }) =>
				Ok({ state: nextSt, stateChanged: True, result })
			Err(e) => Err(e)
		}
	},
}

## Attach effectful stateless tool handler
withEffectfulStatelessToolHandler : Spec(state), (ToolCall -> Effect.Effect(Result(ToolResult, Str))) -> Spec(state)
withEffectfulStatelessToolHandler = |spec, handler| {
	..spec,
	toolHandler: |st, call| {
		eff = handler(call)
		match Effect.run!(eff) {
			Ok(result) =>
				Ok({ state: st, stateChanged: False, result })
			Err(e) => Err(e)
		}
	},
}

## Register a mutating method
register : Spec(state), Method(_in, _out), (state, Str -> Result({ state : state, out : Str }, Str)) -> Spec(state)
register = |spec, m, fn| {
	decl = {
		name: m.name,
		description: m.description,
		promptHint: m.promptHint,
		readOnly: False,
	}
	handler : Handler(state)
	handler = {
		declaration: decl,
		invoke: |st, inp|
			match fn(st, inp) {
				Ok({ state: nextSt, out }) =>
					Ok({ state: nextSt, stateChanged: True, out })
				Err(e) => Err(e)
			},
	}
	{ ..spec, handlers: List.append(spec.handlers, handler) }
}

## Register a read-only method (stateChanged: False, zero state serialization)
registerReadOnly : Spec(state), Method(_in, _out), (state, Str -> Result(Str, Str)) -> Spec(state)
registerReadOnly = |spec, m, fn| {
	decl = {
		name: m.name,
		description: m.description,
		promptHint: m.promptHint,
		readOnly: True,
	}
	handler : Handler(state)
	handler = {
		declaration: decl,
		invoke: |st, inp|
			match fn(st, inp) {
				Ok(out) =>
					Ok({ state: st, stateChanged: False, out })
				Err(e) => Err(e)
			},
	}
	{ ..spec, handlers: List.append(spec.handlers, handler) }
}

## Register a stateless method on an ephemeral agent
registerEphemeral : Spec({}), Method(_in, _out), (Str -> Result(Str, Str)) -> Spec({})
registerEphemeral = |spec, m, fn| {
	decl = {
		name: m.name,
		description: m.description,
		promptHint: m.promptHint,
		readOnly: False,
	}
	handler : Handler({})
	handler = {
		declaration: decl,
		invoke: |st, inp|
			match fn(inp) {
				Ok(out) =>
					Ok({ state: st, stateChanged: False, out })
				Err(e) => Err(e)
			},
	}
	{ ..spec, handlers: List.append(spec.handlers, handler) }
}

## Register a typed mutating method with custom codec
registerTyped :
	Spec(state), Method(inp, out), (state, inp -> Result({ state : state, out : out }, Str)), (Str -> Result(inp, Str)), (out -> Str) -> Spec(state)
registerTyped = |spec, m, fn, decodeInput, encodeOutput| {
	decl = {
		name: m.name,
		description: m.description,
		promptHint: m.promptHint,
		readOnly: False,
	}
	handler : Handler(state)
	handler = {
		declaration: decl,
		invoke: |st, rawInp|
			match decodeInput(rawInp) {
				Err(e) => Err("invalid input for ${m.name}: ${e}")
				Ok(parsedInp) =>
					match fn(st, parsedInp) {
						Ok({ state: nextSt, out }) =>
							Ok({ state: nextSt, stateChanged: True, out: encodeOutput(out) })
						Err(e) => Err(e)
					}
			},
	}
	{ ..spec, handlers: List.append(spec.handlers, handler) }
}

## Register a typed read-only method with custom codec
registerReadOnlyTyped :
	Spec(state), Method(inp, out), (state, inp -> Result(out, Str)), (Str -> Result(inp, Str)), (out -> Str) -> Spec(state)
registerReadOnlyTyped = |spec, m, fn, decodeInput, encodeOutput| {
	decl = {
		name: m.name,
		description: m.description,
		promptHint: m.promptHint,
		readOnly: True,
	}
	handler : Handler(state)
	handler = {
		declaration: decl,
		invoke: |st, rawInp|
			match decodeInput(rawInp) {
				Err(e) => Err("invalid input for ${m.name}: ${e}")
				Ok(parsedInp) =>
					match fn(st, parsedInp) {
						Ok(out) =>
							Ok({ state: st, stateChanged: False, out: encodeOutput(out) })
						Err(e) => Err(e)
					}
			},
	}
	{ ..spec, handlers: List.append(spec.handlers, handler) }
}

## Register a typed ephemeral method with custom codec
registerEphemeralTyped :
	Spec({}), Method(inp, out), (inp -> Result(out, Str)), (Str -> Result(inp, Str)), (out -> Str) -> Spec({})
registerEphemeralTyped = |spec, m, fn, decodeInput, encodeOutput| {
	decl = {
		name: m.name,
		description: m.description,
		promptHint: m.promptHint,
		readOnly: False,
	}
	handler : Handler({})
	handler = {
		declaration: decl,
		invoke: |st, rawInp|
			match decodeInput(rawInp) {
				Err(e) => Err("invalid input for ${m.name}: ${e}")
				Ok(parsedInp) =>
					match fn(parsedInp) {
						Ok(out) =>
							Ok({ state: st, stateChanged: False, out: encodeOutput(out) })
						Err(e) => Err(e)
					}
			},
	}
	{ ..spec, handlers: List.append(spec.handlers, handler) }
}

## Register a mutating method with an effectful handler
registerEffectful :
	Spec(state), Method(_in, _out), (state, Str -> Effect.Effect(Result({ state : state, out : Str }, Str))) -> Spec(state)
registerEffectful = |spec, m, fn| {
	decl = {
		name: m.name,
		description: m.description,
		promptHint: m.promptHint,
		readOnly: False,
	}
	handler : Handler(state)
	handler = {
		declaration: decl,
		invoke: |st, rawInp| {
			eff = fn(st, rawInp)
			match Effect.run!(eff) {
				Ok({ state: nextSt, out }) =>
					Ok({ state: nextSt, stateChanged: True, out })
				Err(e) => Err(e)
			}
		},
	}
	{ ..spec, handlers: List.append(spec.handlers, handler) }
}

## Register a typed mutating method with an effectful handler and custom codec
registerEffectfulTyped :
	Spec(state), Method(inp, out), (state, inp -> Effect.Effect(Result({ state : state, out : out }, Str))), (Str -> Result(inp, Str)), (out -> Str) -> Spec(state)
registerEffectfulTyped = |spec, m, fn, decodeInput, encodeOutput| {
	decl = {
		name: m.name,
		description: m.description,
		promptHint: m.promptHint,
		readOnly: False,
	}
	handler : Handler(state)
	handler = {
		declaration: decl,
		invoke: |st, rawInp|
			match decodeInput(rawInp) {
				Err(e) => Err("invalid input for ${m.name}: ${e}")
				Ok(parsedInp) => {
					eff = fn(st, parsedInp)
					match Effect.run!(eff) {
						Ok({ state: nextSt, out }) =>
							Ok({ state: nextSt, stateChanged: True, out: encodeOutput(out) })
						Err(e) => Err(e)
					}
				}
			},
	}
	{ ..spec, handlers: List.append(spec.handlers, handler) }
}

## Build an Agent from a Spec (type erasure into ErasedAgent)
build : Spec(state) -> Agent(state)
build = |spec| {
	erased : ErasedAgent
	erased = {
		metadata: {
			name: spec.name,
			version: spec.version,
			description: spec.description,
			tools: spec.tools,
			methods: List.map(spec.handlers, |h| h.declaration),
			mode: spec.mode,
			httpMount: spec.httpMount,
		},
		init: |configStr|
			match (spec.init)(configStr) {
				Ok(initState) =>
					if spec.mode == Ephemeral {
						Ok("{}")
					} else {
						Ok((spec.serializeState)(initState))
					}
				Err(err) => Err(err)
			},
		invokeMethod: |methodName, stateJson, inputJson| {
			match findHandler(spec.handlers, methodName) {
				Err(err) => Err(err)
				Ok(handler) => {
					stateRes =
						if spec.mode == Ephemeral {
							(spec.deserializeState)("{}")
						} else {
							(spec.deserializeState)(stateJson)
						}
					match stateRes {
						Err(err) => Err("Failed to deserialize state: ${err}")
						Ok(currentState) =>
							match (handler.invoke)(currentState, inputJson) {
								Err(err) => Err(err)
								Ok(res) => {
									nextStateJson =
										if res.stateChanged and spec.mode == Durable {
											(spec.serializeState)(res.state)
										} else {
											""
										}
									Ok({
										nextStateJson,
										stateChanged: res.stateChanged,
										out: res.out,
									})
								}
							}
					}
				}
			}
		},
		handleToolCall: |stateJson, call| {
			stateRes =
				if spec.mode == Ephemeral {
					(spec.deserializeState)("{}")
				} else {
					(spec.deserializeState)(stateJson)
				}
			match stateRes {
				Err(err) => Err("Failed to deserialize state: ${err}")
				Ok(currentState) =>
					match (spec.toolHandler)(currentState, call) {
						Err(err) => Err(err)
						Ok(res) => {
							nextStateJson =
								if res.stateChanged and spec.mode == Durable {
									(spec.serializeState)(res.state)
								} else {
									""
								}
							Ok({
								nextStateJson,
								stateChanged: res.stateChanged,
								result: res.result,
							})
						}
					}
			}
		},
		handleMessage: |stateJson, msgStr| {
			match findHandler(spec.handlers, "handle-message") {
				Ok(handler) => {
					stateRes =
						if spec.mode == Ephemeral {
							(spec.deserializeState)("{}")
						} else {
							(spec.deserializeState)(stateJson)
						}
					match stateRes {
						Err(err) => Err("Failed to deserialize state: ${err}")
						Ok(currentState) =>
							match (handler.invoke)(currentState, msgStr) {
								Err(err) => Err(err)
								Ok(res) => {
									nextStateJson =
										if res.stateChanged and spec.mode == Durable {
											(spec.serializeState)(res.state)
										} else {
											""
										}
									Ok({
										nextStateJson,
										stateChanged: res.stateChanged,
										response: res.out,
									})
								}
							}
					}
				}
				Err(_) =>
					Err("No handle-message handler registered for this agent. Register one or use Golem.agent/simpleAgent instead.")
			}
		},
	}
	{ agents: [erased] }
}

## Bundle multiple agents into a single Wasm component
bundle : List(Agent({})) -> Agent({})
bundle = |agentList| {
	flattened = List.join_map(agentList, |a| a.agents)
	{ agents: flattened }
}

## Synonym for bundle
bundleAll : List(Agent({})) -> Agent({})
bundleAll = bundle

bundle2 : Agent(_), Agent(_) -> Agent({})
bundle2 = |a, b| {
	{ agents: List.concat(a.agents, b.agents) }
}

erase : Agent(_) -> Agent({})
erase = |a| { agents: a.agents }

## ⚠️ Backwards-compatible legacy single-agent definition.
## Note: This shim always sets `stateChanged: True` on every `handleMessage`
## call, which forces a full state serialisation even for read-only queries.
## For zero-cost read-only bypass, migrate to `Golem.agent` with `readOnly`.
defineAgent :
	{
		init : Str -> Result(state, Str),
		handleMessage : state, Str -> Result({ state : state, response : Str }, Str),
		handleToolCall : state, ToolCall -> Result({ state : state, result : ToolResult }, Str),
		metadata : {
			name : Str,
			version : Str,
			description : Str,
			tools : List(ToolDefinition),
		},
		serializeState : state -> Str,
		deserializeState : Str -> Result(state, Str),
	} -> Agent(state)
defineAgent = |config| {
	erased : ErasedAgent
	erased = {
		metadata: {
			name: config.metadata.name,
			version: config.metadata.version,
			description: config.metadata.description,
			tools: config.metadata.tools,
			methods: [],
			mode: Durable,
			httpMount: None,
		},
		init: |configStr|
			match (config.init)(configStr) {
				Ok(st) => Ok((config.serializeState)(st))
				Err(e) => Err(e)
			},
		invokeMethod: |_methodName, stateJson, inputJson|
			match (config.deserializeState)(stateJson) {
				Err(e) => Err("Failed to deserialize state: ${e}")
				Ok(st) =>
					match (config.handleMessage)(st, inputJson) {
						Ok({ state: nextSt, response }) =>
							Ok({
								nextStateJson: (config.serializeState)(nextSt),
								stateChanged: True,
								out: response,
							})
						Err(e) => Err(e)
					}
			},
		handleToolCall: |stateJson, call|
			match (config.deserializeState)(stateJson) {
				Err(e) => Err("Failed to deserialize state: ${e}")
				Ok(st) =>
					match (config.handleToolCall)(st, call) {
						Ok({ state: nextSt, result }) =>
							Ok({
								nextStateJson: (config.serializeState)(nextSt),
								stateChanged: True,
								result,
							})
						Err(e) => Err(e)
					}
			},
		handleMessage: |stateJson, msgStr|
			match (config.deserializeState)(stateJson) {
				Err(e) => Err("Failed to deserialize state: ${e}")
				Ok(st) =>
					match (config.handleMessage)(st, msgStr) {
						Ok({ state: nextSt, response }) =>
							Ok({
								nextStateJson: (config.serializeState)(nextSt),
								stateChanged: True,
								response,
							})
						Err(e) => Err(e)
					}
			},
	}
	{ agents: [erased] }
}

findHandler : List(Handler(state)), Str -> Result(Handler(state), Str)
findHandler = |handlers, targetName| {
	match handlers {
		[] => Err("Method '${targetName}' not found")
		[first, .. as rest] =>
			if first.declaration.name == targetName {
				Ok(first)
			} else {
				findHandler(rest, targetName)
			}
	}
}

## Logging functions
logTrace : Str -> Effect.Effect({})
logTrace = |msg| Effect.log(Trace, msg)

logDebug : Str -> Effect.Effect({})
logDebug = |msg| Effect.log(Debug, msg)

logInfo : Str -> Effect.Effect({})
logInfo = |msg| Effect.log(Info, msg)

logWarn : Str -> Effect.Effect({})
logWarn = |msg| Effect.log(Warn, msg)

logError : Str -> Effect.Effect({})
logError = |msg| Effect.log(Error, msg)

## Get current Golem Worker ID
workerId : Effect.Effect(Str)
workerId = Effect.getWorkerId

## Typed Remote Procedure Call with explicit target instance constructor
rpcInstance : Str, Str, Method(_in, _out), Str -> Effect.Effect(Result(Str, Str))
rpcInstance = |targetWorker, constructor, m, payload|
	Effect.rpcInvoke(targetWorker, constructor, m.name, payload)

rpc : Str, Method(_in, _out), Str -> Effect.Effect(Result(Str, Str))
rpc = |targetWorker, m, payload|
	rpcInstance(targetWorker, "{}", m, payload)

rpcScheduleInstance : Str, Str, Method(_in, _out), Str, U64 -> Effect.Effect(Result(Str, Str))
rpcScheduleInstance = |targetWorker, constructor, m, payload, delayMillis|
	Effect.rpcSchedule(targetWorker, constructor, m.name, payload, delayMillis)

rpcSchedule : Str, Method(_in, _out), Str, U64 -> Effect.Effect(Result(Str, Str))
rpcSchedule = |targetWorker, m, payload, delayMillis|
	rpcScheduleInstance(targetWorker, "{}", m, payload, delayMillis)

rpcFireAndForgetInstance : Str, Str, Method(_in, _out), Str -> Effect.Effect(Result(Str, Str))
rpcFireAndForgetInstance = |targetWorker, constructor, m, payload|
	Effect.rpcFireAndForget(targetWorker, constructor, m.name, payload)

rpcFireAndForget : Str, Method(_in, _out), Str -> Effect.Effect(Result(Str, Str))
rpcFireAndForget = |targetWorker, m, payload|
	rpcFireAndForgetInstance(targetWorker, "{}", m, payload)

## Binary Remote Procedure Call with explicit constructor
rpcBytesInstance : Str, Str, Method(_in, _out), List(U8) -> Effect.Effect(Result(List(U8), Str))
rpcBytesInstance = |targetWorker, constructor, m, payload|
	Effect.rpcInvokeBytes(targetWorker, constructor, m.name, payload)

rpcBytes : Str, Method(_in, _out), List(U8) -> Effect.Effect(Result(List(U8), Str))
rpcBytes = |targetWorker, m, payload|
	rpcBytesInstance(targetWorker, "{}", m, payload)

rpcScheduleBytesInstance : Str, Str, Method(_in, _out), List(U8), U64 -> Effect.Effect(Result(List(U8), Str))
rpcScheduleBytesInstance = |targetWorker, constructor, m, payload, delayMillis|
	Effect.rpcScheduleBytes(targetWorker, constructor, m.name, payload, delayMillis)

rpcScheduleBytes : Str, Method(_in, _out), List(U8), U64 -> Effect.Effect(Result(List(U8), Str))
rpcScheduleBytes = |targetWorker, m, payload, delayMillis|
	rpcScheduleBytesInstance(targetWorker, "{}", m, payload, delayMillis)

rpcFireAndForgetBytesInstance : Str, Str, Method(_in, _out), List(U8) -> Effect.Effect(Result({}, Str))
rpcFireAndForgetBytesInstance = |targetWorker, constructor, m, payload|
	Effect.rpcFireAndForgetBytes(targetWorker, constructor, m.name, payload)

rpcFireAndForgetBytes : Str, Method(_in, _out), List(U8) -> Effect.Effect(Result({}, Str))
rpcFireAndForgetBytes = |targetWorker, m, payload|
	rpcFireAndForgetBytesInstance(targetWorker, "{}", m, payload)

## Typed Remote Procedure Call with input and output codecs and explicit constructor
rpcTypedInstance :
	Str, Str, Method(inp, out), inp, (inp -> Str), (Str -> Result(out, Str)) -> Effect.Effect(Result(out, Str))
rpcTypedInstance = |targetWorker, constructor, m, inputVal, encodeInput, decodeOutput| {
	payload = encodeInput(inputVal)
	Effect.map(
		Effect.rpcInvoke(targetWorker, constructor, m.name, payload),
		|res|
			match res {
				Ok(resp) => decodeOutput(resp)
				Err(err) => Err(err)
			},
	)
}

rpcTyped :
	Str, Method(inp, out), inp, (inp -> Str), (Str -> Result(out, Str)) -> Effect.Effect(Result(out, Str))
rpcTyped = |targetWorker, m, inputVal, encodeInput, decodeOutput|
	rpcTypedInstance(targetWorker, "{}", m, inputVal, encodeInput, decodeOutput)

## Scheduled Typed RPC with input codec and explicit constructor
rpcScheduleTypedInstance :
	Str, Str, Method(inp, _out), inp, (inp -> Str), U64 -> Effect.Effect(Result(Str, Str))
rpcScheduleTypedInstance = |targetWorker, constructor, m, inputVal, encodeInput, delayMillis| {
	payload = encodeInput(inputVal)
	Effect.rpcSchedule(targetWorker, constructor, m.name, payload, delayMillis)
}

rpcScheduleTyped :
	Str, Method(inp, _out), inp, (inp -> Str), U64 -> Effect.Effect(Result(Str, Str))
rpcScheduleTyped = |targetWorker, m, inputVal, encodeInput, delayMillis|
	rpcScheduleTypedInstance(targetWorker, "{}", m, inputVal, encodeInput, delayMillis)

## Fire-and-forget Typed RPC with input codec and explicit constructor
rpcFireAndForgetTypedInstance :
	Str, Str, Method(inp, _out), inp, (inp -> Str) -> Effect.Effect(Result(Str, Str))
rpcFireAndForgetTypedInstance = |targetWorker, constructor, m, inputVal, encodeInput| {
	payload = encodeInput(inputVal)
	Effect.rpcFireAndForget(targetWorker, constructor, m.name, payload)
}

rpcFireAndForgetTyped :
	Str, Method(inp, _out), inp, (inp -> Str) -> Effect.Effect(Result(Str, Str))
rpcFireAndForgetTyped = |targetWorker, m, inputVal, encodeInput|
	rpcFireAndForgetTypedInstance(targetWorker, "{}", m, inputVal, encodeInput)

## Set Golem Persistence Mode
setPersistence : PersistenceMode -> Effect.Effect({})
setPersistence = |mode|
	Effect.setPersistenceMode(mode)

## Perform outgoing HTTP request with raw JSON string
http : Str -> Effect.Effect(Result(Str, Str))
http = |reqJson|
	Effect.httpRequest(reqJson)

encodeHttpRequest : HttpRequest -> Str
encodeHttpRequest = |req| {
	headersList =
		req.headers
			|> List.map(|h| "{\"name\":\"${escape(h.name)}\",\"value\":\"${escape(h.value)}\"}")
			|> Str.join_with(",")
	secretHeadersList =
		req.secretHeaders
			|> List.map(
				|sh| {
					hVal = Types.unwrapSecretHandle(sh.handle)
					"{\"name\":\"${escape(sh.name)}\",\"prefix\":\"${escape(sh.prefix)}\",\"handle\":${Str.inspect(hVal)}}"
				},
			)
			|> Str.join_with(",")
	"{\"url\":\"${escape(req.url)}\",\"method\":\"${escape(req.method)}\",\"headers\":[${headersList}],\"body\":\"${escape(req.body)}\",\"secretHeaders\":[${secretHeadersList}]}"
}

## Perform an outgoing HTTP request using a typed HttpRequest record.
## For raw JSON control use `Golem.http`.
httpRequest : HttpRequest -> Effect.Effect(Result(Str, Str))
httpRequest = |req|
	Effect.httpRequest(encodeHttpRequest(req))

## Perform binary HTTP request
httpRequestBytes : Str, List(U8) -> Effect.Effect(Result(List(U8), Str))
httpRequestBytes = |reqJson, bodyBytes|
	Effect.httpRequestBytes(reqJson, bodyBytes)

## Read raw bytes from durable filesystem without lossy UTF-8 conversion
readFileBytes : Str -> Effect.Effect(Result(List(U8), Str))
readFileBytes = |path|
	Effect.readFileBytes(path)

## Write raw bytes to durable filesystem
writeFileBytes : Str, List(U8) -> Effect.Effect(Result({}, Str))
writeFileBytes = |path, bytes|
	Effect.writeFileBytes(path, bytes)

## Append raw bytes to durable filesystem
appendFileBytes : Str, List(U8) -> Effect.Effect(Result({}, Str))
appendFileBytes = |path, bytes|
	Effect.appendFileBytes(path, bytes)

## Read text file from durable filesystem
readFile : Str -> Effect.Effect(Result(Str, Str))
readFile = |path|
	Effect.readFile(path)

## Write text file to durable filesystem
writeFile : Str, Str -> Effect.Effect(Result({}, Str))
writeFile = |path, content|
	Effect.writeFile(path, content)

## Read raw binary bytes from Key-Value store
kvGetBytes : Str, Str -> Effect.Effect(Result(List(U8), Str))
kvGetBytes = |bucket, key|
	Effect.kvGetBytes(bucket, key)

## Write raw binary bytes to Key-Value store
kvSetBytes : Str, Str, List(U8) -> Effect.Effect(Result({}, Str))
kvSetBytes = |bucket, key, bytes|
	Effect.kvSetBytes(bucket, key, bytes)

## Read text from Key-Value store
kvGet : Str, Str -> Effect.Effect(Result(Str, Str))
kvGet = |bucket, key|
	Effect.kvGet(bucket, key)

## Write text to Key-Value store
kvSet : Str, Str, Str -> Effect.Effect(Result({}, Str))
kvSet = |bucket, key, val|
	Effect.kvSet(bucket, key, val)

## Read raw binary bytes from Blobstore
blobGetBytes : Str, Str -> Effect.Effect(Result(List(U8), Str))
blobGetBytes = |container, object|
	Effect.blobGetBytes(container, object)

## Write raw binary bytes to Blobstore
blobPutBytes : Str, Str, List(U8) -> Effect.Effect(Result({}, Str))
blobPutBytes = |container, object, bytes|
	Effect.blobPutBytes(container, object, bytes)

## Read text from Blobstore
blobGet : Str, Str -> Effect.Effect(Result(Str, Str))
blobGet = |container, object|
	Effect.blobGet(container, object)

## Write text to Blobstore
blobPut : Str, Str, Str -> Effect.Effect(Result({}, Str))
blobPut = |container, object, val|
	Effect.blobPut(container, object, val)

## Connect to a WebSocket endpoint
websocketConnect : Str -> Effect.Effect(Result(U32, Str))
websocketConnect = |url|
	Effect.wsConnect(url)

## Send text message over WebSocket connection
websocketSend : U32, Str -> Effect.Effect(Result({}, Str))
websocketSend = |handle, msg|
	Effect.wsSend(handle, msg)

## Receive text message from WebSocket connection
websocketReceive : U32 -> Effect.Effect(Result(Str, Str))
websocketReceive = |handle|
	Effect.wsReceive(handle)

## Close WebSocket connection
websocketClose : U32 -> Effect.Effect(Result({}, Str))
websocketClose = |handle|
	Effect.wsClose(handle)

## Durable sleep for specified milliseconds
sleep : U64 -> Effect.Effect({})
sleep = |millis|
	Effect.sleepMillis(millis)

## Get current timestamp in milliseconds
now : Effect.Effect(U64)
now = Effect.nowMillis

wrapSecretHandle : U32 -> SecretHandle
wrapSecretHandle = |raw| Types.wrapSecretHandle(raw)

unwrapSecretHandle : SecretHandle -> U32
unwrapSecretHandle = |handle| Types.unwrapSecretHandle(handle)

## Acquire an opaque secret handle from the host environment without leaking plaintext into guest memory
getSecretHandle : Str -> Effect.Effect(Result(SecretHandle, Str))
getSecretHandle = |name|
	Effect.map(
		Effect.getSecretHandle(name),
		|res|
			match res {
				Ok(raw) => Ok(wrapSecretHandle(raw))
				Err(err) => Err(err)
			},
	)

## Explicitly reveal secret plaintext into guest memory
revealSecret : SecretHandle -> Effect.Effect(Result(Str, Str))
revealSecret = |handle|
	Effect.revealSecret(unwrapSecretHandle(handle))

## Generate a unique idempotency key
generateIdempotencyKey : Effect.Effect(Result(Str, Str))
generateIdempotencyKey =
	Effect.generateIdempotencyKey

## Get the current oplog index.
##
## **⚠️ Wasm32 limitation:** The Golem 1.6 WIT interface (`golem:api/host@1.5.0`)
## does not expose an oplog-index query function, so this always returns `0`
## when compiled for Golem Cloud. The value is non-zero only in the native
## host test harness (returns `1` to allow unit tests to progress).
##
## Because `revertAgent` accepts the index returned by this function, calling
## `getOplogIndex |> await |> revertAgent` on Golem Cloud will always revert
## the worker back to the **very beginning** of its oplog. Use `revertAgent`
## only when you have an index obtained from a prior snapshot or from an
## oplog-processor callback — not from this function.
##
## Once Golem exposes `get-oplog-index` in a future WIT revision, this function
## will return the live index automatically and this warning will be removed.
getOplogIndex : Effect.Effect(U64)
getOplogIndex =
	Effect.getOplogIndex

## Commit oplog up to current index with specified replica quorum
oplogCommit : U8 -> Effect.Effect({})
oplogCommit = |replicas|
	Effect.oplogCommit(replicas)

## Create a durable promise
createPromise : Effect.Effect(Result(U64, Str))
createPromise =
	Effect.createPromise

## Complete a durable promise with payload bytes
completePromise : U64, List(U8) -> Effect.Effect(Result(Bool, Str))
completePromise = |handle, data|
	Effect.completePromise(handle, data)

## Await and retrieve a durable promise's payload bytes
getPromise : U64 -> Effect.Effect(Result(List(U8), Str))
getPromise = |handle|
	Effect.getPromise(handle)

## Create an external webhook URL associated with a durable promise
createWebhook : U64 -> Effect.Effect(Result(Str, Str))
createWebhook = |handle|
	Effect.createWebhook(handle)

## Get dynamic configuration value from configuration store
configGet : Str -> Effect.Effect(Result(Str, Str))
configGet = |key|
	Effect.configGet(key)

## Get current distributed trace ID
traceId : Effect.Effect(Str)
traceId =
	Effect.traceId

## Get current distributed span ID
spanId : Effect.Effect(Str)
spanId =
	Effect.spanId

## Get W3C trace context headers as JSON string
traceHeaders : Effect.Effect(Result(Str, Str))
traceHeaders =
	Effect.traceHeaders

## Get current idempotence mode
getIdempotenceMode : Effect.Effect(Bool)
getIdempotenceMode =
	Effect.getIdempotenceMode

## Set current idempotence mode
setIdempotenceMode : Bool -> Effect.Effect({})
setIdempotenceMode = |mode|
	Effect.setIdempotenceMode(mode)

## Revert agent execution state back to a specified oplog index.
##
## **⚠️ Do not pass the value of `getOplogIndex` as this index on Golem Cloud.**
## Because `getOplogIndex` always returns `0` on wasm32 (see its documentation),
## doing so will revert the worker to the **beginning of its oplog**, erasing
## all state. Only call `revertAgent` with an index from a known-good snapshot,
## from an oplog-processor callback, or from a value you saved before the
## operation you want to roll back.
revertAgent : U64 -> Effect.Effect(Result({}, Str))
revertAgent = |oplogIndex|
	Effect.revertAgent(oplogIndex)

## Execute a SQL statement on PostgreSQL or MySQL database
dbExecute : Str, Str -> Effect.Effect(Result(U64, Str))
dbExecute = |connStr, sql|
	Effect.dbExecute(connStr, sql)

## Execute a parameterized SQL statement on PostgreSQL or MySQL database with JSON parameters
dbExecuteParams : Str, Str, Str -> Effect.Effect(Result(U64, Str))
dbExecuteParams = |connStr, sql, paramsJson|
	Effect.dbExecuteParams(connStr, sql, paramsJson)

## Execute a SQL query on PostgreSQL or MySQL database returning JSON results
dbQuery : Str, Str -> Effect.Effect(Result(Str, Str))
dbQuery = |connStr, sql|
	Effect.dbQuery(connStr, sql)

## Execute a parameterized SQL query on PostgreSQL or MySQL database returning JSON results
dbQueryParams : Str, Str, Str -> Effect.Effect(Result(Str, Str))
dbQueryParams = |connStr, sql, paramsJson|
	Effect.dbQueryParams(connStr, sql, paramsJson)

## Open a chunked streaming HTTP response stream (returns stream handle)
httpStreamOpen : Str, List(U8) -> Effect.Effect(Result(U32, Str))
httpStreamOpen = |reqJson, bodyBytes|
	Effect.httpStreamOpen(reqJson, bodyBytes)

## Read up to maxBytes chunk from an active HTTP response stream (empty list signifies EOF)
httpStreamReadChunk : U32, U64 -> Effect.Effect(Result(List(U8), Str))
httpStreamReadChunk = |handle, maxBytes|
	Effect.httpStreamReadChunk(handle, maxBytes)

## Close an active HTTP response stream handle
httpStreamClose : U32 -> Effect.Effect(Result({}, Str))
httpStreamClose = |handle|
	Effect.httpStreamClose(handle)
