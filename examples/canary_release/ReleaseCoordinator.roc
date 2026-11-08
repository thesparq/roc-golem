module [
	agent,
	startCanary,
	approveGate,
	emergencyRollback,
	statusMethod,
]

import pf.Golem exposing [
	Agent,
	Method,
	define,
	registerTyped,
	registerReadOnlyTyped,
	withTools,
	withStatelessToolHandler,
	method,
	build,
]
import pf.Types as PfTypes exposing [ToolCall, ToolResult]
import pf.Json
import Types exposing [
	Result,
	State,
	StartInput,
	StartOutput,
	DecisionInput,
	RollbackInput,
	Status,
	LogBatch,
	DiagnosticReport,
	encodeStartInput,
	decodeStartInput,
	encodeStartOutput,
	decodeStartOutput,
	encodeDecisionInput,
	decodeDecisionInput,
	encodeRollbackInput,
	decodeRollbackInput,
	encodeStatus,
	decodeStatus,
	encodeLogBatch,
	decodeLogBatch,
	encodeDiagnosticReport,
	decodeDiagnosticReport,
]
import DiagnosticsWorker

dashboardPath = "/dashboard.html"

startCanary : Method(StartInput, StartOutput)
startCanary = method({
	name: "start_canary",
	description: "Starts a new canary deployment for a given service.",
	promptHint: "Call this to initialize a canary with initial traffic.",
})

approveGate : Method(DecisionInput, Str)
approveGate = method({
	name: "approve_gate",
	description: "Approves or rejects promotion of the canary traffic weight.",
	promptHint: "Call to advance canary to next rollout stage.",
})

emergencyRollback : Method(RollbackInput, Str)
emergencyRollback = method({
	name: "emergency_rollback",
	description: "Rolls the canary back to 0% of traffic.",
	promptHint: "Call this to take the canary out of rotation immediately.",
})

statusMethod : Method({}, Status)
statusMethod = method({
	name: "get_status",
	description: "Returns the current rollout status.",
	promptHint: "Query current canary status.",
})

agent : Agent(State)
agent =
	define(
		{
			name: "canary-release",
			version: "1.0.0",
			description: "Canary rollout orchestrator: typed methods, a gate, a live dashboard.",
		},
		initCoordinator,
		serializeState,
		deserializeState,
	)
		|> withTools([
			{
				name: "analyze_logs",
				description: "Scans log lines for panics and OOMs",
				parameters: [
					{
						name: "logs",
						description: "Raw log text to analyze",
						paramType: "string",
						required: True,
					},
				],
			},
		])
		|> withStatelessToolHandler(handleTool)
		|> registerTyped(
			startCanary,
			handleStart,
			decodeStartInput,
			encodeStartOutput,
		)
		|> registerTyped(
			approveGate,
			handleApprove,
			decodeDecisionInput,
			|s| s,
		)
		|> registerTyped(
			emergencyRollback,
			handleRollback,
			decodeRollbackInput,
			|s| s,
		)
		|> registerReadOnlyTyped(
			statusMethod,
			handleStatus,
			|_| Ok({}),
			encodeStatus,
		)
		|> build

Config : {
	service : Str,
}

initCoordinator : Str -> Result(State, Str)
initCoordinator = |configJson| {
	cfg : Config
	cfg = Json.decode_or(configJson, { service: "default-service" })
	Ok({
		service: cfg.service,
		activeVersion: "v1.0.0",
		canaryVersion: "",
		weight: 0.U64,
		region: "us-east-1",
		pendingTimerHandle: 0.U32,
		approvalWebhookUrl: "",
		events: ["coordinator_initialized"],
	})
}

handleStart : State, StartInput -> Result({ state : State, out : StartOutput }, Str)
handleStart = |state, input| {
	diagReport =
		DiagnosticsWorker.handleRunDiagnostics({
			service: state.service,
			lines: input.logs,
		})
			?? { service: state.service, faults: 0.U64, healthy: True, summary: "ok" }

	if !diagReport.healthy {
		abortEvent = "canary_aborted: ${diagReport.summary}"
		events = List.append(state.events, abortEvent)
		nextState = { ..state, events: events }
		Ok({
			state: nextState,
			out: {
				status: "aborted_unhealthy",
				version: input.version,
				weight: 0.U64,
				approvalWebhookUrl: "",
				faults: diagReport.faults,
			},
		})
	} else {
		webhookUrl = "https://golem.cloud/webhooks/canary/${state.service}/gate-approve"
		startEvent = "canary_started: ${input.version} at ${Str.inspect(input.weight)}%"
		events = List.append(state.events, startEvent)
		timerHandle = 42.U32

		nextState = {
			..state,
			canaryVersion: input.version,
			weight: input.weight,
			pendingTimerHandle: timerHandle,
			approvalWebhookUrl: webhookUrl,
			events: events,
		}

		Ok({
			state: nextState,
			out: {
				status: "canary_started",
				version: input.version,
				weight: input.weight,
				approvalWebhookUrl: webhookUrl,
				faults: 0.U64,
			},
		})
	}
}

handleApprove : State, DecisionInput -> Result({ state : State, out : Str }, Str)
handleApprove = |state, input| {
	if input.approved {
		promotedVer = if Str.is_empty(state.canaryVersion) state.activeVersion else state.canaryVersion
		events = List.append(state.events, "approved_by_${input.reviewer}: promoted ${promotedVer} to 100%")
		nextState = {
			..state,
			activeVersion: promotedVer,
			canaryVersion: "",
			weight: 100.U64,
			pendingTimerHandle: 0.U32,
			events: events,
		}
		Ok({
			state: nextState,
			out: "Promoted ${promotedVer} to 100% (approved by ${input.reviewer})",
		})
	} else {
		events = List.append(state.events, "rejected_by_${input.reviewer}: canary ${state.canaryVersion} reset to 0%")
		nextState = {
			..state,
			canaryVersion: "",
			weight: 0.U64,
			pendingTimerHandle: 0.U32,
			events: events,
		}
		Ok({
			state: nextState,
			out: "Canary rejected by ${input.reviewer}",
		})
	}
}

handleRollback : State, RollbackInput -> Result({ state : State, out : Str }, Str)
handleRollback = |state, input| {
	events = List.append(state.events, "emergency_rollback: ${input.reason}")
	nextState = {
		..state,
		weight: 0.U64,
		pendingTimerHandle: 0.U32,
		events: events,
	}
	Ok({
		state: nextState,
		out: "Rolled back to 0%: ${input.reason}",
	})
}

handleStatus : State, {} -> Result(Status, Str)
handleStatus = |state, _|
	Ok({
		service: state.service,
		activeVersion: state.activeVersion,
		canaryVersion: state.canaryVersion,
		weight: state.weight,
		region: state.region,
		pendingTimerHandle: state.pendingTimerHandle,
		approvalWebhookUrl: state.approvalWebhookUrl,
		events: state.events,
	})

handleTool : ToolCall -> Result(ToolResult, Str)
handleTool = |call| {
	if call.name == "analyze_logs" or call.name == "analyze-logs" {
		logs = call.arguments
		panicCount = countOccurrences(logs, "PANIC")
		oomCount = countOccurrences(logs, "OOM")
		faults = panicCount + oomCount
		Ok({
			id: call.id,
			success: True,
			output: Json.encode({ faults: faults }),
		})
	} else {
		Ok({
			id: call.id,
			success: False,
			output: Json.encode({ error: "Unknown tool ${call.name}" }),
		})
	}
}

countOccurrences : Str, Str -> U64
countOccurrences = |text, needle|
	match Str.split_first(text, needle) {
		Ok({ after, .. }) => 1.U64 + countOccurrences(after, needle)
		Err(_) => 0.U64
	}

renderDashboard : State -> Str
renderDashboard = |st| {
	eventItems =
		st.events
			|> List.map(|e| "<li>${e}</li>")
			|> Str.join_with("")
	"<!doctype html><html><body><h1>Release Guardian: ${st.service}</h1><p>Active: ${st.activeVersion} | Canary: ${st.canaryVersion} at ${Str.inspect(st.weight)}%</p><ul>${eventItems}</ul></body></html>"
}

serializeState : State -> Str
serializeState = |st|
	encodeStatus({
		service: st.service,
		activeVersion: st.activeVersion,
		canaryVersion: st.canaryVersion,
		weight: st.weight,
		region: st.region,
		pendingTimerHandle: st.pendingTimerHandle,
		approvalWebhookUrl: st.approvalWebhookUrl,
		events: st.events,
	})

deserializeState : Str -> Result(State, Str)
deserializeState = |json| {
	match decodeStatus(json) {
		Ok(status) =>
			Ok({
				service: status.service,
				activeVersion: status.activeVersion,
				canaryVersion: status.canaryVersion,
				weight: status.weight,
				region: status.region,
				pendingTimerHandle: status.pendingTimerHandle,
				approvalWebhookUrl: status.approvalWebhookUrl,
				events: status.events,
			})
		Err(err) => Err(err)
	}
}
