app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [
	Agent,
	defineAgent,
	getConfig!,
	getRetryPolicies!,
	newQuotaToken!,
	nextRetryDelay!,
	quotaCommit!,
	quotaReserve!,
	resolveRetryPolicy!,
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

			_ =>
				{
					state,
					replies: ["Unknown command '${message}'. Available: increment, decrement, get, quota <units>, retry"],
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
