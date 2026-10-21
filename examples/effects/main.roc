app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, defineAgent]
import pf.Golem exposing [
	awaitWebhook!,
	createWebhook!,
	fork!,
	http!,
	idempotencyKey!,
	logInfo!,
	makeAgentId!,
	parseAgentId!,
	rpc!,
	rpcCancel!,
	rpcFireAndForget!,
	rpcSchedule!,
	rpcScheduleCancelable!,
]

# Agent State
#
# `baseUrl` is taken from the agent config when it looks like a URL, so a test
# harness can hand the agent an address to call; otherwise it falls back to a
# local default. `webhookUrl` is the callback URL of the webhook this agent
# created (`webhook-create`), kept in state because the webhook lives across
# invocations: the POST arrives while no invocation is running. `lastForkId` and
# `lastKey` are what the `fork` and `idempotency` commands compare the next
# invocation against.
State :: {
	baseUrl : Str,
	lastStatus : Str,
	lastBody : Str,
	lastRpc : Str,
	# Handle of the most recent cancelable scheduled invocation; 0 means there
	# is none, which is what `cancel` reports back.
	cancelHandle : U32,
	webhookUrl : Str,
	# The phantom id of the copy the last `fork` created.
	lastForkId : Str,
	# The first idempotency key the last `idempotency` command generated.
	lastKey : Str,
}

agent : Agent(State)
agent = defineAgent({
	init!: |config|
		{
			baseUrl: if (config_starts_with_http(config)) config else "http://127.0.0.1:18080/",
			lastStatus: "none",
			lastBody: "",
			lastRpc: "none",
			cancelHandle: 0,
			webhookUrl: "",
			lastForkId: "",
			lastKey: "",
		},

	handleMessage!: |state, message|
		match message {
			"fetch" => {
				request = "{\"url\":\"${state.baseUrl}\",\"method\":\"GET\"}"
				match http!(request) {
					Ok(responseJson) => {
						status = extract_json_str(responseJson, "status") ?? "unknown"
						body = extract_json_str(responseJson, "body") ?? ""
						logInfo!("http call returned status ${status}")
						{
							state: { ..state, lastStatus: status, lastBody: quote_free(body) },
							# The platform hands the whole response over as JSON
							# (`{status, headers, body}`); parsing the body properly is
							# the app's business, so this example only reports its size.
							replies: ["HTTP ${status} (${U64.to_str(Str.count_utf8_bytes(responseJson))} byte response)"],
						}
					}

					Err(err) =>
						{
							state,
							replies: ["HTTP failed: ${err}"],
						}
				}
			}

			"rpc" =>
			# The target is an agent id: <agent-type>(<constructor params>). Golem
			# 1.6 resolves the target's agent type from the deployment's registered
			# types, so this can be another component's agent; this arm calls another
			# instance of itself for a cheap check of the same component, while the
			# `runCommand` commands below call the counter agent.
				match rpc!("effects-agent(\"other\")", "handle-message", "status") {
					Ok(reply) =>
						{
							state: { ..state, lastRpc: reply },
							replies: ["RPC reply: ${reply}"],
						}

					Err(err) =>
						{
							state,
							replies: ["RPC failed: ${err}"],
						}
				}

			"status" =>
				{
					state,
					replies: ["effects-agent is up"],
				}

			"cancel" =>
				match rpcCancel!(state.cancelHandle) {
					Ok(_) =>
						{
							state: { ..state, cancelHandle: 0 },
							replies: ["Canceled scheduled invocation (handle ${U32.to_str(state.cancelHandle)})"],
						}

					Err(err) =>
						{
							state,
							replies: ["Cancel failed: ${err}"],
						}
				}

			_ =>
				runCommand(state, message)
		},

	handleToolCall!: |state, toolCall|
		{
			state,
			result: {
				id: toolCall.id,
				success: False,
				output: "{\"error\": \"Unknown tool ${toolCall.name}\"}",
				stdout: "",
				stderr: "",
			},
		},

	metadata: {
		name: "effects-agent",
		version: "1.0.0",
		description: "Calls HTTP endpoints and other agents from a Golem Cloud agent",
		tools: [],
		config: [],
		agentKind: Regular,
		# Golem only lets agent types deployed through an HTTP API create
		# webhooks, and a deployment entry needs a mount. This agent serves no
		# files, so the mount carries no bindings; its `{id}` capture is the
		# constructor parameter, the way every regular mount is bound.
		httpMount: Mount({
			pathPrefix: ["effects", "{id}"],
			staticBindings: [],
			filesystemBindings: [],
			fileResponseHeaders: [],
			corsAllowedPatterns: [],
		}),
		openapiProvider: NoOpenApi,
	},

	serializeState: |state|
		"{\"baseUrl\":\"${state.baseUrl}\",\"lastStatus\":\"${state.lastStatus}\",\"lastBody\":\"${state.lastBody}\",\"lastRpc\":\"${state.lastRpc}\",\"cancelHandle\":${U32.to_str(state.cancelHandle)},\"webhookUrl\":\"${state.webhookUrl}\",\"lastForkId\":\"${state.lastForkId}\",\"lastKey\":\"${state.lastKey}\"}",

	deserializeState: |stateJson|
		Ok({
			baseUrl: extract_json_str(stateJson, "baseUrl") ?? "http://127.0.0.1:18080/",
			lastStatus: extract_json_str(stateJson, "lastStatus") ?? "none",
			lastBody: extract_json_str(stateJson, "lastBody") ?? "",
			lastRpc: extract_json_str(stateJson, "lastRpc") ?? "none",
			cancelHandle: U32.from_str(extract_json_str(stateJson, "cancelHandle") ?? "0") ?? 0,
			webhookUrl: extract_json_str(stateJson, "webhookUrl") ?? "",
			lastForkId: extract_json_str(stateJson, "lastForkId") ?? "",
			lastKey: extract_json_str(stateJson, "lastKey") ?? "",
		}),
})

## Commands that call another agent — everything `golem:agent/host` offers
## beyond `rpc!` — are `<verb> <arguments…>`, with one space-separated token per
## argument. Every verb's arguments are listed in the usage reply below.
runCommand : State, Str => { state : State, replies : List(Str) }
runCommand = |state, message| {
	command = splitCommand(message)

	match command.verb {
		"fire-and-forget" =>
		# fire-and-forget <target> <method> <payload>
			match rpcFireAndForget!(command.a, command.b, command.c) {
				Ok(identity) =>
					{
						state: { ..state, lastRpc: quote_free(identity) },
						replies: ["Fire-and-forget sent: ${identity}"],
					}

				Err(err) =>
					{
						state,
						replies: ["Fire-and-forget failed: ${err}"],
					}
			}

		"schedule" =>
		# schedule <target> <method> <payload> <delayMillis>
			match U64.from_str(command.d) {
				Err(_) =>
					{
						state,
						replies: ["schedule needs a delay in milliseconds"],
					}

				Ok(delayMillis) =>
					match rpcSchedule!(command.a, command.b, command.c, delayMillis) {
						Ok(identity) =>
							{
								state: { ..state, lastRpc: quote_free(identity) },
								replies: ["Scheduled in ${U64.to_str(delayMillis)}ms: ${identity}"],
							}

						Err(err) =>
							{
								state,
								replies: ["Scheduling failed: ${err}"],
							}
					}
			}

		"schedule-cancelable" =>
		# schedule-cancelable <target> <method> <payload> <delayMillis>
			match U64.from_str(command.d) {
				Err(_) =>
					{
						state,
						replies: ["schedule-cancelable needs a delay in milliseconds"],
					}

				Ok(delayMillis) =>
					match rpcScheduleCancelable!(command.a, command.b, command.c, delayMillis) {
						Ok(handle) =>
							{
								state: { ..state, cancelHandle: handle },
								replies: ["Cancelable scheduled in ${U64.to_str(delayMillis)}ms: handle ${U32.to_str(handle)}"],
							}

						Err(err) =>
							{
								state,
								replies: ["Cancelable scheduling failed: ${err}"],
							}
					}
			}

		"parse-agent-id" =>
		# parse-agent-id <agent-id>
			match parseAgentId!(command.a) {
				Ok(details) =>
					{
						state: { ..state, lastRpc: quote_free(details) },
						replies: ["Parsed agent id: ${details}"],
					}

				Err(err) =>
					{
						state,
						replies: ["Parsing failed: ${err}"],
					}
			}

		"make-agent-id" =>
		# make-agent-id <agent-type-name> <constructor-parameter>
			match makeAgentId!(command.a, command.b) {
				Ok(agentId) =>
					{
						state: { ..state, lastRpc: quote_free(agentId) },
						replies: ["Made agent id: ${agentId}"],
					}

				Err(err) =>
					{
						state,
						replies: ["Making an agent id failed: ${err}"],
					}
			}

		"principal-rpc" =>
		# principal-rpc <target>: calls the target's `permission principal`
		# command through `rpc!` and reports what the *callee* sees as its
		# principal. An invocation that came from another agent arrives as an
		# `agent` principal, so this is what shows the runtime handing the
		# caller's own identity over instead of the caller's user.
			match rpc!(command.a, "handle-message", "permission principal") {
				Ok(reply) =>
					{
						state: { ..state, lastRpc: quote_free(reply) },
						replies: ["RPC principal: ${reply}"],
					}

				Err(err) =>
					{
						state,
						replies: ["RPC principal failed: ${err}"],
					}
			}

		"webhook-create" =>
		# Creates the promise + webhook pair and stores the callback URL; a POST
		# to that URL (from outside Golem) completes the promise.
			match createWebhook!({}) {
				Ok(url) =>
					{
						state: { ..state, webhookUrl: quote_free(url) },
						replies: ["Webhook created: ${url}"],
					}

				Err(err) =>
					{
						state,
						replies: ["Webhook creation failed: ${err}"],
					}
			}

		"webhook-await" =>
		# Waits for the POST that completes the stored URL's promise and returns
		# its body.
			match awaitWebhook!(state.webhookUrl) {
				Ok(body) =>
					{
						state,
						replies: ["Webhook body: ${body}"],
					}

				Err(err) =>
					{
						state,
						replies: ["Webhook await failed: ${err}"],
					}
			}

		"fork" =>
		# fork <target> <method> <payload>: forks this agent at this point. The
		# copy the caller is invoking is the original and answers with the fork's
		# phantom id; the forked copy is a separate agent that calls the target,
		# which is how the e2e observes both sides of the fork.
			forkAndReport!(state, command)

		"idempotency" => {
			# Two keys from this invocation, and the first key of the previous one:
			# each call sits at its own oplog position, so the two here differ from
			# each other and from the earlier invocation's, while a replay of this
			# invocation hands the same positions the same keys back.
			first = idempotencyKey!({})
			second = idempotencyKey!({})
			previous = if (state.lastKey == "") "none" else state.lastKey
			same = if (first == second) "true" else "false"
			{
				state: { ..state, lastKey: first },
				replies: ["Idempotency keys: first=${first} second=${second} agree=${same} previous-first=${previous}"],
			}
		}

		_ =>
			{
				state,
				replies: ["Commands: fetch, rpc, cancel, principal-rpc <id>, webhook-create, webhook-await, idempotency, fork <id> <method> <payload>, fire-and-forget <id> <method> <payload>, schedule <id> <method> <payload> <millis>, schedule-cancelable <id> <method> <payload> <millis>, parse-agent-id <id>, make-agent-id <type> <param>"],
			}
	}
}

## Forks this agent, then reports on the side this execution is: the original
## answers the invocation the caller is waiting for, and the forked copy — a new
## agent with the same agent id plus the phantom id in `fork-result` — calls the
## target agent, so the e2e can see that the copy really continued.
## `command.a`/`b`/`c` are the target, method and payload of that call.
forkAndReport! : State, Command => { state : State, replies : List(Str) }
forkAndReport! = |state, command|
	match fork!({}) {
		Err(err) =>
			{
				state,
				replies: ["Forking failed: ${err}"],
			}

		Ok(result) =>
			match Str.split_first(result, ":") {
				Err(_) =>
					{
						state,
						replies: ["Fork returned an unexpected result: ${result}"],
					}

				Ok({ before: side, after: forkedId }) =>
					if (side == "forked")
						match rpc!(command.a, command.b, command.c) {
							Ok(reply) =>
								{
									state,
									replies: ["Forked copy ${forkedId} called ${command.a}: ${reply}"],
								}

							Err(err) =>
								{
									state,
									replies: ["Forked copy ${forkedId} could not call ${command.a}: ${err}"],
								}
						}
					else {
						previous = if (state.lastForkId == "") "none" else state.lastForkId
						{
							state: { ..state, lastForkId: forkedId },
							replies: ["Forked copy ${forkedId} (original side; previous fork: ${previous})"],
						}
					}
			}
	}

Command : {
	verb : Str,
	a : Str,
	b : Str,
	c : Str,
	d : Str,
}

## Splits a command message into its tokens; missing trailing ones come back
## empty, so each verb reads only the arguments it takes.
splitCommand : Str -> Command
splitCommand = |message| {
	verb = splitToken(message)
	a = splitToken(verb.rest)
	b = splitToken(a.rest)
	c = splitToken(b.rest)
	d = splitToken(c.rest)
	{ verb: verb.first, a: a.first, b: b.first, c: c.first, d: d.first }
}

## Splits the first space-separated token off `text`; `rest` keeps what follows
## (empty when there is no space left).
splitToken : Str -> { first : Str, rest : Str }
splitToken = |text|
	match Str.split_first(text, " ") {
		Ok({ before, after }) => { first: before, rest: after }

		Err(_) => { first: text, rest: "" }
	}

## The state is embedded in JSON without an escaper, so keep stored strings free
## of characters that would break it. The reply itself is escaped by the
## platform.
quote_free : Str -> Str
quote_free = |text| {
	no_quotes = Str.replace_each(text, "\"", "'")
	Str.replace_each(no_quotes, "\\", "/")
}

config_starts_with_http : Str -> Bool
config_starts_with_http = |config|
	match Str.split_first(config, "http") {
		Ok(_) => True
		Err(_) => False
	}

## Minimal JSON field reader, matching the style used by the platform's own
## hooks: it looks for `"field":"` first and takes everything up to the closing
## quote, and falls back to reading a bare (numeric) value up to `,` or `}`,
## which is what the HTTP response's `status` is.
extract_json_str : Str, Str -> Try(Str, [NotFound])
extract_json_str = |json, field|
	match Str.split_first(json, "\"${field}\":\"") {
		Ok({ before: _, after }) =>
			match Str.split_first(after, "\"") {
				Ok({ before, after: _ }) => Ok(before)
				Err(_) => Err(NotFound)
			}
		Err(_) =>
			match Str.split_first(json, "\"${field}\":") {
				Ok({ before: _, after }) =>
					match Str.split_first(after, ",") {
						Ok({ before, after: _ }) => Ok(before)
						Err(_) =>
							match Str.split_first(after, "}") {
								Ok({ before, after: _ }) => Ok(before)
								Err(_) => Ok(after)
							}
					}
				Err(_) => Err(NotFound)
			}
	}
