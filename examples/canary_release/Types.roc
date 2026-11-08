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

import pf.Json as GolemJson exposing [encode, decode]

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
	encode(inp)

decodeStartInput : Str -> Result(StartInput, Str)
decodeStartInput = |json|
	decode(json)

encodeStartOutput : StartOutput -> Str
encodeStartOutput = |out|
	encode(out)

decodeStartOutput : Str -> Result(StartOutput, Str)
decodeStartOutput = |json|
	decode(json)

encodeDecisionInput : DecisionInput -> Str
encodeDecisionInput = |inp|
	encode(inp)

decodeDecisionInput : Str -> Result(DecisionInput, Str)
decodeDecisionInput = |json|
	decode(json)

encodeRollbackInput : RollbackInput -> Str
encodeRollbackInput = |inp|
	encode(inp)

decodeRollbackInput : Str -> Result(RollbackInput, Str)
decodeRollbackInput = |json|
	decode(json)

encodeStatus : Status -> Str
encodeStatus = |st|
	encode(st)

decodeStatus : Str -> Result(Status, Str)
decodeStatus = |json|
	decode(json)

encodeLogBatch : LogBatch -> Str
encodeLogBatch = |batch|
	encode(batch)

decodeLogBatch : Str -> Result(LogBatch, Str)
decodeLogBatch = |json|
	decode(json)

encodeDiagnosticReport : DiagnosticReport -> Str
encodeDiagnosticReport = |rep|
	encode(rep)

decodeDiagnosticReport : Str -> Result(DiagnosticReport, Str)
decodeDiagnosticReport = |json|
	decode(json)
