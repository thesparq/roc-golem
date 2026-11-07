module [
	agent,
	runDiagnostics,
	handleRunDiagnostics,
]

import pf.Golem exposing [
	Agent,
	Method,
	defineEphemeral,
	registerEphemeralTyped,
	method,
	build,
]
import Types exposing [
	Result,
	LogBatch,
	DiagnosticReport,
	encodeLogBatch,
	decodeLogBatch,
	encodeDiagnosticReport,
	decodeDiagnosticReport,
]

runDiagnostics : Method(LogBatch, DiagnosticReport)
runDiagnostics = method({
	name: "run_diagnostics",
	description: "Scans a log batch for panics and OOMs without holding state.",
	promptHint: "Call this to inspect container logs before promoting a canary.",
})

agent : Agent({})
agent =
	defineEphemeral({
		name: "diagnostics-worker",
		version: "1.0.0",
		description: "Stateless diagnostics worker for parallel log analysis.",
	})
		|> registerEphemeralTyped(
			runDiagnostics,
			handleRunDiagnostics,
			decodeLogBatch,
			encodeDiagnosticReport,
		)
		|> build

handleRunDiagnostics : LogBatch -> Result(DiagnosticReport, Str)
handleRunDiagnostics = |batch| {
	panicCount = countOccurrences(batch.lines, "PANIC")
	oomCount = countOccurrences(batch.lines, "OOM")
	faults = panicCount + oomCount
	healthy = faults == 0.U64
	summary =
		if healthy {
			"Clean logs: 0 faults detected across inspected lines for ${batch.service}"
		} else {
			"ALERT: ${Str.inspect(faults)} fault(s) detected in ${batch.service} (PANIC=${Str.inspect(panicCount)}, OOM=${Str.inspect(oomCount)})"
		}

	Ok({
		service: batch.service,
		faults: faults,
		healthy: healthy,
		summary: summary,
	})
}

countOccurrences : Str, Str -> U64
countOccurrences = |text, needle|
	match Str.split_first(text, needle) {
		Ok({ after, .. }) => 1.U64 + countOccurrences(after, needle)
		Err(_) => 0.U64
	}
