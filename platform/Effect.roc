module [
	Effect,
	log,
	getWorkerId,
	rpcInvoke,
	setPersistenceMode,
	httpRequest,
	wsConnect,
	wsSend,
	wsReceive,
	wsClose,
	sleepMillis,
	nowMillis,
]

import Types exposing [LogLevel, PersistenceMode]

Effect(a) :: {} -> a

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

rocFxLog : U8, Str -> {}

rocFxGetWorkerId : {} -> Str

rocFxRpcInvoke : Str, Str, Str -> Result(Str, Str)

rocFxSetPersistence : U8 -> {}

rocFxHttpRequest : Str -> Result(Str, Str)

rocFxWsConnect : Str -> Result(U32, Str)

rocFxWsSend : U32, Str -> Result({}, Str)

rocFxWsReceive : U32 -> Result(Str, Str)

rocFxWsClose : U32 -> Result({}, Str)

rocFxSleepMillis : U64 -> {}

rocFxNowMillis : {} -> U64
