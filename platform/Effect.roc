module [
	Effect,
	map,
	log,
	getWorkerId,
	rpcInvoke,
	rpcSchedule,
	rpcFireAndForget,
	setPersistenceMode,
	httpRequest,
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
	wsConnect,
	wsSend,
	wsReceive,
	wsClose,
	sleepMillis,
	nowMillis,
	getSecretHandle,
	revealSecret,
	run,
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
	rpcInvokeBytes,
	rpcScheduleBytes,
	rpcFireAndForgetBytes,
	httpStreamOpen,
	httpStreamReadChunk,
	httpStreamClose,
	dbExecuteParams,
	dbQueryParams,
]

import Types exposing [LogLevel, PersistenceMode]

Effect(a) :: {} -> a

run : Effect(a) -> a
run = |Effect(thunk)| thunk({})

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
			rocFxLog(levelU8, msg)
		},
	)

getWorkerId : Effect(Str)
getWorkerId =
	Effect(|_| rocFxGetWorkerId({}))

rpcInvoke : Str, Str, Str -> Effect(Result(Str, Str))
rpcInvoke = |targetWorker, fnName, payload|
	Effect(|_| rocFxRpcInvoke(targetWorker, fnName, payload))

rpcSchedule : Str, Str, Str, U64 -> Effect(Result(Str, Str))
rpcSchedule = |targetWorker, fnName, payload, delayMillis|
	Effect(|_| rocFxRpcSchedule(targetWorker, fnName, payload, delayMillis))

rpcFireAndForget : Str, Str, Str -> Effect(Result(Str, Str))
rpcFireAndForget = |targetWorker, fnName, payload|
	Effect(|_| rocFxRpcFireAndForget(targetWorker, fnName, payload))

rpcInvokeBytes : Str, Str, List(U8) -> Effect(Result(List(U8), Str))
rpcInvokeBytes = |targetWorker, fnName, payload|
	Effect(|_| rocFxRpcInvokeBytes(targetWorker, fnName, payload))

rpcScheduleBytes : Str, Str, List(U8), U64 -> Effect(Result(List(U8), Str))
rpcScheduleBytes = |targetWorker, fnName, payload, delayMillis|
	Effect(|_| rocFxRpcScheduleBytes(targetWorker, fnName, payload, delayMillis))

rpcFireAndForgetBytes : Str, Str, List(U8) -> Effect(Result({}, Str))
rpcFireAndForgetBytes = |targetWorker, fnName, payload|
	Effect(|_| rocFxRpcFireAndForgetBytes(targetWorker, fnName, payload))

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
			rocFxSetPersistence(modeU8)
		},
	)

httpRequest : Str -> Effect(Result(Str, Str))
httpRequest = |reqJson|
	Effect(|_| rocFxHttpRequest(reqJson))

wsConnect : Str -> Effect(Result(U32, Str))
wsConnect = |url|
	Effect(|_| rocFxWsConnect(url))

wsSend : U32, Str -> Effect(Result({}, Str))
wsSend = |handle, msg|
	Effect(|_| rocFxWsSend(handle, msg))

wsReceive : U32 -> Effect(Result(Str, Str))
wsReceive = |handle|
	Effect(|_| rocFxWsReceive(handle))

wsClose : U32 -> Effect(Result({}, Str))
wsClose = |handle|
	Effect(|_| rocFxWsClose(handle))

sleepMillis : U64 -> Effect({})
sleepMillis = |millis|
	Effect(|_| rocFxSleepMillis(millis))

nowMillis : Effect(U64)
nowMillis =
	Effect(|_| rocFxNowMillis({}))

getSecretHandle : Str -> Effect(Result(U32, Str))
getSecretHandle = |name|
	Effect(|_| rocFxGetSecretHandle(name))

revealSecret : U32 -> Effect(Result(Str, Str))
revealSecret = |handle|
	Effect(|_| rocFxRevealSecret(handle))

generateIdempotencyKey : Effect(Result(Str, Str))
generateIdempotencyKey =
	Effect(|_| rocFxGenerateIdempotencyKey({}))

getOplogIndex : Effect(U64)
getOplogIndex =
	Effect(|_| rocFxGetOplogIndex({}))

oplogCommit : U8 -> Effect({})
oplogCommit = |replicas|
	Effect(|_| rocFxOplogCommit(replicas))

createPromise : Effect(Result(U64, Str))
createPromise =
	Effect(|_| rocFxCreatePromise({}))

completePromise : U64, List(U8) -> Effect(Result(Bool, Str))
completePromise = |handle, data|
	Effect(|_| rocFxCompletePromise(handle, data))

getPromise : U64 -> Effect(Result(List(U8), Str))
getPromise = |handle|
	Effect(|_| rocFxGetPromise(handle))

createWebhook : U64 -> Effect(Result(Str, Str))
createWebhook = |handle|
	Effect(|_| rocFxCreateWebhook(handle))

rocFxLog : U8, Str -> {}

rocFxGetWorkerId : {} -> Str

rocFxRpcInvoke : Str, Str, Str -> Result(Str, Str)

rocFxRpcSchedule : Str, Str, Str, U64 -> Result(Str, Str)

rocFxRpcFireAndForget : Str, Str, Str -> Result(Str, Str)

rocFxSetPersistence : U8 -> {}

rocFxHttpRequest : Str -> Result(Str, Str)

rocFxWsConnect : Str -> Result(U32, Str)

rocFxWsSend : U32, Str -> Result({}, Str)

rocFxWsReceive : U32 -> Result(Str, Str)

rocFxWsClose : U32 -> Result({}, Str)

rocFxSleepMillis : U64 -> {}

rocFxNowMillis : {} -> U64

httpRequestBytes : Str, List(U8) -> Effect(Result(List(U8), Str))
httpRequestBytes = |reqJson, bodyBytes|
	Effect(|_| rocFxHttpRequestBytes(reqJson, bodyBytes))

readFileBytes : Str -> Effect(Result(List(U8), Str))
readFileBytes = |path|
	Effect(|_| rocFxReadFileBytes(path))

writeFileBytes : Str, List(U8) -> Effect(Result({}, Str))
writeFileBytes = |path, bytes|
	Effect(|_| rocFxWriteFileBytes(path, bytes))

appendFileBytes : Str, List(U8) -> Effect(Result({}, Str))
appendFileBytes = |path, bytes|
	Effect(|_| rocFxAppendFileBytes(path, bytes))

readFile : Str -> Effect(Result(Str, Str))
readFile = |path|
	Effect(|_| rocFxReadFile(path))

writeFile : Str, Str -> Effect(Result({}, Str))
writeFile = |path, content|
	Effect(|_| rocFxWriteFile(path, content))

kvGetBytes : Str, Str -> Effect(Result(List(U8), Str))
kvGetBytes = |bucket, key|
	Effect(|_| rocFxKvGetBytes(bucket, key))

kvSetBytes : Str, Str, List(U8) -> Effect(Result({}, Str))
kvSetBytes = |bucket, key, bytes|
	Effect(|_| rocFxKvSetBytes(bucket, key, bytes))

kvGet : Str, Str -> Effect(Result(Str, Str))
kvGet = |bucket, key|
	Effect(|_| rocFxKvGet(bucket, key))

kvSet : Str, Str, Str -> Effect(Result({}, Str))
kvSet = |bucket, key, val|
	Effect(|_| rocFxKvSet(bucket, key, val))

blobGetBytes : Str, Str -> Effect(Result(List(U8), Str))
blobGetBytes = |container, object|
	Effect(|_| rocFxBlobGetBytes(container, object))

blobPutBytes : Str, Str, List(U8) -> Effect(Result({}, Str))
blobPutBytes = |container, object, bytes|
	Effect(|_| rocFxBlobPutBytes(container, object, bytes))

blobGet : Str, Str -> Effect(Result(Str, Str))
blobGet = |container, object|
	Effect(|_| rocFxBlobGet(container, object))

blobPut : Str, Str, Str -> Effect(Result({}, Str))
blobPut = |container, object, val|
	Effect(|_| rocFxBlobPut(container, object, val))

rocFxReadFileBytes : Str -> Result(List(U8), Str)

rocFxWriteFileBytes : Str, List(U8) -> Result({}, Str)

rocFxAppendFileBytes : Str, List(U8) -> Result({}, Str)

rocFxReadFile : Str -> Result(Str, Str)

rocFxWriteFile : Str, Str -> Result({}, Str)

rocFxKvGetBytes : Str, Str -> Result(List(U8), Str)

rocFxKvSetBytes : Str, Str, List(U8) -> Result({}, Str)

rocFxKvGet : Str, Str -> Result(Str, Str)

rocFxKvSet : Str, Str, Str -> Result({}, Str)

rocFxBlobGetBytes : Str, Str -> Result(List(U8), Str)

rocFxBlobPutBytes : Str, Str, List(U8) -> Result({}, Str)

rocFxBlobGet : Str, Str -> Result(Str, Str)

rocFxBlobPut : Str, Str, Str -> Result({}, Str)

rocFxHttpRequestBytes : Str, List(U8) -> Result(List(U8), Str)

rocFxGetSecretHandle : Str -> Result(U32, Str)

rocFxRevealSecret : U32 -> Result(Str, Str)

rocFxGenerateIdempotencyKey : {} -> Result(Str, Str)

rocFxGetOplogIndex : {} -> U64

rocFxOplogCommit : U8 -> {}

rocFxCreatePromise : {} -> Result(U64, Str)

rocFxCompletePromise : U64, List(U8) -> Result(Bool, Str)

rocFxGetPromise : U64 -> Result(List(U8), Str)

rocFxCreateWebhook : U64 -> Result(Str, Str)

configGet : Str -> Effect(Result(Str, Str))
configGet = |key|
	Effect(|_| rocFxConfigGet(key))

traceId : Effect(Str)
traceId =
	Effect(|_| rocFxTraceId({}))

spanId : Effect(Str)
spanId =
	Effect(|_| rocFxSpanId({}))

traceHeaders : Effect(Result(Str, Str))
traceHeaders =
	Effect(|_| rocFxTraceHeaders({}))

getIdempotenceMode : Effect(Bool)
getIdempotenceMode =
	Effect(|_| rocFxGetIdempotenceMode({}))

setIdempotenceMode : Bool -> Effect({})
setIdempotenceMode = |mode|
	Effect(|_| rocFxSetIdempotenceMode(mode))

revertAgent : U64 -> Effect(Result({}, Str))
revertAgent = |oplogIndex|
	Effect(|_| rocFxRevertAgent(oplogIndex))

dbExecute : Str, Str -> Effect(Result(U64, Str))
dbExecute = |connStr, sql|
	Effect(|_| rocFxDbExecute(connStr, sql))

dbExecuteParams : Str, Str, Str -> Effect(Result(U64, Str))
dbExecuteParams = |connStr, sql, paramsJson|
	Effect(|_| rocFxDbExecuteParams(connStr, sql, paramsJson))

dbQuery : Str, Str -> Effect(Result(Str, Str))
dbQuery = |connStr, sql|
	Effect(|_| rocFxDbQuery(connStr, sql))

dbQueryParams : Str, Str, Str -> Effect(Result(Str, Str))
dbQueryParams = |connStr, sql, paramsJson|
	Effect(|_| rocFxDbQueryParams(connStr, sql, paramsJson))

httpStreamOpen : Str, List(U8) -> Effect(Result(U32, Str))
httpStreamOpen = |reqJson, bodyBytes|
	Effect(|_| rocFxHttpStreamOpen(reqJson, bodyBytes))

httpStreamReadChunk : U32, U64 -> Effect(Result(List(U8), Str))
httpStreamReadChunk = |handle, maxBytes|
	Effect(|_| rocFxHttpStreamRead(handle, maxBytes))

httpStreamClose : U32 -> Effect(Result({}, Str))
httpStreamClose = |handle|
	Effect(|_| rocFxHttpStreamClose(handle))

rocFxConfigGet : Str -> Result(Str, Str)

rocFxTraceId : {} -> Str

rocFxSpanId : {} -> Str

rocFxTraceHeaders : {} -> Result(Str, Str)

rocFxGetIdempotenceMode : {} -> Bool

rocFxSetIdempotenceMode : Bool -> {}

rocFxRevertAgent : U64 -> Result({}, Str)

rocFxDbExecute : Str, Str -> Result(U64, Str)

rocFxDbExecuteParams : Str, Str, Str -> Result(U64, Str)

rocFxDbQuery : Str, Str -> Result(Str, Str)

rocFxDbQueryParams : Str, Str, Str -> Result(Str, Str)

rocFxRpcInvokeBytes : Str, Str, List(U8) -> Result(List(U8), Str)

rocFxRpcScheduleBytes : Str, Str, List(U8), U64 -> Result(List(U8), Str)

rocFxRpcFireAndForgetBytes : Str, Str, List(U8) -> Result({}, Str)

rocFxHttpStreamOpen : Str, List(U8) -> Result(U32, Str)

rocFxHttpStreamRead : U32, U64 -> Result(List(U8), Str)

rocFxHttpStreamClose : U32 -> Result({}, Str)
