app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [
	Agent,
	appendFile!,
	blobCreateContainer!,
	blobDelete!,
	blobGet!,
	blobHas!,
	blobList!,
	blobPut!,
	defineAgent,
	deleteFile!,
	fileExists!,
	fileSize!,
	getConfig!,
	getRetryPolicies!,
	kvDelete!,
	kvExists!,
	kvGet!,
	kvSet!,
	listDir!,
	makeDir!,
	newQuotaToken!,
	nextRetryDelay!,
	oplogCommit!,
	quotaCommit!,
	quotaReserve!,
	readFile!,
	resolveAgentId!,
	resolveComponentId!,
	resolveRetryPolicy!,
	trap!,
	writeFile!,
]

State :: {
	count : I64,
}

agent : Agent(State)
agent = defineAgent({
	init!: |_config|
		{ count: 0 },

	handleMessage!: |state, message| {
		# Commands are a verb and, for the ones that take one, an argument, so the
		# dispatch reads the verb and the verb's own arm reads the rest.
		command = splitToken(message)

		match command.first {
			"increment" => {
				nextCount = state.count + 1
				{
					state: { count: nextCount },
					replies: ["Counter incremented to ${I64.to_str(nextCount)}"],
				}
			}

			"decrement" => {
				nextCount = state.count - 1
				{
					state: { count: nextCount },
					replies: ["Counter decremented to ${I64.to_str(nextCount)}"],
				}
			}

			"config" =>
				match getConfig!("greeting") {
					Ok(greeting) =>
						{
							state,
							replies: ["Config greeting: ${greeting}"],
						}

					Err(err) =>
						{
							state,
							replies: ["Config error: ${err}"],
						}
				}

			"get" =>
				{
					state,
					replies: ["Current count is ${I64.to_str(state.count)}"],
				}

			"quota" =>
			# quota <units>: reserves and commits that many units of the
			# `example-requests` resource the manifest declares, so the work a
			# method protected with a reservation is what the quota accounts for.
				match U64.from_str(command.rest) {
					Err(_) =>
						{
							state,
							replies: ["Usage: quota <units>, for example `quota 6`"],
						}

					Ok(units) =>
						{
							state,
							replies: [useQuota(units)],
						}
				}

			"retry" =>
				{
					state,
					replies: [retryReport({})],
				}

			"trap" =>
			# trap <reason>: fails the invocation with the reason, which is what
			# the ABI harness checks the trap path with and what the e2e observes
			# as the failed invocation's message. Nothing after it runs.
				{
					state,
					replies: [trapWith!(command.rest)],
				}

			"oplog-commit" =>
			# oplog-commit <replicas>: waits for the oplog to reach that many
			# replicas (or all of them) and then returns.
				match U8.from_str(command.rest) {
					Err(_) =>
						{
							state,
							replies: ["Usage: oplog-commit <replicas>, for example `oplog-commit 2`"],
						}

					Ok(replicas) => {
						oplogCommit!(replicas)
						{
							state,
							replies: ["Committed the oplog to ${U8.to_str(replicas)} replicas"],
						}
					}
				}

			"resolve-agent-id" => {
				# resolve-agent-id <component> <agent-name>, for example
				# `resolve-agent-id roc-golem:counter counter-agent("c-1")`.
				target = splitToken(command.rest)
				{
					state,
					replies: [describeResolveAgentId!(target.first, target.rest)],
				}
			}

			"resolve-component-id" =>
			# resolve-component-id <component>.
				{
					state,
					replies: [describeResolveComponentId!(command.rest)],
				}

			"kv" =>
			# kv <bucket> <key> <value>: a key-value round trip (set, read it
			# back, check it exists, delete, read the deleted key — the last
			# read is what shows a missing key answering the literal `null`).
				{
					state,
					replies: [kvRoundTrip!(command.rest)],
				}

			"blob" =>
			# blob <container> <object> <value>: a blobstore round trip (ensure
			# the container, put, read back, check, list, delete, check again).
				{
					state,
					replies: [blobRoundTrip!(command.rest)],
				}

			"fs" =>
			# fs <verb> <path> [text]: one effect per verb (write, append, read,
			# exists, size, mkdir, delete, list) against the agent's own
			# filesystem. A file one invocation wrote is there for the next one:
			# the writes are recorded in the oplog and replayed, so the
			# filesystem is durable with the agent.
				{
					state,
					replies: [fileCommand!(command.rest)],
				}

			_ =>
				{
					state,
					replies: ["Unknown command '${message}'. Available: increment, decrement, get, quota <units>, retry, trap <reason>, oplog-commit <replicas>, resolve-agent-id <component> <agent-name>, resolve-component-id <component>, kv <bucket> <key> <value>, blob <container> <object> <value>, fs <verb> <path> [text]"],
				}
		}
	},

	handleToolCall!: |state, toolCall|
		match toolCall.name {
			"get_count" =>
				{
					state,
					result: {
						id: toolCall.id,
						success: True,
						output: "{\"count\": ${I64.to_str(state.count)}}",
					},
				}

			_ =>
				{
					state,
					result: {
						id: toolCall.id,
						success: False,
						output: "Unknown tool ${toolCall.name}",
					},
				}
		},

	metadata: {
		name: "counter-agent",
		version: "1.0.0",
		description: "A durable Roc counter agent running on Golem Cloud",
		tools: [
			{
				name: "get_count",
				description: "Returns current counter value",
				parameters: [],
				readOnly: True,
			},
		],
		config: [
			{
				name: "greeting",
				secret: False,
			},
		],
		agentKind: Regular,
		httpMount: None,
		openapiProvider: NoOpenApi,
	},

	serializeState: |state|
		"{\"count\":${I64.to_str(state.count)}}",

	deserializeState: |stateJson|
		match Str.split_first(stateJson, "\"count\":") {
			Ok({ before: _, after }) =>
				match Str.split_first(after, "}") {
					Ok({ before, after: _ }) =>
						match I64.from_str(before) {
							Ok(c) => Ok({ count: c })
							Err(_) => Err("Failed to parse count integer")
						}

					Err(_) => Err("Invalid JSON format")
				}

			Err(_) => Err("Missing count field")
		},
})

## Splits the first space-separated token off `text`; `rest` keeps what follows
## (empty when there is no space left).
splitToken : Str -> { first : Str, rest : Str }
splitToken = |text|
	match Str.split_first(text, " ") {
		Ok({ before, after }) => { first: before, rest: after }

		Err(_) => { first: text, rest: "" }
	}

## Splits the first two space-separated tokens off `text` (`first`, `second`);
## `rest` is everything after them, spaces included, so a command's last
## argument can be text with spaces in it.
splitThree : Str -> { first : Str, second : Str, rest : Str }
splitThree = |text| {
	firstTwo = splitToken(text)
	secondOfThem = splitToken(firstTwo.rest)
	{ first: firstTwo.first, second: secondOfThem.first, rest: secondOfThem.rest }
}

## `true` / `false` as the text the round-trip reports render their checks with.
boolText : Bool -> Str
boolText = |value|
	if (value) "true" else "false"

## The key-value round trip `kv <bucket> <key> <value>` runs. Every step's
## outcome is in the one reply, so a check sees exactly how far it got; a step
## that fails is reported as `error(<message>)` in its own slot instead of
## hiding the rest.
kvRoundTrip! : Str => Str
kvRoundTrip! = |args| {
	{ first: bucket, second: key, rest: value } = splitThree(args)

	setOutcome = match kvSet!(bucket, key, value) {
		Ok(_) => "ok"
		Err(err) => "error(${err})"
	}
	readOutcome = match kvGet!(bucket, key) {
		Ok(text) => text
		Err(err) => "error(${err})"
	}
	existsOutcome = match kvExists!(bucket, key) {
		Ok(found) => boolText(found)
		Err(err) => "error(${err})"
	}
	deleteOutcome = match kvDelete!(bucket, key) {
		Ok(_) => "ok"
		Err(err) => "error(${err})"
	}
	afterOutcome = match kvExists!(bucket, key) {
		Ok(found) => boolText(found)
		Err(err) => "error(${err})"
	}
	missingOutcome = match kvGet!(bucket, key) {
		Ok(text) => text
		Err(err) => "error(${err})"
	}

	"kv set=${setOutcome} get=${readOutcome} exists=${existsOutcome} delete=${deleteOutcome} exists-after=${afterOutcome} missing=${missingOutcome}"
}

## The blobstore round trip `blob <container> <object> <value>` runs, reported
## the same way as `kvRoundTrip!`.
blobRoundTrip! : Str => Str
blobRoundTrip! = |args| {
	{ first: container, second: object, rest: value } = splitThree(args)

	createOutcome = match blobCreateContainer!(container) {
		Ok(_) => "ok"
		Err(err) => "error(${err})"
	}
	putOutcome = match blobPut!(container, object, value) {
		Ok(_) => "ok"
		Err(err) => "error(${err})"
	}
	getOutcome = match blobGet!(container, object) {
		Ok(text) => text
		Err(err) => "error(${err})"
	}
	hasOutcome = match blobHas!(container, object) {
		Ok(found) => boolText(found)
		Err(err) => "error(${err})"
	}
	listOutcome = match blobList!(container) {
		Ok(names) => names
		Err(err) => "error(${err})"
	}
	deleteOutcome = match blobDelete!(container, object) {
		Ok(_) => "ok"
		Err(err) => "error(${err})"
	}
	hasAfterOutcome = match blobHas!(container, object) {
		Ok(found) => boolText(found)
		Err(err) => "error(${err})"
	}

	"blob create=${createOutcome} put=${putOutcome} get=${getOutcome} has=${hasOutcome} list=${listOutcome} delete=${deleteOutcome} has-after=${hasAfterOutcome}"
}

## The filesystem command `fs <verb> <path> [text]` runs, one effect per verb,
## with the outcome in the reply: a read answers the file's text, and a failure
## is reported as `fs <verb> failed: <error>` instead of hiding the rest. The
## path rule is the platform's: a leading `/` names the agent's root, `.`/`..`
## normalize inside it, and a path that would step above it is refused.
fileCommand! : Str => Str
fileCommand! = |args| {
	{ first: verb, second: path, rest: text } = splitThree(args)

	match verb {
		"read" =>
			match readFile!(path) {
				Ok(contents) => "fs read: ${contents}"
				Err(err) => "fs read failed: ${err}"
			}

		"write" =>
			match writeFile!(path, text) {
				Ok(_) => "fs wrote ${path}"
				Err(err) => "fs write failed: ${err}"
			}

		"append" =>
			match appendFile!(path, text) {
				Ok(_) => "fs appended ${path}"
				Err(err) => "fs append failed: ${err}"
			}

		"exists" =>
			match fileExists!(path) {
				Ok(found) => "fs exists: ${boolText(found)}"
				Err(err) => "fs exists failed: ${err}"
			}

		"size" =>
			match fileSize!(path) {
				Ok(size) => "fs size: ${U64.to_str(size)}"
				Err(err) => "fs size failed: ${err}"
			}

		"mkdir" =>
			match makeDir!(path) {
				Ok(_) => "fs made dir ${path}"
				Err(err) => "fs mkdir failed: ${err}"
			}

		"delete" =>
			match deleteFile!(path) {
				Ok(_) => "fs deleted ${path}"
				Err(err) => "fs delete failed: ${err}"
			}

		"list" =>
			match listDir!(path) {
				Ok(entries) => "fs list: ${entries}"
				Err(err) => "fs list failed: ${err}"
			}

		_ => "Usage: fs write|append|read|exists|size|mkdir|delete|list <path> [text]"
	}
}

## Fails the invocation with `reason`; the host traps, so nothing after this
## returns. It sits in a helper so the handler's arm still has a reply to name:
## the value never comes back.
trapWith! : Str => Str
trapWith! = |reason| {
	trap!(reason)
	"trap! returned: this is unreachable"
}

## The agent id `resolve-agent-id <component> <agent-name>` reports:
## `{"componentId": …, "agentId": …}` from the host, or why the reference
## resolved to nothing.
describeResolveAgentId! : Str, Str => Str
describeResolveAgentId! = |componentReference, agentName|
	match resolveAgentId!(componentReference, agentName) {
		Ok(agentIdJson) => "Resolved agent: ${agentIdJson}"
		Err(err) => "Resolving an agent id failed: ${err}"
	}

## The component id `resolve-component-id <component>` reports, or why the
## reference resolved to nothing.
describeResolveComponentId! : Str => Str
describeResolveComponentId! = |componentReference|
	match resolveComponentId!(componentReference) {
		Ok(componentId) => "Resolved component: ${componentId}"
		Err(err) => "Resolving a component failed: ${err}"
	}

## Reserves and commits `units` of the `example-requests` resource the manifest
## declares, the way a method accounts for work it protected with a reservation:
## the token is sized for the reservation, and the commit reports what the work
## actually used — a smaller amount returns the difference, a larger one is
## charged to the token as debt, and a reservation that is never committed counts
## as zero. The manifest gives this resource a small capacity and
## `enforcementAction: reject`, so a reservation that no longer fits reports
## `quota reservation failed` instead of waiting for capacity.
useQuota : U64 => Str
useQuota = |units| {
	token = newQuotaToken!("example-requests", units)

	match quotaReserve!(token, units) {
		Err(err) => "Quota rejected: ${err}"

		Ok(reservation) =>
			match quotaCommit!(reservation, units) {
				Ok(_) => "Committed ${U64.to_str(units)} units"
				Err(err) => "Committing the reservation failed: ${err}"
			}
	}
}

## Reports the retry policies the environment declared, resolves the rule that
## applies to a `send` operation, and walks the resolved policy's delay schedule
## twice. The continuation `nextRetryDelay!` hands back is opaque, so the second
## call passes the first decision's `state` back unchanged — and picking that
## array out of the decision (it is the decision's last field) is the only JSON
## this has to take apart. A verb no rule matches resolves to the environment's
## default rule (every environment has a `predTrue` one), so there is no empty
## resolution to report here.
retryReport : {} => Str
retryReport = |_| {
	policies = getRetryPolicies!({})
	context = "{\"verb\":\"send\",\"nounUri\":\"quota://e2e\",\"properties\":{\"attempt\":1}}"

	match resolveRetryPolicy!("send", "quota://e2e", "{\"attempt\":1}") {
		Err(err) => "Resolving a retry policy failed: ${err}"

		Ok("") => "policies=${policies} match=(none)"

		Ok(policyJson) =>
			match nextRetryDelay!(policyJson, "[]", 0, context) {
				Err(err) => "Computing the first retry delay failed: ${err}"

				Ok(first) =>
					match nextRetryDelay!(policyJson, stateJsonOf(first), 150000000, context) {
						Err(err) => "Computing the second retry delay failed: ${err}"

						Ok(second) => "policies=${policies} match=${policyJson} first=${first} second=${second}"
					}
			}
	}
}

## The `state` array of a `{state, delayNanos}` decision: the decision renders as
## `{"delayNanos":…,"state":[…]}` — the array is the last field, so what sits
## between its brackets is the continuation to pass back.
stateJsonOf : Str -> Str
stateJsonOf = |decision|
	match Str.split_first(decision, "\"state\":[") {
		Ok({ before: _, after }) =>
			match Str.split_first(after, "]") {
				Ok({ before: elements, after: _ }) => "[${elements}]"
				Err(_) => "[]"
			}

		Err(_) => "[]"
	}
