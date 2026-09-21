module [
	Agent,
	defineAgent,
	logTrace!,
	logDebug!,
	logInfo!,
	logWarn!,
	logError!,
	workerId!,
	rpc!,
	setPersistence!,
	http!,
	websocketConnect!,
	websocketSend!,
	websocketReceive!,
	websocketClose!,
	sleep!,
	now!,
]

import Effect
import Types exposing [LogLevel, PersistenceMode, ToolDefinition, ToolCall, ToolResult, Metadata]

Agent(state) : {
	init! : Str => state,
	handleMessage! : state, Str => { state : state, replies : List(Str) },
	handleToolCall! : state, ToolCall => { state : state, result : ToolResult },
	metadata : Metadata,
	serializeState : state -> Str,
	deserializeState : Str -> Try(state, Str),
}

defineAgent : Agent(state) -> Agent(state)
defineAgent = |agent|
	agent

logTrace! : Str => {}
logTrace! = |msg|
	Effect.log!(Trace, msg)

logDebug! : Str => {}
logDebug! = |msg|
	Effect.log!(Debug, msg)

logInfo! : Str => {}
logInfo! = |msg|
	Effect.log!(Info, msg)

logWarn! : Str => {}
logWarn! = |msg|
	Effect.log!(Warn, msg)

logError! : Str => {}
logError! = |msg|
	Effect.log!(Error, msg)

workerId! : {} => Str
workerId! = |_|
	Effect.getWorkerId!({})

rpc! : Str, Str, Str => Try(Str, Str)
rpc! = |targetWorker, fnName, payload|
	Effect.rpcInvoke!(targetWorker, fnName, payload)

setPersistence! : PersistenceMode => {}
setPersistence! = |mode|
	Effect.setPersistenceMode!(mode)

http! : Str => Try(Str, Str)
http! = |reqJson|
	Effect.httpRequest!(reqJson)

websocketConnect! : Str => Try(U32, Str)
websocketConnect! = |url|
	Effect.wsConnect!(url)

websocketSend! : U32, Str => Try({}, Str)
websocketSend! = |handle, msg|
	Effect.wsSend!(handle, msg)

websocketReceive! : U32 => Try(Str, Str)
websocketReceive! = |handle|
	Effect.wsReceive!(handle)

websocketClose! : U32 => Try({}, Str)
websocketClose! = |handle|
	Effect.wsClose!(handle)

sleep! : U64 => {}
sleep! = |millis|
	Effect.sleepMillis!(millis)

now! : {} => U64
now! = |_|
	Effect.nowMillis!({})
