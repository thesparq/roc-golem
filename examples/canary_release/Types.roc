module [
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
	"{\"version\":\"${inp.version}\",\"weight\":${Num.to_str(inp.weight)},\"logs\":\"${escapeJson(inp.logs)}\",\"permissionCard\":\"${escapeJson(inp.permissionCard)}\"}"

decodeStartInput : Str -> Result(StartInput, Str)
decodeStartInput = |json|
	Ok({
		version: extractStr(json, "version") ?? "v1.1.0",
		weight: extractU64(json, "weight") ?? 10,
		logs: extractStr(json, "logs") ?? "",
		permissionCard: extractStr(json, "permissionCard") ?? "",
	})

encodeStartOutput : StartOutput -> Str
encodeStartOutput = |out|
	"{\"status\":\"${out.status}\",\"version\":\"${out.version}\",\"weight\":${Num.to_str(out.weight)},\"approvalWebhookUrl\":\"${out.approvalWebhookUrl}\",\"faults\":${Num.to_str(out.faults)}}"

decodeStartOutput : Str -> Result(StartOutput, Str)
decodeStartOutput = |json|
	Ok({
		status: extractStr(json, "status") ?? "",
		version: extractStr(json, "version") ?? "",
		weight: extractU64(json, "weight") ?? 0,
		approvalWebhookUrl: extractStr(json, "approvalWebhookUrl") ?? "",
		faults: extractU64(json, "faults") ?? 0,
	})

encodeDecisionInput : DecisionInput -> Str
encodeDecisionInput = |inp| {
	apprStr = if inp.approved "true" else "false"
	"{\"approved\":${apprStr},\"reviewer\":\"${escapeJson(inp.reviewer)}\"}"
}

decodeDecisionInput : Str -> Result(DecisionInput, Str)
decodeDecisionInput = |json| {
	approved =
		match Str.split_first(json, "\"approved\":") {
			Ok({ after }) =>
				if Str.starts_with(Str.trim(after), "true") Bool.true else Bool.false
			Err(_) => Bool.false
		}
	reviewer = extractStr(json, "reviewer") ?? "unknown"
	Ok({ approved, reviewer })
}

encodeRollbackInput : RollbackInput -> Str
encodeRollbackInput = |inp|
	"{\"reason\":\"${escapeJson(inp.reason)}\"}"

decodeRollbackInput : Str -> Result(RollbackInput, Str)
decodeRollbackInput = |json|
	Ok({
		reason: extractStr(json, "reason") ?? "manual rollback",
	})

encodeStatus : Status -> Str
encodeStatus = |st| {
	evts =
		st.events
			|> List.map(|e| "\"${escapeJson(e)}\"")
			|> Str.join_with(",")
	"{\"service\":\"${escapeJson(st.service)}\",\"activeVersion\":\"${escapeJson(st.activeVersion)}\",\"canaryVersion\":\"${escapeJson(st.canaryVersion)}\",\"weight\":${Num.to_str(st.weight)},\"region\":\"${escapeJson(st.region)}\",\"pendingTimerHandle\":${Num.to_str(st.pendingTimerHandle)},\"approvalWebhookUrl\":\"${escapeJson(st.approvalWebhookUrl)}\",\"events\":[${evts}]}"
}

decodeStatus : Str -> Result(Status, Str)
decodeStatus = |json|
	Ok({
		service: extractStr(json, "service") ?? "",
		activeVersion: extractStr(json, "activeVersion") ?? "",
		canaryVersion: extractStr(json, "canaryVersion") ?? "",
		weight: extractU64(json, "weight") ?? 0,
		region: extractStr(json, "region") ?? "",
		pendingTimerHandle: Num.to_u32(extractU64(json, "pendingTimerHandle") ?? 0),
		approvalWebhookUrl: extractStr(json, "approvalWebhookUrl") ?? "",
		events: extractStringList(json, "events"),
	})

encodeLogBatch : LogBatch -> Str
encodeLogBatch = |batch|
	"{\"service\":\"${escapeJson(batch.service)}\",\"lines\":\"${escapeJson(batch.lines)}\"}"

decodeLogBatch : Str -> Result(LogBatch, Str)
decodeLogBatch = |json|
	Ok({
		service: extractStr(json, "service") ?? "",
		lines: extractStr(json, "lines") ?? "",
	})

encodeDiagnosticReport : DiagnosticReport -> Str
encodeDiagnosticReport = |rep| {
	healthyStr = if rep.healthy "true" else "false"
	"{\"service\":\"${escapeJson(rep.service)}\",\"faults\":${Num.to_str(rep.faults)},\"healthy\":${healthyStr},\"summary\":\"${escapeJson(rep.summary)}\"}"
}

decodeDiagnosticReport : Str -> Result(DiagnosticReport, Str)
decodeDiagnosticReport = |json| {
	faults = extractU64(json, "faults") ?? 0
	healthy =
		match Str.split_first(json, "\"healthy\":") {
			Ok({ after }) =>
				if Str.starts_with(Str.trim(after), "true") Bool.true else Bool.false
			Err(_) => faults == 0
		}
	Ok({
		service: extractStr(json, "service") ?? "",
		faults,
		healthy,
		summary: extractStr(json, "summary") ?? "",
	})
}

# --- Helpers ---

escapeJson : Str -> Str
escapeJson = |s|
	s
		|> Str.replace_each("\\", "\\\\")
		|> Str.replace_each("\"", "\\\"")
		|> Str.replace_each("\n", "\\n")
		|> Str.replace_each("\r", "\\r")
		|> Str.replace_each("\t", "\\t")

extractStr : Str, Str -> Result(Str, [NotFound])
extractStr = |json, field_name| {
	target = "\"${field_name}\":\""
	match Str.split_first(json, target) {
		Ok({ after }) =>
			match Str.split_first(after, "\"") {
				Ok({ before }) => Ok(before),
				Err(_) => Err(NotFound),
			}
		Err(_) => Err(NotFound),
	}
}

extractU64 : Str, Str -> Result(U64, [NotFound])
extractU64 = |json, field_name| {
	target = "\"${field_name}\":"
	match Str.split_first(json, target) {
		Ok({ after }) => {
			trimmed = Str.trim(after)
			# Find end delimiter (, or } or whitespace)
			delimited =
				match Str.split_first(trimmed, ",") {
					Ok({ before }) => before,
					Err(_) =>
						match Str.split_first(trimmed, "}") {
							Ok({ before }) => before,
							Err(_) => trimmed,
						},
				}
			match Str.to_u64(Str.trim(delimited)) {
				Ok(val) => Ok(val),
				Err(_) => Err(NotFound),
			}
		}
		Err(_) => Err(NotFound),
	}
}

extractStringList : Str, Str -> List(Str)
extractStringList = |json, field_name| {
	target = "\"${field_name}\":["
	match Str.split_first(json, target) {
		Ok({ after }) =>
			match Str.split_first(after, "]") {
				Ok({ before }) =>
					if Str.is_empty(Str.trim(before)) {
						[]
					} else {
						Str.split_on(before, ",")
							|> List.map(|item| {
								trimmed = Str.trim(item)
								withoutPrefix = if Str.starts_with(trimmed, "\"") Str.drop_prefix(trimmed, "\"") else trimmed
								withoutSuffix = if Str.ends_with(withoutPrefix, "\"") Str.drop_suffix(withoutPrefix, "\"") else withoutPrefix
								withoutSuffix
							})
					}
				Err(_) => [],
			}
		Err(_) => [],
	}
}
