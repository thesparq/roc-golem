import Types exposing [LogLevel, PersistenceMode, Result]

Effect(a) := [Effect({} => a)].{
	rocFxLog! : U8, Str => {}
	rocFxGetWorkerId! : {} => Str
	rocFxRpcInvoke! : Str, Str, Str, Str => Result(Str, Str)
	rocFxRpcSchedule! : Str, Str, Str, Str, U64 => Result(Str, Str)
	rocFxRpcFireAndForget! : Str, Str, Str, Str => Result(Str, Str)
	rocFxSetPersistence! : U8 => {}
	rocFxHttpRequest! : Str => Result(Str, Str)
	rocFxWsConnect! : Str => Result(U32, Str)
	rocFxWsSend! : U32, Str => Result({}, Str)
	rocFxWsReceive! : U32 => Result(Str, Str)
	rocFxWsClose! : U32 => Result({}, Str)
	rocFxSleepMillis! : U64 => {}
	rocFxNowMillis! : {} => U64
	rocFxReadFileBytes! : Str => Result(List(U8), Str)
	rocFxWriteFileBytes! : Str, List(U8) => Result({}, Str)
	rocFxAppendFileBytes! : Str, List(U8) => Result({}, Str)
	rocFxReadFile! : Str => Result(Str, Str)
	rocFxWriteFile! : Str, Str => Result({}, Str)
	rocFxKvGetBytes! : Str, Str => Result(List(U8), Str)
	rocFxKvSetBytes! : Str, Str, List(U8) => Result({}, Str)
	rocFxKvGet! : Str, Str => Result(Str, Str)
	rocFxKvSet! : Str, Str, Str => Result({}, Str)
	rocFxBlobGetBytes! : Str, Str => Result(List(U8), Str)
	rocFxBlobPutBytes! : Str, Str, List(U8) => Result({}, Str)
	rocFxBlobGet! : Str, Str => Result(Str, Str)
	rocFxBlobPut! : Str, Str, Str => Result({}, Str)
	rocFxHttpRequestBytes! : Str, List(U8) => Result(List(U8), Str)
	rocFxGetSecretHandle! : Str => Result(U32, Str)
	rocFxRevealSecret! : U32 => Result(Str, Str)
	rocFxGenerateIdempotencyKey! : {} => Result(Str, Str)
	rocFxGetOplogIndex! : {} => U64
	rocFxOplogCommit! : U8 => {}
	rocFxCreatePromise! : {} => Result(U64, Str)
	rocFxCompletePromise! : U64, List(U8) => Result(Bool, Str)
	rocFxGetPromise! : U64 => Result(List(U8), Str)
	rocFxCreateWebhook! : U64 => Result(Str, Str)
	rocFxConfigGet! : Str => Result(Str, Str)
	rocFxTraceId! : {} => Str
	rocFxSpanId! : {} => Str
	rocFxTraceHeaders! : {} => Result(Str, Str)
	rocFxGetIdempotenceMode! : {} => Bool
	rocFxSetIdempotenceMode! : Bool => {}
	rocFxRevertAgent! : U64 => Result({}, Str)
	rocFxDbExecute! : Str, Str => Result(U64, Str)
	rocFxDbExecuteParams! : Str, Str, Str => Result(U64, Str)
	rocFxDbQuery! : Str, Str => Result(Str, Str)
	rocFxDbQueryParams! : Str, Str, Str => Result(Str, Str)
	rocFxRpcInvokeBytes! : Str, Str, Str, List(U8) => Result(List(U8), Str)
	rocFxRpcScheduleBytes! : Str, Str, Str, List(U8), U64 => Result(List(U8), Str)
	rocFxRpcFireAndForgetBytes! : Str, Str, Str, List(U8) => Result({}, Str)
	rocFxHttpStreamOpen! : Str, List(U8) => Result(U32, Str)
	rocFxHttpStreamRead! : U32, U64 => Result(List(U8), Str)
	rocFxHttpStreamClose! : U32 => Result({}, Str)

	run! : Effect(a) => a
	run! = |Effect(thunk)| thunk({})

	map : Effect(a), (a -> b) -> Effect(b)
	map = |Effect(thunk), fn|
		Effect(|_| fn(thunk({})))

	log : LogLevel, Str -> Effect({})
	log = |level, msg|
		Effect(
			|_| {
				levelU8 : U8
				levelU8 =
					match level {
						Trace => 0
						Debug => 1
						Info => 2
						Warn => 3
						Error => 4
					}
				Effect.rocFxLog!(levelU8, msg)
			},
		)

	getWorkerId : Effect(Str)
	getWorkerId =
		Effect(|_| Effect.rocFxGetWorkerId!({}))

	rpcInvoke : Str, Str, Str, Str -> Effect(Result(Str, Str))
	rpcInvoke = |targetWorker, constructor, fnName, payload|
		Effect(|_| Effect.rocFxRpcInvoke!(targetWorker, constructor, fnName, payload))

	rpcSchedule : Str, Str, Str, Str, U64 -> Effect(Result(Str, Str))
	rpcSchedule = |targetWorker, constructor, fnName, payload, delayMillis|
		Effect(|_| Effect.rocFxRpcSchedule!(targetWorker, constructor, fnName, payload, delayMillis))

	rpcFireAndForget : Str, Str, Str, Str -> Effect(Result(Str, Str))
	rpcFireAndForget = |targetWorker, constructor, fnName, payload|
		Effect(|_| Effect.rocFxRpcFireAndForget!(targetWorker, constructor, fnName, payload))

	rpcInvokeBytes : Str, Str, Str, List(U8) -> Effect(Result(List(U8), Str))
	rpcInvokeBytes = |targetWorker, constructor, fnName, payload|
		Effect(|_| Effect.rocFxRpcInvokeBytes!(targetWorker, constructor, fnName, payload))

	rpcScheduleBytes : Str, Str, Str, List(U8), U64 -> Effect(Result(List(U8), Str))
	rpcScheduleBytes = |targetWorker, constructor, fnName, payload, delayMillis|
		Effect(|_| Effect.rocFxRpcScheduleBytes!(targetWorker, constructor, fnName, payload, delayMillis))

	rpcFireAndForgetBytes : Str, Str, Str, List(U8) -> Effect(Result({}, Str))
	rpcFireAndForgetBytes = |targetWorker, constructor, fnName, payload|
		Effect(|_| Effect.rocFxRpcFireAndForgetBytes!(targetWorker, constructor, fnName, payload))

	setPersistenceMode : PersistenceMode -> Effect({})
	setPersistenceMode = |mode|
		Effect(
			|_| {
				modeU8 : U8
				modeU8 =
					match mode {
						PersistNothing => 0
						PersistStateOnly => 1
						PersistEverything => 2
					}
				Effect.rocFxSetPersistence!(modeU8)
			},
		)

	httpRequest : Str -> Effect(Result(Str, Str))
	httpRequest = |reqJson|
		Effect(|_| Effect.rocFxHttpRequest!(reqJson))

	wsConnect : Str -> Effect(Result(U32, Str))
	wsConnect = |url|
		Effect(|_| Effect.rocFxWsConnect!(url))

	wsSend : U32, Str -> Effect(Result({}, Str))
	wsSend = |handle, msg|
		Effect(|_| Effect.rocFxWsSend!(handle, msg))

	wsReceive : U32 -> Effect(Result(Str, Str))
	wsReceive = |handle|
		Effect(|_| Effect.rocFxWsReceive!(handle))

	wsClose : U32 -> Effect(Result({}, Str))
	wsClose = |handle|
		Effect(|_| Effect.rocFxWsClose!(handle))

	sleepMillis : U64 -> Effect({})
	sleepMillis = |millis|
		Effect(|_| Effect.rocFxSleepMillis!(millis))

	nowMillis : Effect(U64)
	nowMillis =
		Effect(|_| Effect.rocFxNowMillis!({}))

	getSecretHandle : Str -> Effect(Result(U32, Str))
	getSecretHandle = |name|
		Effect(|_| Effect.rocFxGetSecretHandle!(name))

	revealSecret : U32 -> Effect(Result(Str, Str))
	revealSecret = |handle|
		Effect(|_| Effect.rocFxRevealSecret!(handle))

	generateIdempotencyKey : Effect(Result(Str, Str))
	generateIdempotencyKey =
		Effect(|_| Effect.rocFxGenerateIdempotencyKey!({}))

	getOplogIndex : Effect(U64)
	getOplogIndex =
		Effect(|_| Effect.rocFxGetOplogIndex!({}))

	oplogCommit : U8 -> Effect({})
	oplogCommit = |replicas|
		Effect(|_| Effect.rocFxOplogCommit!(replicas))

	createPromise : Effect(Result(U64, Str))
	createPromise =
		Effect(|_| Effect.rocFxCreatePromise!({}))

	completePromise : U64, List(U8) -> Effect(Result(Bool, Str))
	completePromise = |handle, data|
		Effect(|_| Effect.rocFxCompletePromise!(handle, data))

	getPromise : U64 -> Effect(Result(List(U8), Str))
	getPromise = |handle|
		Effect(|_| Effect.rocFxGetPromise!(handle))

	createWebhook : U64 -> Effect(Result(Str, Str))
	createWebhook = |handle|
		Effect(|_| Effect.rocFxCreateWebhook!(handle))

	httpRequestBytes : Str, List(U8) -> Effect(Result(List(U8), Str))
	httpRequestBytes = |reqJson, bodyBytes|
		Effect(|_| Effect.rocFxHttpRequestBytes!(reqJson, bodyBytes))

	readFileBytes : Str -> Effect(Result(List(U8), Str))
	readFileBytes = |path|
		Effect(|_| Effect.rocFxReadFileBytes!(path))

	writeFileBytes : Str, List(U8) -> Effect(Result({}, Str))
	writeFileBytes = |path, bytes|
		Effect(|_| Effect.rocFxWriteFileBytes!(path, bytes))

	appendFileBytes : Str, List(U8) -> Effect(Result({}, Str))
	appendFileBytes = |path, bytes|
		Effect(|_| Effect.rocFxAppendFileBytes!(path, bytes))

	readFile : Str -> Effect(Result(Str, Str))
	readFile = |path|
		Effect(|_| Effect.rocFxReadFile!(path))

	writeFile : Str, Str -> Effect(Result({}, Str))
	writeFile = |path, content|
		Effect(|_| Effect.rocFxWriteFile!(path, content))

	kvGetBytes : Str, Str -> Effect(Result(List(U8), Str))
	kvGetBytes = |bucket, key|
		Effect(|_| Effect.rocFxKvGetBytes!(bucket, key))

	kvSetBytes : Str, Str, List(U8) -> Effect(Result({}, Str))
	kvSetBytes = |bucket, key, bytes|
		Effect(|_| Effect.rocFxKvSetBytes!(bucket, key, bytes))

	kvGet : Str, Str -> Effect(Result(Str, Str))
	kvGet = |bucket, key|
		Effect(|_| Effect.rocFxKvGet!(bucket, key))

	kvSet : Str, Str, Str -> Effect(Result({}, Str))
	kvSet = |bucket, key, val|
		Effect(|_| Effect.rocFxKvSet!(bucket, key, val))

	blobGetBytes : Str, Str -> Effect(Result(List(U8), Str))
	blobGetBytes = |container, object|
		Effect(|_| Effect.rocFxBlobGetBytes!(container, object))

	blobPutBytes : Str, Str, List(U8) -> Effect(Result({}, Str))
	blobPutBytes = |container, object, bytes|
		Effect(|_| Effect.rocFxBlobPutBytes!(container, object, bytes))

	blobGet : Str, Str -> Effect(Result(Str, Str))
	blobGet = |container, object|
		Effect(|_| Effect.rocFxBlobGet!(container, object))

	blobPut : Str, Str, Str -> Effect(Result({}, Str))
	blobPut = |container, object, val|
		Effect(|_| Effect.rocFxBlobPut!(container, object, val))

	configGet : Str -> Effect(Result(Str, Str))
	configGet = |key|
		Effect(|_| Effect.rocFxConfigGet!(key))

	traceId : Effect(Str)
	traceId =
		Effect(|_| Effect.rocFxTraceId!({}))

	spanId : Effect(Str)
	spanId =
		Effect(|_| Effect.rocFxSpanId!({}))

	traceHeaders : Effect(Result(Str, Str))
	traceHeaders =
		Effect(|_| Effect.rocFxTraceHeaders!({}))

	getIdempotenceMode : Effect(Bool)
	getIdempotenceMode =
		Effect(|_| Effect.rocFxGetIdempotenceMode!({}))

	setIdempotenceMode : Bool -> Effect({})
	setIdempotenceMode = |mode|
		Effect(|_| Effect.rocFxSetIdempotenceMode!(mode))

	revertAgent : U64 -> Effect(Result({}, Str))
	revertAgent = |oplogIndex|
		Effect(|_| Effect.rocFxRevertAgent!(oplogIndex))

	dbExecute : Str, Str -> Effect(Result(U64, Str))
	dbExecute = |connStr, sql|
		Effect(|_| Effect.rocFxDbExecute!(connStr, sql))

	dbExecuteParams : Str, Str, Str -> Effect(Result(U64, Str))
	dbExecuteParams = |connStr, sql, paramsJson|
		Effect(|_| Effect.rocFxDbExecuteParams!(connStr, sql, paramsJson))

	dbQuery : Str, Str -> Effect(Result(Str, Str))
	dbQuery = |connStr, sql|
		Effect(|_| Effect.rocFxDbQuery!(connStr, sql))

	dbQueryParams : Str, Str, Str -> Effect(Result(Str, Str))
	dbQueryParams = |connStr, sql, paramsJson|
		Effect(|_| Effect.rocFxDbQueryParams!(connStr, sql, paramsJson))

	httpStreamOpen : Str, List(U8) -> Effect(Result(U32, Str))
	httpStreamOpen = |reqJson, bodyBytes|
		Effect(|_| Effect.rocFxHttpStreamOpen!(reqJson, bodyBytes))

	httpStreamReadChunk : U32, U64 -> Effect(Result(List(U8), Str))
	httpStreamReadChunk = |handle, maxBytes|
		Effect(|_| Effect.rocFxHttpStreamRead!(handle, maxBytes))

	httpStreamClose : U32 -> Effect(Result({}, Str))
	httpStreamClose = |handle|
		Effect(|_| Effect.rocFxHttpStreamClose!(handle))

}
