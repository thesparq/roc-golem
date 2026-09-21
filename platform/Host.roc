Host :: [].{
	rocFxLog! : U8, Str => {}
	rocFxGetWorkerId! : {} => Str
	rocFxRpcInvoke! : Str, Str, Str => Try(Str, Str)
	rocFxSetPersistence! : U8 => {}
	rocFxHttpRequest! : Str => Try(Str, Str)
	rocFxWsConnect! : Str => Try(U32, Str)
	rocFxWsSend! : U32, Str => Try({}, Str)
	rocFxWsReceive! : U32 => Try(Str, Str)
	rocFxWsClose! : U32 => Try({}, Str)
	rocFxSleepMillis! : U64 => {}
	rocFxNowMillis! : {} => U64
}
