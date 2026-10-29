app [agent] { pf: platform "../../platform/main.roc" }

# Release Guardian — a canary-rollout orchestrator written against the typed
# authoring surface in `platform/Agent.roc`. Each method is declared once with
# `method`, bound to a typed handler with `register` / `registerReadOnly`, and
# the platform derives the metadata, the JSON dispatch and the state codecs.
#
# The blueprint's `ReleaseCoordinator` also used permission cards as values,
# secret handles, tool middleware and method streams. This platform does not
# expose those yet (see `docs/golem-1.6-parity.md`), so this app uses what it
# does expose: typed methods, config (one secret-sourced key), a scheduling
# effect, a webhook approval gate, the agent's durable filesystem, and a tool.

import pf.Golem exposing [
	Agent,
	build,
	createWebhook!,
	define,
	getConfig!,
	logInfo!,
	method,
	register,
	registerReadOnly,
	rpcSchedule!,
	withConfig,
	workerId!,
	writeFile!,
]

## The agent's state: the rollout's own record. `events` is the durable log the
## read-only status method reports, so the app's history lives in the state the
## platform snapshots rather than anywhere else.
State : {
	service : Str,
	canary : Str,
	weight : U64,
	events : List(Str),
	approvalUrl : Str,
	region : Str,
}

## Config the agent reads with `getConfig!`. The platform has no secret-handle
## surface yet (`docs/golem-1.6-parity.md`), and a `secret: True` declaration
## would require the manifest to supply the key as `secret<T>`, so this app
## declares only the plain key it reads.
configDeclarations = [
	{ name: "region", secret: False },
]

## The tool this agent publishes (`golem:tool/guest`). `readOnly: True` maps to
## the 1.6 read-only flag, so the runtime may cache its answer.
dashboardPath = "/dashboard.html"

tools = [
	{
		name: "analyze_logs",
		description: "Scans log lines for panics, OOMs and latency spikes.",
		parameters: [],
		readOnly: True,
	},
]

## Declared methods. The name is what the CLI/RPC invokes; the description and
## prompt hint are what the tooling shows a caller.
startCanary = method({
	name: "start_canary",
	description: "Starts a canary rollout and returns the gate's approval URL.",
	promptHint: "Call this to begin rolling a version out to a percentage of traffic.",
})

approveGate = method({
	name: "approve_gate",
	description: "Records a reviewer's decision for a pending gate.",
	promptHint: "Call this once a human has approved or rejected the canary.",
})

emergencyRollback = method({
	name: "emergency_rollback",
	description: "Rolls the canary back to 0% of traffic.",
	promptHint: "Call this to take the canary out of rotation immediately.",
})

statusMethod = method({
	name: "get_status",
	description: "Reports the current rollout state, without writing anything.",
	promptHint: "",
})

## The typed inputs and outputs. A handler's parameter and result are ordinary
## values: the platform decodes the caller's JSON into these and encodes the
## answer back, so an app never touches a JSON string itself.
StartInput : { version : Str, weight : U64 }

StartOutput : { version : Str, weight : U64, approvalUrl : Str }

DecisionInput : { approved : Bool, reviewer : Str }

RollbackInput : { reason : Str }

Status : { service : Str, canary : Str, weight : U64, events : List(Str), region : Str }

## A blank line for `Err`-happy effects whose answer we do not need; the effect
## has already been recorded, so dropping the text costs nothing.
ignoreEffect : Try({}, Str) -> {}
ignoreEffect = |res|
	match res {
		Ok(_) => {}
		Err(_) => {}
	}

ignoreText : Try(Str, Str) -> {}
ignoreText = |res|
	match res {
		Ok(_) => {}
		Err(_) => {}
	}

## The live dashboard the agent writes into its own durable filesystem. Every
## write is recorded in the invocation's oplog and replayed, so the file
## survives restarts and snapshots.
renderDashboard : Str, Str, U64, List(Str) -> Str
renderDashboard = |service, canary, weight, events| {
	eventItems = List.map(events, |e| "<li>${e}</li>")
	"<!doctype html><html><body><h1>Release Guardian: ${service}</h1><p>Canary ${canary} at ${U64.to_str(weight)}%</p><ul>${Str.join_with(eventItems, "")}</ul></body></html>"
}

writeDashboard : Str, Str, U64, List(Str) => {}
writeDashboard = |service, canary, weight, events|
	ignoreEffect(writeFile!(dashboardPath, renderDashboard(service, canary, weight, events)))

## The tool handler: the blueprint's `LogAnalyzerTool`, reduced to what the
## string-in / string-out tool surface carries. `call.arguments` is the caller's
## text; `stdout` is written to the stream the caller attached. The parameter and
## result are left to inference from `withToolHandler`.
handleTool = |state, call|
	if call.name == "analyze_logs" {
		faults = countLogFaults(call.arguments)
		{
			state,
			result: {
				id: call.id,
				success: True,
				output: "{\"faults\": ${U64.to_str(faults)}}",
				stdout: "analyze-logs: ${U64.to_str(faults)} fault(s)\n",
				stderr: "",
			},
		}
	} else {
		{
			state,
			result: {
				id: call.id,
				success: False,
				output: "{\"error\": \"Unknown tool ${call.name}\"}",
				stdout: "",
				stderr: "",
			},
		}
	}

## Counts the lines mentioning a panic or an OOM, the way the blueprint's
## analyzer scans raw container logs. Roc has no `Str.split` in this nightly, so
## the count walks the text one occurrence at a time.
countLogFaults : Str -> U64
countLogFaults = |text|
	countOccurrences(text, "PANIC") + countOccurrences(text, "OOM")

countOccurrences : Str, Str -> U64
countOccurrences = |text, needle|
	match Str.split_first(text, needle) {
		Err(_) => 0
		Ok(split) => 1 + countOccurrences(split.after, needle)
	}

## The typed handlers. Each is a `state, input => ...` function annotated on its
## binding; `register` / `registerReadOnly` read the input and output types from
## it and derive the JSON codecs, so the handlers deal in values, never text.
handleStart! : State, StartInput => Try({ state : State, out : StartOutput }, Str)
handleStart! = |state, input| {
	region = match getConfig!("region") {
		Ok(value) => value
		Err(_) => "unknown"
	}
	approvalUrl = match createWebhook!({}) {
		Ok(url) => url
		Err(_) => ""
	}
	logInfo!("canary ${input.version} at ${U64.to_str(input.weight)}% in ${region}")
	events = List.append(state.events, "canary ${input.version} at ${U64.to_str(input.weight)}%")
	writeDashboard(state.service, input.version, input.weight, events)
	# The safety timer: a scheduled self-call that rolls back if the gate is never
	# answered. The invocation identity it answers with is a follow-up the app
	# would keep to cancel the timer on approval.
	ignoreText(
		rpcSchedule!(
			workerId!({}),
			"emergency_rollback",
			"{\"reason\":\"approval gate timed out\"}",
			300000,
		),
	)
	Ok({
		state: {
			..state,
			canary: input.version,
			weight: input.weight,
			events: events,
			approvalUrl: approvalUrl,
			region: region,
		},
		out: { version: input.version, weight: input.weight, approvalUrl: approvalUrl },
	})
}

handleApprove! : State, DecisionInput => Try({ state : State, out : Str }, Str)
handleApprove! = |state, input| {
	if input.approved {
		events = List.append(state.events, "approved by ${input.reviewer}")
		writeDashboard(state.service, state.canary, 100, events)
		Ok({
			state: { ..state, weight: 100, events: events },
			out: "Promoted ${state.canary} to 100% (approved by ${input.reviewer})",
		})
	} else {
		events = List.append(state.events, "rejected by ${input.reviewer}")
		writeDashboard(state.service, state.canary, 0, events)
		Ok({
			state: { ..state, weight: 0, events: events },
			out: "Canary ${state.canary} rejected by ${input.reviewer}",
		})
	}
}

handleRollback! : State, RollbackInput => Try({ state : State, out : Str }, Str)
handleRollback! = |state, input| {
	logInfo!("emergency rollback: ${input.reason}")
	events = List.append(state.events, "rolled back: ${input.reason}")
	writeDashboard(state.service, state.canary, 0, events)
	Ok({
		state: { ..state, weight: 0, events: events },
		out: "Rolled ${state.canary} back to 0%: ${input.reason}",
	})
}

## Read-only: a pure `->` handler. It may not write the state or perform
## effects, which is exactly what the 1.6 read-only flag declares.
handleStatus : State, {} -> Status
handleStatus = |state, _input| {
	service: state.service,
	canary: state.canary,
	weight: state.weight,
	events: state.events,
	region: state.region,
}

agent : Agent(State)
agent =
	define(
		{
			name: "canary-release",
			version: "1.0.0",
			description: "Canary rollout orchestrator: typed methods, a gate, a live dashboard.",
		},
		|service|
			{
				service: service,
				canary: "",
				weight: 0,
				events: [],
				approvalUrl: "",
				region: "unknown",
			},
	)
		|> withConfig(configDeclarations)
		|> register(startCanary, handleStart!)
		|> register(approveGate, handleApprove!)
		|> register(emergencyRollback, handleRollback!)
		|> registerReadOnly(statusMethod, handleStatus)
		|> build
