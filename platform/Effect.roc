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

	## The agent's own filesystem (`wasi:filesystem@0.2.6`): one effect per
	## operation, files named by path and bytes carried as lossy-UTF-8 text. The
	## host resolves the path against the agent's root and opens the file itself,
	## so no descriptor reaches the app and nothing has to be closed.
	readFile! : Str => Try(Str, Str)
	readFile! = |path| {
		Host.rocFxReadFile!(path)
	}

	writeFile! : Str, Str => Try({}, Str)
	writeFile! = |path, value| {
		Host.rocFxWriteFile!(path, value)
	}

	appendFile! : Str, Str => Try({}, Str)
	appendFile! = |path, value| {
		Host.rocFxAppendFile!(path, value)
	}

	fileExists! : Str => Try(Bool, Str)
	fileExists! = |path| {
		Host.rocFxFileExists!(path)
	}

	fileSize! : Str => Try(U64, Str)
	fileSize! = |path| {
		Host.rocFxFileSize!(path)
	}

	deleteFile! : Str => Try({}, Str)
	deleteFile! = |path| {
		Host.rocFxDeleteFile!(path)
	}

	makeDir! : Str => Try({}, Str)
	makeDir! = |path| {
		Host.rocFxMakeDir!(path)
	}

	listDir! : Str => Try(Str, Str)
	listDir! = |path| {
		Host.rocFxListDir!(path)
	}

	## External durable streams (`golem:agent/durable-streams`): a reader and a
	## writer for a service that speaks the Durable Streams protocol. Both are
	## host-side resources the app carries as `U32` handles, and every structured
	## value travels as JSON in the WIT shape (see `Golem.roc` for the shapes and
	## the README for what the app owns).
	durableStreamReader! : Str => Try(U32, Str)
	durableStreamReader! = |optionsJson| {
		Host.rocFxDurableStreamReader!(optionsJson)
	}

	durableStreamRead! : U32, Str => Try(Str, Str)
	durableStreamRead! = |handle, requestJson| {
		Host.rocFxDurableStreamRead!(handle, requestJson)
	}

	durableStreamWriter! : Str => Try(U32, Str)
	durableStreamWriter! = |optionsJson| {
		Host.rocFxDurableStreamWriter!(optionsJson)
	}

	durableStreamAppend! : U32, Str => Try(Str, Str)
	durableStreamAppend! = |handle, requestJson| {
		Host.rocFxDurableStreamAppend!(handle, requestJson)
	}

	durableStreamRelease! : U32 => Try({}, Str)
	durableStreamRelease! = |handle| {
		Host.rocFxDurableStreamRelease!(handle)
	}

	## Databases (`golem:rdbms`): three engines, one `open` per interface, and a
	## shared query/execute/transaction surface. Connections and transactions are
	## `U32` handles in the host's handle table; a statement's params are a JSON
	## array of `db-value` encodings and a result is `{"columns": …, "rows": …}`
	## (the encoding is spelled out in the README's database section).
	dbOpen! : Str => Try(U32, Str)
	dbOpen! = |address| {
		Host.rocFxDbOpen!(address)
	}

	dbOpenMysql! : Str => Try(U32, Str)
	dbOpenMysql! = |address| {
		Host.rocFxDbOpenMysql!(address)
	}

	dbOpenIgnite! : Str => Try(U32, Str)
	dbOpenIgnite! = |address| {
		Host.rocFxDbOpenIgnite!(address)
	}

	dbQuery! : U32, Str, Str => Try(Str, Str)
	dbQuery! = |handle, statement, params|
		Host.rocFxDbQuery!(handle, statement, params)

	dbExecute! : U32, Str, Str => Try(U64, Str)
	dbExecute! = |handle, statement, params|
		Host.rocFxDbExecute!(handle, statement, params)

	dbBegin! : U32 => Try(U32, Str)
	dbBegin! = |handle|
		Host.rocFxDbBegin!(handle)

	dbCommit! : U32 => Try({}, Str)
	dbCommit! = |handle|
		Host.rocFxDbCommit!(handle)

	dbRollback! : U32 => Try({}, Str)
	dbRollback! = |handle|
		Host.rocFxDbRollback!(handle)

	## Permissions (`golem:permissions`) and the principal: every one of these is a
	## synchronous host call, so nothing is awaited here. Cards are host-side
	## capabilities the app carries as `U32` handles.
	getPrincipal! : {} => Str
	getPrincipal! = |_|
		Host.rocFxGetPrincipal!({})

	permissionsWallet! : {} => Try(Str, Str)
	permissionsWallet! = |_|
		Host.rocFxPermissionsWallet!({})

	permissionsWalletVersion! : {} => Try(Str, Str)
	permissionsWalletVersion! = |_|
		Host.rocFxPermissionsWalletVersion!({})

	permissionId! : U32 => Try(Str, Str)
	permissionId! = |handle|
		Host.rocFxPermissionId!(handle)

	permissionParents! : U32 => Try(Str, Str)
	permissionParents! = |handle|
		Host.rocFxPermissionParents!(handle)

	permissionExpiresAt! : U32 => Try(Str, Str)
	permissionExpiresAt! = |handle|
		Host.rocFxPermissionExpiresAt!(handle)

	permissionInspect! : U32 => Try(Str, Str)
	permissionInspect! = |handle|
		Host.rocFxPermissionInspect!(handle)

	permissionDerive! : U32, Str => Try(U32, Str)
	permissionDerive! = |handle, boundsJson|
		Host.rocFxPermissionDerive!(handle, boundsJson)

	permissionDeriveFromWallet! : Str => Try(U32, Str)
	permissionDeriveFromWallet! = |boundsJson|
		Host.rocFxPermissionDeriveFromWallet!(boundsJson)

	permissionDeriveScope! : Str => Try(U32, Str)
	permissionDeriveScope! = |requestJson|
		Host.rocFxPermissionDeriveScope!(requestJson)

	permissionInstall! : U32, Str => Try({}, Str)
	permissionInstall! = |handle, holderJson|
		Host.rocFxPermissionInstall!(handle, holderJson)

	permissionRevoke! : U32 => Try(U32, Str)
	permissionRevoke! = |handle|
		Host.rocFxPermissionRevoke!(handle)

	permissionRelease! : U32 => Try({}, Str)
	permissionRelease! = |handle|
		Host.rocFxPermissionRelease!(handle)

	permissionListModules! : {} => Try(Str, Str)
	permissionListModules! = |_|
		Host.rocFxPermissionListModules!({})

	permissionValidateGrant! : Str => Try({}, Str)
	permissionValidateGrant! = |grantJson|
		Host.rocFxPermissionValidateGrant!(grantJson)
}
