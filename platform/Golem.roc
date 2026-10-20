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
	newQuotaToken!,
	quotaReserve!,
	quotaCommit!,
	quotaSplit!,
	quotaMerge!,
	getRetryPolicies!,
	getRetryPolicy!,
	resolveRetryPolicy!,
	nextRetryDelay!,
	setRetryPolicy!,
	removeRetryPolicy!,
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

## Asks the host for a quota token on the resource `resourceName`, which the
## manifest declares under `resourceDefaults.<environment>.<resourceName>`.
## `expectedUse` is the amount this token's reservations are sized for: it tells
## the host how much credit to schedule, and consumes nothing by itself. The
## token lives host-side; the number returned is the handle the other quota
## effects work with.
newQuotaToken! : Str, U64 => U32
newQuotaToken! = |resourceName, expectedUse|
	Effect.newQuotaToken!(resourceName, expectedUse)

## Reserves `amount` from the token's allocation. `Ok` is the handle of a
## reservation for `quotaCommit!`; `Err` is the rejection a `reject`ing resource
## answered with, including its estimate of the wait for capacity when it has
## one. A `throttle`ing resource waits here instead, and a `terminate`ing one
## fails the invocation.
quotaReserve! : U32, U64 => Try(U32, Str)
quotaReserve! = |token, amount|
	Effect.quotaReserve!(token, amount)

## Commits what the reserved work actually used: less than was reserved returns
## the unused capacity, more charges the excess to the token as debt, and a
## reservation that is never committed counts as zero. A handle is spent once.
quotaCommit! : U32, U64 => Try({}, Str)
quotaCommit! = |reservation, used|
	Effect.quotaCommit!(reservation, used)

## Splits `childExpectedUse` off the token into a second one. Both stay in this
## agent instance — quota tokens cannot travel between agents in this platform.
quotaSplit! : U32, U64 => Try(U32, Str)
quotaSplit! = |token, childExpectedUse|
	Effect.quotaSplit!(token, childExpectedUse)

## Merges `other` back into the token, consuming it. The two tokens have to name
## the same resource.
quotaMerge! : U32, U32 => Try({}, Str)
quotaMerge! = |token, other|
	Effect.quotaMerge!(token, other)

## Every retry policy active for this agent, as a JSON array of named policies
## (`[{"name": …, "priority": …, "predicate": {"nodes": […]},
## "policy": {"nodes": […]}}, …]`). The trees are the host's flattened form —
## the root is `nodes[0]` and children are indices into the same list — with
## durations as nanosecond counts; the README's "Quotas and retry" section spells
## the shape out.
getRetryPolicies! : {} => Str
getRetryPolicies! = |_|
	Effect.getRetryPolicies!({})

## One policy by name, the same JSON as above — or `""` when no policy has that
## name.
getRetryPolicy! : Str => Try(Str, Str)
getRetryPolicy! = |name|
	Effect.getRetryPolicy!(name)

## The first policy whose predicate matches `verb` / `nounUri` with the given
## properties (a JSON object of strings, integers and booleans, `{}` for none),
## as policy JSON — or `""` when none matches. A predicate the host cannot
## evaluate comes back as `Err`, without failing the invocation.
resolveRetryPolicy! : Str, Str, Str => Try(Str, Str)
resolveRetryPolicy! = |verb, nounUri, propertiesJson|
	Effect.resolveRetryPolicy!(verb, nounUri, propertiesJson)

## Advances the host's retry evaluator once, after a failed attempt: `policyJson`
## is a policy from above (`""` takes the agent's configured default),
## `stateJson` the `state` the previous decision returned (`[]` starts a
## sequence), `elapsedNanos` the time since the first attempt, and `contextJson`
## `{"verb": …, "nounUri": …, "properties": {…}}`. The answer is
## `{"state": […], "delayNanos": …}`, with `null` for a delay when the policy is
## exhausted (a zero delay still allows another attempt).
nextRetryDelay! : Str, Str, U64, Str => Try(Str, Str)
nextRetryDelay! = |policyJson, stateJson, elapsedNanos, contextJson|
	Effect.nextRetryDelay!(policyJson, stateJson, elapsedNanos, contextJson)

## Adds or overwrites a named policy, taking the JSON `getRetryPolicies!`
## prints. The host records it in the agent's oplog, so it outlives a restart.
setRetryPolicy! : Str => Try({}, Str)
setRetryPolicy! = |policyJson|
	Effect.setRetryPolicy!(policyJson)

## Removes a named policy (also recorded in the oplog). A policy the manifest
## declared comes back on the next deployment; this removes the agent's own copy.
removeRetryPolicy! : Str => {}
removeRetryPolicy! = |name|
	Effect.removeRetryPolicy!(name)
