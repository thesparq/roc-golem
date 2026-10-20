import Types exposing [LogLevel, PersistenceMode]
import Host

## Low-level effect implementations that call the Rust host functions.
Effect := {}.{
	log! : LogLevel, Str => {}
	log! = |level, msg| {
		levelU8 : U8
		levelU8 =
			match level {
				Trace => 0
				Debug => 1
				Info => 2
				Warn => 3
				Error => 4
			}
		Host.rocFxLog!(levelU8, msg)
	}

	getWorkerId! : {} => Str
	getWorkerId! = |_| Host.rocFxGetWorkerId!({})

	createWebhook! : {} => Try(Str, Str)
	createWebhook! = |_| Host.rocFxCreateWebhook!({})

	awaitWebhook! : Str => Try(Str, Str)
	awaitWebhook! = |url| Host.rocFxAwaitWebhook!(url)

	rpcInvoke! : Str, Str, Str => Try(Str, Str)
	rpcInvoke! = |targetWorker, fnName, payload| {
		Host.rocFxRpcInvoke!(targetWorker, fnName, payload)
	}

	rpcFireAndForget! : Str, Str, Str => Try(Str, Str)
	rpcFireAndForget! = |targetWorker, fnName, payload| {
		Host.rocFxRpcFireAndForget!(targetWorker, fnName, payload)
	}

	rpcSchedule! : Str, Str, Str, U64 => Try(Str, Str)
	rpcSchedule! = |targetWorker, fnName, payload, delayMillis| {
		Host.rocFxRpcSchedule!(targetWorker, fnName, payload, delayMillis)
	}

	rpcScheduleCancelable! : Str, Str, Str, U64 => Try(U32, Str)
	rpcScheduleCancelable! = |targetWorker, fnName, payload, delayMillis| {
		Host.rocFxRpcScheduleCancelable!(targetWorker, fnName, payload, delayMillis)
	}

	rpcCancel! : U32 => Try({}, Str)
	rpcCancel! = |handle| {
		Host.rocFxRpcCancel!(handle)
	}

	parseAgentId! : Str => Try(Str, Str)
	parseAgentId! = |agentId| {
		Host.rocFxParseAgentId!(agentId)
	}

	makeAgentId! : Str, Str => Try(Str, Str)
	makeAgentId! = |agentTypeName, constructorParam| {
		Host.rocFxMakeAgentId!(agentTypeName, constructorParam)
	}

	setPersistenceMode! : PersistenceMode => {}
	setPersistenceMode! = |mode| {
		modeU8 : U8
		modeU8 =
			match mode {
				PersistNothing => 0
				PersistStateOnly => 1
				PersistEverything => 2
			}
		Host.rocFxSetPersistence!(modeU8)
	}

	httpRequest! : Str => Try(Str, Str)
	httpRequest! = |reqJson| {
		Host.rocFxHttpRequest!(reqJson)
	}

	wsConnect! : Str => Try(U32, Str)
	wsConnect! = |url| {
		Host.rocFxWsConnect!(url)
	}

	wsSend! : U32, Str => Try({}, Str)
	wsSend! = |handle, msg| {
		Host.rocFxWsSend!(handle, msg)
	}

	wsReceive! : U32 => Try(Str, Str)
	wsReceive! = |handle| {
		Host.rocFxWsReceive!(handle)
	}

	wsClose! : U32 => Try({}, Str)
	wsClose! = |handle| {
		Host.rocFxWsClose!(handle)
	}

	sleepMillis! : U64 => {}
	sleepMillis! = |millis| {
		Host.rocFxSleepMillis!(millis)
	}

	nowMillis! : {} => U64
	nowMillis! = |_| {
		Host.rocFxNowMillis!({})
	}

	getConfig! : Str => Try(Str, Str)
	getConfig! = |key| {
		Host.rocFxGetConfig!(key)
	}

	## Quotas: a token, a reservation and the commit are three host calls, so
	## the app carries the two handles between them.
	newQuotaToken! : Str, U64 => U32
	newQuotaToken! = |resourceName, expectedUse| {
		Host.rocFxNewQuotaToken!(resourceName, expectedUse)
	}

	quotaReserve! : U32, U64 => Try(U32, Str)
	quotaReserve! = |token, amount| {
		Host.rocFxQuotaReserve!(token, amount)
	}

	quotaCommit! : U32, U64 => Try({}, Str)
	quotaCommit! = |reservation, used| {
		Host.rocFxQuotaCommit!(reservation, used)
	}

	quotaSplit! : U32, U64 => Try(U32, Str)
	quotaSplit! = |token, childExpectedUse| {
		Host.rocFxQuotaSplit!(token, childExpectedUse)
	}

	quotaMerge! : U32, U32 => Try({}, Str)
	quotaMerge! = |token, other| {
		Host.rocFxQuotaMerge!(token, other)
	}

	## Retry policies: the trees travel as the JSON `getRetryPolicies!`
	## prints, so nothing is parsed on the Roc side.
	getRetryPolicies! : {} => Str
	getRetryPolicies! = |_| {
		Host.rocFxGetRetryPolicies!({})
	}

	getRetryPolicy! : Str => Try(Str, Str)
	getRetryPolicy! = |name| {
		Host.rocFxGetRetryPolicy!(name)
	}

	resolveRetryPolicy! : Str, Str, Str => Try(Str, Str)
	resolveRetryPolicy! = |verb, nounUri, propertiesJson| {
		Host.rocFxResolveRetryPolicy!(verb, nounUri, propertiesJson)
	}

	nextRetryDelay! : Str, Str, U64, Str => Try(Str, Str)
	nextRetryDelay! = |policyJson, stateJson, elapsedNanos, contextJson| {
		Host.rocFxNextRetryDelay!(policyJson, stateJson, elapsedNanos, contextJson)
	}

	setRetryPolicy! : Str => Try({}, Str)
	setRetryPolicy! = |policyJson| {
		Host.rocFxSetRetryPolicy!(policyJson)
	}

	removeRetryPolicy! : Str => {}
	removeRetryPolicy! = |name| {
		Host.rocFxRemoveRetryPolicy!(name)
	}

	## Oplog and execution control (`golem:api/host`), the part of it an app can
	## use: no oplog cursor and no idempotence switch survive in Golem 1.6, so
	## what is left is failing an invocation, naming an external write, forking
	## the agent, committing the oplog, and resolving component and agent ids.
	trap! : Str => {}
	trap! = |reason| {
		Host.rocFxTrap!(reason)
	}

	idempotencyKey! : {} => Str
	idempotencyKey! = |_| {
		Host.rocFxIdempotencyKey!({})
	}

	fork! : {} => Try(Str, Str)
	fork! = |_| {
		Host.rocFxFork!({})
	}

	oplogCommit! : U8 => {}
	oplogCommit! = |replicas| {
		Host.rocFxOplogCommit!(replicas)
	}

	resolveAgentId! : Str, Str => Try(Str, Str)
	resolveAgentId! = |componentReference, agentName| {
		Host.rocFxResolveAgentId!(componentReference, agentName)
	}

	resolveComponentId! : Str => Try(Str, Str)
	resolveComponentId! = |componentReference| {
		Host.rocFxResolveComponentId!(componentReference)
	}

	## The two shared stores (`wasi:keyvalue`, `wasi:blobstore`): one effect per
	## operation. Values are bytes and travel as lossy-UTF-8 text, the convention
	## `http!` uses for bodies; buckets, keys, containers and objects are named by
	## strings, so no handle has to be carried between calls. A missing key is
	## `"null"` (`kvExists!` is the direct check).
	kvGet! : Str, Str => Try(Str, Str)
	kvGet! = |bucket, key| {
		Host.rocFxKvGet!(bucket, key)
	}

	kvSet! : Str, Str, Str => Try({}, Str)
	kvSet! = |bucket, key, value| {
		Host.rocFxKvSet!(bucket, key, value)
	}

	kvDelete! : Str, Str => Try({}, Str)
	kvDelete! = |bucket, key| {
		Host.rocFxKvDelete!(bucket, key)
	}

	kvExists! : Str, Str => Try(Bool, Str)
	kvExists! = |bucket, key| {
		Host.rocFxKvExists!(bucket, key)
	}

	blobCreateContainer! : Str => Try({}, Str)
	blobCreateContainer! = |name| {
		Host.rocFxBlobCreateContainer!(name)
	}

	blobPut! : Str, Str, Str => Try({}, Str)
	blobPut! = |container, object, value| {
		Host.rocFxBlobPut!(container, object, value)
	}

	blobGet! : Str, Str => Try(Str, Str)
	blobGet! = |container, object| {
		Host.rocFxBlobGet!(container, object)
	}

	blobHas! : Str, Str => Try(Bool, Str)
	blobHas! = |container, object| {
		Host.rocFxBlobHas!(container, object)
	}

	blobList! : Str => Try(Str, Str)
	blobList! = |container| {
		Host.rocFxBlobList!(container)
	}

	blobDelete! : Str, Str => Try({}, Str)
	blobDelete! = |container, object| {
		Host.rocFxBlobDelete!(container, object)
	}
}
