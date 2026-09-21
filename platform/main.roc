platform "golem"
	requires {
		[State : state] for agent : _
	}
	exposes [
		Golem,
		Types,
	]
	packages {}
	provides {
		"main_init_for_host_1_exposed_generic": main_init_for_host,
		"main_handle_message_for_host_1_exposed_generic": main_handle_message_for_host,
		"main_handle_tool_call_for_host_1_exposed_generic": main_handle_tool_call_for_host,
		"main_metadata_for_host_1_exposed_generic": main_metadata_for_host,
	}
	hosted {
		"rocFxLog": Host.rocFxLog!,
		"rocFxGetWorkerId": Host.rocFxGetWorkerId!,
		"rocFxRpcInvoke": Host.rocFxRpcInvoke!,
		"rocFxSetPersistence": Host.rocFxSetPersistence!,
		"rocFxHttpRequest": Host.rocFxHttpRequest!,
		"rocFxWsConnect": Host.rocFxWsConnect!,
		"rocFxWsSend": Host.rocFxWsSend!,
		"rocFxWsReceive": Host.rocFxWsReceive!,
		"rocFxWsClose": Host.rocFxWsClose!,
		"rocFxSleepMillis": Host.rocFxSleepMillis!,
		"rocFxNowMillis": Host.rocFxNowMillis!,
	}

import Types exposing [
	ToolDefinition,
	ToolParameter,
	ToolCall,
	ToolResult,
	Metadata,
]
import Golem
import Host

main_init_for_host : Str => Try(Str, Str)
main_init_for_host = |encoded_args| {
	_state = (agent.init!)(encoded_args)
	# TODO JSON encode
	Ok("TODO_ENCODED_STATE")
}

main_handle_message_for_host : Str, Str => Try(Str, Str)
main_handle_message_for_host = |_encoded_state, _encoded_msg| {
	Ok("TODO_ENCODED_STATE_AND_REPLIES")
}

main_handle_tool_call_for_host : Str, Str => Try(Str, Str)
main_handle_tool_call_for_host = |_encoded_state, _encoded_tool_call| {
	Ok("TODO_ENCODED_STATE_AND_TOOL_RESULT")
}

main_metadata_for_host : {} => Metadata
main_metadata_for_host = |_| {
	agent.metadata
}
