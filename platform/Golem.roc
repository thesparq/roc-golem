module [
	Agent,
	ErasedAgent,
	Method,
	Spec,
	Handler,
	method,
	define,
	defineEphemeral,
	withTools,
	withToolHandler,
	withStatelessToolHandler,
	withEffectfulToolHandler,
	withEffectfulStatelessToolHandler,
	register,
	registerReadOnly,
	registerEphemeral,
	registerTyped,
	registerReadOnlyTyped,
	registerEphemeralTyped,
	registerEffectful,
	registerEffectfulTyped,
	build,
	bundle,
	defineAgent,
	logTrace,
	logDebug,
	logInfo,
	logWarn,
	logError,
	workerId,
	rpc,
	rpcSchedule,
	rpcFireAndForget,
	rpcTyped,
	rpcScheduleTyped,
	rpcFireAndForgetTyped,
	setPersistence,
	http,
	httpRequestBytes,
	readFileBytes,
	writeFileBytes,
	appendFileBytes,
	readFile,
	writeFile,
	kvGetBytes,
	kvSetBytes,
	kvGet,
	kvSet,
	blobGetBytes,
	blobPutBytes,
	blobGet,
	blobPut,
	websocketConnect,
	websocketSend,
	websocketReceive,
	websocketClose,
	sleep,
	now,
	SecretHandle,
	SecretHeader,
	getSecretHandle,
	revealSecret,
	generateIdempotencyKey,
	getOplogIndex,
	oplogCommit,
	createPromise,
	completePromise,
	getPromise,
	createWebhook,
	configGet,
	traceId,
	spanId,
	traceHeaders,
	getIdempotenceMode,
	setIdempotenceMode,
	revertAgent,
	dbExecute,
	dbQuery,
	rpcBytes,
	rpcScheduleBytes,
	rpcFireAndForgetBytes,
	httpStreamOpen,
	httpStreamReadChunk,
	httpStreamClose,
	dbExecuteParams,
	dbQueryParams,
]

import Types exposing [
	Metadata,
	MethodDeclaration,
	AgentMode,
	ToolDefinition,
	ToolCall,
	ToolResult,
	PersistenceMode,
	SecretHandle,
	SecretHeader,
]
import Effect

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
	invoke : state, Str -> Result({ state : state, stateChanged : Bool, out : Str }, Str),
}

## Specification builder for a strongly-typed agent
Spec(state) : {
	name : Str,
	version : Str,
	description : Str,
	mode : AgentMode,
	init : Str -> Result(state, Str),
	handlers : List(Handler(state)),
	tools : List(ToolDefinition),
	toolHandler : state, ToolCall -> Result({ state : state, stateChanged : Bool, result : ToolResult }, Str),
	serializeState : state -> Str,
	deserializeState : Str -> Result(state, Str),
}

## State-erased agent closure representation
ErasedAgent : {
	metadata : Metadata,
	init : Str -> Result(Str, Str),
	invokeMethod : Str, Str, Str -> Result({ nextStateJson : Str, stateChanged : Bool, out : Str }, Str),
	handleToolCall : Str, ToolCall -> Result({ nextStateJson : Str, stateChanged : Bool, result : ToolResult }, Str),
	handleMessage : Str, Str -> Result({ nextStateJson : Str, stateChanged : Bool, response : Str }, Str),
}

## Component Agent container (holds 1 or N bundled agents)
Agent(state) : {
	agents : List(ErasedAgent),
}

## Define a stateful (Durable) agent
define :
	{ name : Str, version : Str, description : Str },
	(Str -> Result(state, Str)),
	(state -> Str),
	(Str -> Result(state, Str))
	-> Spec(state)
define = |meta, initFn, serializeFn, deserializeFn| {
	name: meta.name,
	version: meta.version,
	description: meta.description,
	mode: Durable,
	init: initFn,
	handlers: [],
	tools: [],
	toolHandler: |st, call|
		Ok({
			state: st,
			stateChanged: Bool.false,
			result: { id: call.id, success: Bool.false, output: "Tool not found: ${call.name}" },
		}),
	serializeState: serializeFn,
	deserializeState: deserializeFn,
}

## Define an ephemeral (stateless) agent
defineEphemeral :
	{ name : Str, version : Str, description : Str }
	-> Spec({})
defineEphemeral = |meta| {
	name: meta.name,
	version: meta.version,
	description: meta.description,
	mode: Ephemeral,
	init: |_| Ok({}),
	handlers: [],
	tools: [],
	toolHandler: |st, call|
		Ok({
			state: st,
			stateChanged: Bool.false,
			result: { id: call.id, success: Bool.false, output: "Tool not found: ${call.name}" },
		}),
	serializeState: |_| "{}",
	deserializeState: |_| Ok({}),
}

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
				Ok({ state: nextSt, stateChanged: Bool.true, result }),
			Err(e) => Err(e),
		},
}

## Attach stateless tool handler
withStatelessToolHandler : Spec(state), (ToolCall -> Result(ToolResult, Str)) -> Spec(state)
withStatelessToolHandler = |spec, handler| {
	..spec,
	toolHandler: |st, call|
		match handler(call) {
			Ok(result) =>
				Ok({ state: st, stateChanged: Bool.false, result }),
			Err(e) => Err(e),
		},
}

## Attach effectful stateful tool handler
withEffectfulToolHandler : Spec(state), (state, ToolCall -> Effect.Effect(Result({ state : state, result : ToolResult }, Str))) -> Spec(state)
withEffectfulToolHandler = |spec, handler| {
	..spec,
	toolHandler: |st, call| {
		eff = handler(st, call)
		match Effect.run(eff) {
			Ok({ state: nextSt, result }) =>
				Ok({ state: nextSt, stateChanged: Bool.true, result }),
			Err(e) => Err(e),
		}
	},
}

## Attach effectful stateless tool handler
withEffectfulStatelessToolHandler : Spec(state), (ToolCall -> Effect.Effect(Result(ToolResult, Str))) -> Spec(state)
withEffectfulStatelessToolHandler = |spec, handler| {
	..spec,
	toolHandler: |st, call| {
		eff = handler(call)
		match Effect.run(eff) {
			Ok(result) =>
				Ok({ state: st, stateChanged: Bool.false, result }),
			Err(e) => Err(e),
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
		readOnly: Bool.false,
	}
	handler : Handler(state)
	handler = {
		declaration: decl,
		invoke: |st, inp|
			match fn(st, inp) {
				Ok({ state: nextSt, out }) =>
					Ok({ state: nextSt, stateChanged: Bool.true, out }),
				Err(e) => Err(e),
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
		readOnly: Bool.true,
	}
	handler : Handler(state)
	handler = {
		declaration: decl,
		invoke: |st, inp|
			match fn(st, inp) {
				Ok(out) =>
					Ok({ state: st, stateChanged: Bool.false, out }),
				Err(e) => Err(e),
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
		readOnly: Bool.false,
	}
	handler : Handler({})
	handler = {
		declaration: decl,
		invoke: |st, inp|
			match fn(inp) {
				Ok(out) =>
					Ok({ state: st, stateChanged: Bool.false, out }),
				Err(e) => Err(e),
			},
	}
	{ ..spec, handlers: List.append(spec.handlers, handler) }
}

## Register a typed mutating method with custom codec
registerTyped :
	Spec(state),
	Method(inp, out),
	(state, inp -> Result({ state : state, out : out }, Str)),
	(Str -> Result(inp, Str)),
	(out -> Str)
	-> Spec(state)
registerTyped = |spec, m, fn, decodeInput, encodeOutput| {
	decl = {
		name: m.name,
		description: m.description,
		promptHint: m.promptHint,
		readOnly: Bool.false,
	}
	handler : Handler(state)
	handler = {
		declaration: decl,
		invoke: |st, rawInp|
			match decodeInput(rawInp) {
				Err(e) => Err("invalid input for ${m.name}: ${e}"),
				Ok(parsedInp) =>
					match fn(st, parsedInp) {
						Ok({ state: nextSt, out }) =>
							Ok({ state: nextSt, stateChanged: Bool.true, out: encodeOutput(out) }),
						Err(e) => Err(e),
					},
			},
	}
	{ ..spec, handlers: List.append(spec.handlers, handler) }
}

## Register a typed read-only method with custom codec
registerReadOnlyTyped :
	Spec(state),
	Method(inp, out),
	(state, inp -> Result(out, Str)),
	(Str -> Result(inp, Str)),
	(out -> Str)
	-> Spec(state)
registerReadOnlyTyped = |spec, m, fn, decodeInput, encodeOutput| {
	decl = {
		name: m.name,
		description: m.description,
		promptHint: m.promptHint,
		readOnly: Bool.true,
	}
	handler : Handler(state)
	handler = {
		declaration: decl,
		invoke: |st, rawInp|
			match decodeInput(rawInp) {
				Err(e) => Err("invalid input for ${m.name}: ${e}"),
				Ok(parsedInp) =>
					match fn(st, parsedInp) {
						Ok(out) =>
							Ok({ state: st, stateChanged: Bool.false, out: encodeOutput(out) }),
						Err(e) => Err(e),
					},
			},
	}
	{ ..spec, handlers: List.append(spec.handlers, handler) }
}

## Register a typed ephemeral method with custom codec
registerEphemeralTyped :
	Spec({}),
	Method(inp, out),
	(inp -> Result(out, Str)),
	(Str -> Result(inp, Str)),
	(out -> Str)
	-> Spec({})
registerEphemeralTyped = |spec, m, fn, decodeInput, encodeOutput| {
	decl = {
		name: m.name,
		description: m.description,
		promptHint: m.promptHint,
		readOnly: Bool.false,
	}
	handler : Handler({})
	handler = {
		declaration: decl,
		invoke: |st, rawInp|
			match decodeInput(rawInp) {
				Err(e) => Err("invalid input for ${m.name}: ${e}"),
				Ok(parsedInp) =>
					match fn(parsedInp) {
						Ok(out) =>
							Ok({ state: st, stateChanged: Bool.false, out: encodeOutput(out) }),
						Err(e) => Err(e),
					},
			},
	}
	{ ..spec, handlers: List.append(spec.handlers, handler) }
}

## Register a mutating method with an effectful handler
registerEffectful :
	Spec(state),
	Method(_in, _out),
	(state, Str -> Effect.Effect(Result({ state : state, out : Str }, Str)))
	-> Spec(state)
registerEffectful = |spec, m, fn| {
	decl = {
		name: m.name,
		description: m.description,
		promptHint: m.promptHint,
		readOnly: Bool.false,
	}
	handler : Handler(state)
	handler = {
		declaration: decl,
		invoke: |st, rawInp| {
			eff = fn(st, rawInp)
			match Effect.run(eff) {
				Ok({ state: nextSt, out }) =>
					Ok({ state: nextSt, stateChanged: Bool.true, out }),
				Err(e) => Err(e),
			}
		},
	}
	{ ..spec, handlers: List.append(spec.handlers, handler) }
}

## Register a typed mutating method with an effectful handler and custom codec
registerEffectfulTyped :
	Spec(state),
	Method(inp, out),
	(state, inp -> Effect.Effect(Result({ state : state, out : out }, Str))),
	(Str -> Result(inp, Str)),
	(out -> Str)
	-> Spec(state)
registerEffectfulTyped = |spec, m, fn, decodeInput, encodeOutput| {
	decl = {
		name: m.name,
		description: m.description,
		promptHint: m.promptHint,
		readOnly: Bool.false,
	}
	handler : Handler(state)
	handler = {
		declaration: decl,
		invoke: |st, rawInp|
			match decodeInput(rawInp) {
				Err(e) => Err("invalid input for ${m.name}: ${e}"),
				Ok(parsedInp) => {
					eff = fn(st, parsedInp)
					match Effect.run(eff) {
						Ok({ state: nextSt, out }) =>
							Ok({ state: nextSt, stateChanged: Bool.true, out: encodeOutput(out) }),
						Err(e) => Err(e),
					}
				},
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
		},
		init: |configStr|
			match spec.init(configStr) {
				Ok(initState) =>
					if spec.mode == Ephemeral {
						Ok("{}")
					} else {
						Ok(spec.serializeState(initState))
					}
				Err(err) => Err(err),
			},
		invokeMethod: |methodName, stateJson, inputJson| {
			match findHandler(spec.handlers, methodName) {
				Err(err) => Err(err),
				Ok(handler) => {
					stateRes =
						if spec.mode == Ephemeral {
							spec.deserializeState("{}")
						} else {
							spec.deserializeState(stateJson)
						}
					match stateRes {
						Err(err) => Err("Failed to deserialize state: ${err}"),
						Ok(currentState) =>
							match handler.invoke(currentState, inputJson) {
								Err(err) => Err(err),
								Ok(res) => {
									nextStateJson =
										if res.stateChanged && spec.mode == Durable {
											spec.serializeState(res.state)
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
					spec.deserializeState("{}")
				} else {
					spec.deserializeState(stateJson)
				}
			match stateRes {
				Err(err) => Err("Failed to deserialize state: ${err}"),
				Ok(currentState) =>
					match (spec.toolHandler)(currentState, call) {
						Err(err) => Err(err),
						Ok(res) => {
							nextStateJson =
								if res.stateChanged && spec.mode == Durable {
									spec.serializeState(res.state)
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
							spec.deserializeState("{}")
						} else {
							spec.deserializeState(stateJson)
						}
					match stateRes {
						Err(err) => Err("Failed to deserialize state: ${err}"),
						Ok(currentState) =>
							match handler.invoke(currentState, msgStr) {
								Err(err) => Err(err),
								Ok(res) => {
									nextStateJson =
										if res.stateChanged && spec.mode == Durable {
											spec.serializeState(res.state)
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
					Ok({
						nextStateJson: "",
						stateChanged: Bool.false,
						response: "Agent received: ${msgStr}",
					}),
			}
		},
	}
	{ agents: [erased] }
}

## Bundle multiple agents into a single Wasm component
bundle : List(Agent(_)) -> Agent({})
bundle = |agentList| {
	flattened = List.join_map(agentList, |a| a.agents)
	{ agents: flattened }
}

## Backwards-compatible legacy single-agent definition
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
		},
		init: |configStr|
			match config.init(configStr) {
				Ok(st) => Ok(config.serializeState(st)),
				Err(e) => Err(e),
			},
		invokeMethod: |_methodName, stateJson, inputJson|
			match config.deserializeState(stateJson) {
				Err(e) => Err("Failed to deserialize state: ${e}"),
				Ok(st) =>
					match config.handleMessage(st, inputJson) {
						Ok({ state: nextSt, response }) =>
							Ok({
								nextStateJson: config.serializeState(nextSt),
								stateChanged: Bool.true,
								out: response,
							}),
						Err(e) => Err(e),
					},
			},
		handleToolCall: |stateJson, call|
			match config.deserializeState(stateJson) {
				Err(e) => Err("Failed to deserialize state: ${e}"),
				Ok(st) =>
					match config.handleToolCall(st, call) {
						Ok({ state: nextSt, result }) =>
							Ok({
								nextStateJson: config.serializeState(nextSt),
								stateChanged: Bool.true,
								result,
							}),
						Err(e) => Err(e),
					},
			},
		handleMessage: |stateJson, msgStr|
			match config.deserializeState(stateJson) {
				Err(e) => Err("Failed to deserialize state: ${e}"),
				Ok(st) =>
					match config.handleMessage(st, msgStr) {
						Ok({ state: nextSt, response }) =>
							Ok({
								nextStateJson: config.serializeState(nextSt),
								stateChanged: Bool.true,
								response,
							}),
						Err(e) => Err(e),
					},
			},
	}
	{ agents: [erased] }
}

findHandler : List(Handler(state)), Str -> Result(Handler(state), Str)
findHandler = |handlers, targetName| {
	match handlers {
		[] => Err("Method '${targetName}' not found"),
		[first, .. as rest] =>
			if first.declaration.name == targetName {
				Ok(first)
			} else {
				findHandler(rest, targetName)
			},
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

## Typed Remote Procedure Call
rpc : Str, Method(_in, _out), Str -> Effect.Effect(Result(Str, Str))
rpc = |targetWorker, m, payload|
	Effect.rpcInvoke(targetWorker, m.name, payload)

rpcSchedule : Str, Method(_in, _out), Str, U64 -> Effect.Effect(Result(Str, Str))
rpcSchedule = |targetWorker, m, payload, delayMillis|
	Effect.rpcSchedule(targetWorker, m.name, payload, delayMillis)

rpcFireAndForget : Str, Method(_in, _out), Str -> Effect.Effect(Result(Str, Str))
rpcFireAndForget = |targetWorker, m, payload|
	Effect.rpcFireAndForget(targetWorker, m.name, payload)

## Binary Remote Procedure Call (Direct bytes over WasmRpc)
rpcBytes : Str, Method(_in, _out), List(U8) -> Effect.Effect(Result(List(U8), Str))
rpcBytes = |targetWorker, m, payload|
	Effect.rpcInvokeBytes(targetWorker, m.name, payload)

rpcScheduleBytes : Str, Method(_in, _out), List(U8), U64 -> Effect.Effect(Result(List(U8), Str))
rpcScheduleBytes = |targetWorker, m, payload, delayMillis|
	Effect.rpcScheduleBytes(targetWorker, m.name, payload, delayMillis)

rpcFireAndForgetBytes : Str, Method(_in, _out), List(U8) -> Effect.Effect(Result({}, Str))
rpcFireAndForgetBytes = |targetWorker, m, payload|
	Effect.rpcFireAndForgetBytes(targetWorker, m.name, payload)

## Typed Remote Procedure Call with input and output codecs
rpcTyped :
	Str,
	Method(inp, out),
	inp,
	(inp -> Str),
	(Str -> Result(out, Str))
	-> Effect.Effect(Result(out, Str))
rpcTyped = |targetWorker, m, inputVal, encodeInput, decodeOutput| {
	payload = encodeInput(inputVal)
	Effect.map(
		Effect.rpcInvoke(targetWorker, m.name, payload),
		|res|
			match res {
				Ok(reply) => decodeOutput(reply),
				Err(err) => Err(err),
			},
	)
}

## Scheduled Typed RPC with input codec
rpcScheduleTyped :
	Str,
	Method(inp, _out),
	inp,
	(inp -> Str),
	U64
	-> Effect.Effect(Result(Str, Str))
rpcScheduleTyped = |targetWorker, m, inputVal, encodeInput, delayMillis| {
	payload = encodeInput(inputVal)
	Effect.rpcSchedule(targetWorker, m.name, payload, delayMillis)
}

## Fire-and-forget Typed RPC with input codec
rpcFireAndForgetTyped :
	Str,
	Method(inp, _out),
	inp,
	(inp -> Str)
	-> Effect.Effect(Result(Str, Str))
rpcFireAndForgetTyped = |targetWorker, m, inputVal, encodeInput| {
	payload = encodeInput(inputVal)
	Effect.rpcFireAndForget(targetWorker, m.name, payload)
}

## Set Golem Persistence Mode
setPersistence : PersistenceMode -> Effect.Effect({})
setPersistence = |mode|
	Effect.setPersistenceMode(mode)

## Perform outgoing HTTP request
http : Str -> Effect.Effect(Result(Str, Str))
http = |reqJson|
	Effect.httpRequest(reqJson)

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

## Acquire an opaque secret handle from the host environment without leaking plaintext into guest memory
getSecretHandle : Str -> Effect.Effect(Result(SecretHandle, Str))
getSecretHandle = |name|
	Effect.getSecretHandle(name)

## Explicitly reveal secret plaintext into guest memory
revealSecret : SecretHandle -> Effect.Effect(Result(Str, Str))
revealSecret = |handle|
	Effect.revealSecret(handle)

## Generate a unique idempotency key
generateIdempotencyKey : Effect.Effect(Result(Str, Str))
generateIdempotencyKey =
	Effect.generateIdempotencyKey

## Get the current oplog index
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

## Revert agent execution state back to specified oplog index
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

