Host :: [].{
	rocFxLog! : U8, Str => {}
	rocFxGetWorkerId! : {} => Str
	rocFxCreateWebhook! : {} => Try(Str, Str)
	rocFxAwaitWebhook! : Str => Try(Str, Str)
	rocFxRpcInvoke! : Str, Str, Str => Try(Str, Str)
	rocFxRpcFireAndForget! : Str, Str, Str => Try(Str, Str)
	rocFxRpcSchedule! : Str, Str, Str, U64 => Try(Str, Str)
	rocFxRpcScheduleCancelable! : Str, Str, Str, U64 => Try(U32, Str)
	rocFxRpcCancel! : U32 => Try({}, Str)
	rocFxParseAgentId! : Str => Try(Str, Str)
	rocFxMakeAgentId! : Str, Str => Try(Str, Str)
	rocFxSetPersistence! : U8 => {}
	rocFxHttpRequest! : Str => Try(Str, Str)
	rocFxWsConnect! : Str => Try(U32, Str)
	rocFxWsSend! : U32, Str => Try({}, Str)
	rocFxWsReceive! : U32 => Try(Str, Str)
	rocFxWsClose! : U32 => Try({}, Str)
	rocFxSleepMillis! : U64 => {}
	rocFxNowMillis! : {} => U64
	rocFxGetConfig! : Str => Try(Str, Str)
}
