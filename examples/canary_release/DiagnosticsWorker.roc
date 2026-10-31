module [
	agent,
	runDiagnostics,
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
	healthy = faults == 0
	summary =
		if healthy {
			"Clean logs: 0 faults detected across inspected lines for ${batch.service}"
		} else {
			"ALERT: ${Num.to_str(faults)} fault(s) detected in ${batch.service} (PANIC=${Num.to_str(panicCount)}, OOM=${Num.to_str(oomCount)})"
		}

	Ok({
		service: batch.service,
		faults,
		healthy,
		summary,
	})
}

countOccurrences : Str, Str -> U64
countOccurrences = |text, needle|
	match Str.split_first(text, needle) {
		Ok({ after }) => 1 + countOccurrences(after, needle)
		Err(_) => 0
	}
