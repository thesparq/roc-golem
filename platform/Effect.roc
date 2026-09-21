module [
	log!,
	getWorkerId!,
	rpcInvoke!,
	setPersistenceMode!,
	httpRequest!,
	wsConnect!,
	wsSend!,
	wsReceive!,
	wsClose!,
	sleepMillis!,
	nowMillis!,
]

import Types exposing [LogLevel, PersistenceMode]
import Host

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

rpcInvoke! : Str, Str, Str => Try(Str, Str)
rpcInvoke! = |targetWorker, fnName, payload| {
	Host.rocFxRpcInvoke!(targetWorker, fnName, payload)
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
