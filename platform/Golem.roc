# This module is imported by applications (`import pf.Golem`), so it still uses
# the legacy module-header syntax: in the pinned nightly (2026-10-19-601b54c)
# the new type-module format crashes the compiler when an application imports
# the module's type through the platform (`exposes`). The internal modules
# (Types, Effect) have been migrated; revisit Golem once the upstream compiler
# bug is fixed. The matching CI tolerance for the deprecation warning lives in
# .github/workflows/ci.yaml.
module [
	Agent,
	defineAgent,
	logTrace!,
	logDebug!,
	logInfo!,
	logWarn!,
	logError!,
	workerId!,
	createWebhook!,
	awaitWebhook!,
	rpc!,
	rpcFireAndForget!,
	rpcSchedule!,
	rpcScheduleCancelable!,
	rpcCancel!,
	parseAgentId!,
	makeAgentId!,
	setPersistence!,
	http!,
	websocketConnect!,
	websocketSend!,
	websocketReceive!,
	websocketClose!,
	sleep!,
	now!,
	getConfig!,
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

## Creates a webhook for this agent and returns its callback URL: a POST to the
## URL completes the promise this effect created. The URL is the handle the app
## stores in its own state; `awaitWebhook!` resolves it back to the promise.
createWebhook! : {} => Try(Str, Str)
createWebhook! = |_|
	Effect.createWebhook!({})

## Waits for the POST to `url` (from `createWebhook!`) and returns its body as
## text.
awaitWebhook! : Str => Try(Str, Str)
awaitWebhook! = |url|
	Effect.awaitWebhook!(url)

rpc! : Str, Str, Str => Try(Str, Str)
rpc! = |targetWorker, fnName, payload|
	Effect.rpcInvoke!(targetWorker, fnName, payload)

## Triggers a remote method and returns its invocation identity right away
## (`{"agentId": …, "idempotencyKey": …}`) instead of waiting for the reply.
rpcFireAndForget! : Str, Str, Str => Try(Str, Str)
rpcFireAndForget! = |targetWorker, fnName, payload|
	Effect.rpcFireAndForget!(targetWorker, fnName, payload)

## Schedules a remote method for `delayMillis` from now and returns the
## scheduled invocation's identity, in the same shape as `rpcFireAndForget!`.
rpcSchedule! : Str, Str, Str, U64 => Try(Str, Str)
rpcSchedule! = |targetWorker, fnName, payload, delayMillis|
	Effect.rpcSchedule!(targetWorker, fnName, payload, delayMillis)

## The same as `rpcSchedule!`, but returns a handle for `rpcCancel!`, which
## cancels the invocation as long as it has not run yet.
rpcScheduleCancelable! : Str, Str, Str, U64 => Try(U32, Str)
rpcScheduleCancelable! = |targetWorker, fnName, payload, delayMillis|
	Effect.rpcScheduleCancelable!(targetWorker, fnName, payload, delayMillis)

## Cancels a scheduled invocation that has not run yet. The handle comes from
## `rpcScheduleCancelable!` and can be used once.
rpcCancel! : U32 => Try({}, Str)
rpcCancel! = |handle|
	Effect.rpcCancel!(handle)

## Splits an agent id into `{"agentType": …, "constructor": …,
## "phantomId": …}` — the same resolution `rpc!` does on its target.
parseAgentId! : Str => Try(Str, Str)
parseAgentId! = |agentId|
	Effect.parseAgentId!(agentId)

## Builds an agent id from an agent type name and its constructor parameter,
## the inverse of `parseAgentId!`.
makeAgentId! : Str, Str => Try(Str, Str)
makeAgentId! = |agentTypeName, constructorParam|
	Effect.makeAgentId!(agentTypeName, constructorParam)

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

getConfig! : Str => Try(Str, Str)
getConfig! = |key|
	Effect.getConfig!(key)
