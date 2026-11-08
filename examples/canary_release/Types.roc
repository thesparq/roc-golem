module [
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

import pf.Json

Result(ok, err) : Try(ok, err)

## Shared wire and state types for the Release Guardian Canary Rollout Suite

State : {
	service : Str,
	activeVersion : Str,
	canaryVersion : Str,
	weight : U64,
	region : Str,
	pendingTimerHandle : U32,
	approvalWebhookUrl : Str,
	events : List(Str),
}

StartInput : {
	version : Str,
	weight : U64,
	logs : Str,
	permissionCard : Str,
}

StartOutput : {
	status : Str,
	version : Str,
	weight : U64,
	approvalWebhookUrl : Str,
	faults : U64,
}

DecisionInput : {
	approved : Bool,
	reviewer : Str,
}

RollbackInput : {
	reason : Str,
}

Status : {
	service : Str,
	activeVersion : Str,
	canaryVersion : Str,
	weight : U64,
	region : Str,
	pendingTimerHandle : U32,
	approvalWebhookUrl : Str,
	events : List(Str),
}

LogBatch : {
	service : Str,
	lines : Str,
}

DiagnosticReport : {
	service : Str,
	faults : U64,
	healthy : Bool,
	summary : Str,
}

# --- Codecs ---

encodeStartInput : StartInput -> Str
encodeStartInput = |inp|
	Json.encode(inp)

decodeStartInput : Str -> Result(StartInput, Str)
decodeStartInput = |json|
	Json.decode(json)

encodeStartOutput : StartOutput -> Str
encodeStartOutput = |out|
	Json.encode(out)

decodeStartOutput : Str -> Result(StartOutput, Str)
decodeStartOutput = |json|
	Json.decode(json)

encodeDecisionInput : DecisionInput -> Str
encodeDecisionInput = |inp|
	Json.encode(inp)

decodeDecisionInput : Str -> Result(DecisionInput, Str)
decodeDecisionInput = |json|
	Json.decode(json)

encodeRollbackInput : RollbackInput -> Str
encodeRollbackInput = |inp|
	Json.encode(inp)

decodeRollbackInput : Str -> Result(RollbackInput, Str)
decodeRollbackInput = |json|
	Json.decode(json)

encodeStatus : Status -> Str
encodeStatus = |st|
	Json.encode(st)

decodeStatus : Str -> Result(Status, Str)
decodeStatus = |json|
	Json.decode(json)

encodeLogBatch : LogBatch -> Str
encodeLogBatch = |batch|
	Json.encode(batch)

decodeLogBatch : Str -> Result(LogBatch, Str)
decodeLogBatch = |json|
	Json.decode(json)

encodeDiagnosticReport : DiagnosticReport -> Str
encodeDiagnosticReport = |rep|
	Json.encode(rep)

decodeDiagnosticReport : Str -> Result(DiagnosticReport, Str)
decodeDiagnosticReport = |json|
	Json.decode(json)
