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
	"{\"version\":\"${inp.version}\",\"weight\":${Str.inspect(inp.weight)},\"logs\":\"${escapeJson(inp.logs)}\",\"permissionCard\":\"${escapeJson(inp.permissionCard)}\"}"

decodeStartInput : Str -> Result(StartInput, Str)
decodeStartInput = |json|
	Ok({
		version: extractStr(json, "version") ?? "v1.1.0",
		weight: extractU64(json, "weight") ?? 10.U64,
		logs: extractStr(json, "logs") ?? "",
		permissionCard: extractStr(json, "permissionCard") ?? "",
	})

encodeStartOutput : StartOutput -> Str
encodeStartOutput = |out|
	"{\"status\":\"${out.status}\",\"version\":\"${out.version}\",\"weight\":${Str.inspect(out.weight)},\"approvalWebhookUrl\":\"${out.approvalWebhookUrl}\",\"faults\":${Str.inspect(out.faults)}}"

decodeStartOutput : Str -> Result(StartOutput, Str)
decodeStartOutput = |json|
	Ok({
		status: extractStr(json, "status") ?? "",
		version: extractStr(json, "version") ?? "",
		weight: extractU64(json, "weight") ?? 0.U64,
		approvalWebhookUrl: extractStr(json, "approvalWebhookUrl") ?? "",
		faults: extractU64(json, "faults") ?? 0.U64,
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
			Ok({ after, .. }) =>
				if Str.starts_with(Str.trim(after), "true") True else False
			Err(_) => False
		}
	reviewer = extractStr(json, "reviewer") ?? "unknown"
	Ok({ approved: approved, reviewer: reviewer })
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
	"{\"service\":\"${escapeJson(st.service)}\",\"activeVersion\":\"${escapeJson(st.activeVersion)}\",\"canaryVersion\":\"${escapeJson(st.canaryVersion)}\",\"weight\":${Str.inspect(st.weight)},\"region\":\"${escapeJson(st.region)}\",\"pendingTimerHandle\":${Str.inspect(st.pendingTimerHandle)},\"approvalWebhookUrl\":\"${escapeJson(st.approvalWebhookUrl)}\",\"events\":[${evts}]}"
}

decodeStatus : Str -> Result(Status, Str)
decodeStatus = |json|
	Ok({
		service: extractStr(json, "service") ?? "",
		activeVersion: extractStr(json, "activeVersion") ?? "",
		canaryVersion: extractStr(json, "canaryVersion") ?? "",
		weight: extractU64(json, "weight") ?? 0.U64,
		region: extractStr(json, "region") ?? "",
		pendingTimerHandle: U64.to_u32_wrap(extractU64(json, "pendingTimerHandle") ?? 0.U64),
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
	"{\"service\":\"${escapeJson(rep.service)}\",\"faults\":${Str.inspect(rep.faults)},\"healthy\":${healthyStr},\"summary\":\"${escapeJson(rep.summary)}\"}"
}

decodeDiagnosticReport : Str -> Result(DiagnosticReport, Str)
decodeDiagnosticReport = |json| {
	faults = extractU64(json, "faults") ?? 0.U64
	healthy =
		match Str.split_first(json, "\"healthy\":") {
			Ok({ after, .. }) =>
				if Str.starts_with(Str.trim(after), "true") True else False
			Err(_) => faults == 0.U64
		}
	Ok({
		service: extractStr(json, "service") ?? "",
		faults: faults,
		healthy: healthy,
		summary: extractStr(json, "summary") ?? "",
	})
}

# --- Helpers ---

escapeJson : Str -> Str
escapeJson = |s|
	s
		|> Str.replace_each("\\", "\\\\")
		|> Str.replace_each("\"", "\\\"")

extractStr : Str, Str -> Result(Str, [NotFound])
extractStr = |json, field_name| {
	target = "\"${field_name}\":\""
	match Str.split_first(json, target) {
		Ok({ after, .. }) =>
			match Str.split_first(after, "\"") {
				Ok({ before, .. }) => Ok(before),
				Err(_) => Err(NotFound),
			}
		Err(_) => Err(NotFound),
	}
}

extractU64 : Str, Str -> Result(U64, [NotFound])
extractU64 = |json, field_name| {
	target = "\"${field_name}\":"
	match Str.split_first(json, target) {
		Ok({ after, .. }) => {
			trimmed = Str.trim(after)
			delimited =
				match Str.split_first(trimmed, ",") {
					Ok({ before, .. }) => before,
					Err(_) =>
						match Str.split_first(trimmed, "}") {
							Ok({ before, .. }) => before,
							Err(_) => trimmed,
						},
				}
			match U64.from_str(Str.trim(delimited)) {
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
		Ok({ after, .. }) =>
			match Str.split_first(after, "]") {
				Ok({ before, .. }) =>
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
