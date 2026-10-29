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
	dbBegin!,
	dbCommit!,
	dbExecute!,
	dbOpen!,
	dbOpenIgnite!,
	dbOpenMysql!,
	dbQuery!,
	dbRollback!,
	defineAgent,
	deleteFile!,
	durableStreamAppend!,
	durableStreamRead!,
	durableStreamReader!,
	durableStreamRelease!,
	durableStreamWriter!,
	fileExists!,
	fileSize!,
	getConfig!,
	getPrincipal!,
	getRetryPolicies!,
	idempotencyKey!,
	kvDelete!,
	kvExists!,
	kvGet!,
	kvSet!,
	listDir!,
	makeDir!,
	newQuotaToken!,
	nextRetryDelay!,
	oplogCommit!,
	permissionDerive!,
	permissionDeriveFromWallet!,
	permissionDeriveScope!,
	permissionExpiresAt!,
	permissionId!,
	permissionInspect!,
	permissionInstall!,
	permissionListModules!,
	permissionParents!,
	permissionRelease!,
	permissionRevoke!,
	permissionValidateGrant!,
	permissionsWallet!,
	permissionsWalletVersion!,
	quotaCommit!,
	quotaReserve!,
	readFile!,
	resolveAgentId!,
	resolveComponentId!,
	resolveRetryPolicy!,
	trap!,
	workerId!,
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

			"db" =>
			# db <verb> [args]: the database effects (`golem:rdbms`). `open
			# <address>` opens a postgres connection and answers its handle;
			# `open-mysql` and `open-ignite` do the same for the other two
			# engines. `roundtrip <table> <address>` opens its own connection and
			# runs a create, an insert, a select and a count, then a rolled-back
			# and a committed transaction and a drop, reporting every step's
			# outcome in one reply.
				{
					state,
					replies: [dbCommand!(command.rest)],
				}

			"permission" =>
			# permission <verb> [args]: the permission-card surface
			# (`golem:permissions`) and the principal. Cards are host-side
			# capabilities the app carries as `U32` handles: `wallet` hands out one
			# handle per installed card, `inspect` / `id` / `parents` / `expires`
			# borrow a handle, `revoke` and `install` consume it, and `release`
			# gives it up. The verbs and their arguments are in the usage reply.
				{
					state,
					replies: [permissionCommand!(command.rest)],
				}

			"stream" =>
			# stream <verb> [args]: external durable streams
			# (`golem:agent/durable-streams`), against a service that speaks the
			# Durable Streams protocol. `roundtrip <url> <first> <second>` appends
			# two JSON messages through a writer, then reads them back from the
			# beginning and again from the checkpoint the read returned;
			# `append <url> <text>` is one append, `read <url>` one read from the
			# beginning, and `bytes <url> <text>` a byte-stream round trip that
			# closes the stream. A URL that does not exist is the service's own
			# `not-found`, which the commands report. The verbs are in the usage
			# reply, and the README's "Durable streams" section has the shapes.
				{
					state,
					replies: [streamCommand!(command.rest)],
				}

			_ =>
				{
					state,
					replies: ["Unknown command '${message}'. Available: increment, decrement, get, quota <units>, retry, trap <reason>, oplog-commit <replicas>, resolve-agent-id <component> <agent-name>, resolve-component-id <component>, kv <bucket> <key> <value>, blob <container> <object> <value>, fs <verb> <path> [text], db <verb> [args], permission <verb> [args], stream <verb> [args]"],
				}
		}
	},

	handleRequest!: |state, _request|
		{
			state,
			response: { status: 404, headers: [], body: "this agent is not an HTTP router" },
		},

	# The multimodal method's handler. This app declares
	# `multimodal: HandlesMultimodal`, so it is registered and reachable; an app
	# with nothing to do with multimodal content writes `NoMultimodal` and a stub
	# like this one (see the README's "Multimodal values"). The hook is a method
	# like any other: it runs against the agent's own state and answers the next
	# one.
	handleMultimodal!: |state, parts|
		{
			state,
			parts: multimodalAnswer(state.count, parts),
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
						stdout: "",
						stderr: "",
					},
				}

			_ =>
				{
					state,
					result: {
						id: toolCall.id,
						success: False,
						output: "Unknown tool ${toolCall.name}",
						stdout: "",
						stderr: "",
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
		methods: [],
		config: [
			{
				name: "greeting",
				secret: False,
			},
		],
		agentKind: Regular,
		httpMount: None,
		httpHandler: NoHandler,
		multimodal: HandlesMultimodal,
		openapiProvider: NoOpenApi,
	},

	serializeState: |state|
		Json.to_str({ count: state.count }),

	deserializeState: |stateJson| {
		parsed : Try({ count : I64 }, [InvalidJson(Str), MissingRequiredField(Str)])
		parsed = Json.parse(stateJson)
		match parsed {
			Ok(fields) => Ok({ count: fields.count })
			Err(_) => Err("Missing count field")
		}
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

## The multimodal method's answer. `parts` is the JSON envelope Golem built from
## the caller's value tree — one `{"tag":"text"|"binary","value":{…}}` object
## per part — and the hook's answer is the envelope the caller's parts become.
##
## This app reads the parts with its own simple string helpers (a value is found
## by its `"field":` prefix and read up to the next quote), reports the count its
## state carried and what it found, and hands the caller's own parts back, so one
## call
## shows both directions carrying text and binary against the agent's state.
## `MultimodalEnvelope` in `pf.Types` is the shape the text carries.
multimodalAnswer : I64, Str -> Str
multimodalAnswer = |count, parts| {
	report = "count=${I64.to_str(count)}; text=${firstField(parts, "\"text\":\"")}; bytes=${firstBytes(parts)}; mime=${firstField(parts, "\"mimeType\":\"")}"
	reportPart = "{\"tag\":\"text\",\"value\":{\"text\":${jsonString(report)},\"language\":null}}"
	received = envelopeParts(parts)

	if (Str.is_empty(received))
		"[${reportPart}]"
	else
		"[${reportPart},${received}]"
}

## An envelope's parts with the enclosing `[` and `]` removed (`""` for `[]`),
## which is what lets an app hand the parts it received back verbatim.
envelopeParts : Str -> Str
envelopeParts = |envelope|
	match Str.split_first(envelope, "[") {
		Err(_) => ""

		Ok({ before: _, after }) =>
			match Str.split_last(after, "]") {
				Err(_) => after
				Ok({ before, after: _ }) => before
			}
	}

## The first `"<field>":"` … `"` string in `envelope` (`""` when the field is
## not there), the simple reading the platform's other JSON parsers use: the
## value must not contain a quote of its own.
firstField : Str, Str -> Str
firstField = |envelope, prefix|
	match Str.split_first(envelope, prefix) {
		Err(_) => ""

		Ok({ before: _, after }) =>
			match Str.split_first(after, "\"") {
				Err(_) => ""
				Ok({ before, after: _ }) => before
			}
	}

## The first `"bytes":` … `]` array in `envelope`, brackets included (`"[]"`
## when the envelope carries no binary part).
firstBytes : Str -> Str
firstBytes = |envelope|
	match Str.split_first(envelope, "\"bytes\":") {
		Err(_) => "[]"

		Ok({ before: _, after }) =>
			match Str.split_first(after, "]") {
				Err(_) => "[]"
				Ok({ before, after: _ }) => "${before}]"
			}
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

## The `db <verb> [args]` commands run. Each one reports its outcome in the
## reply — a failure as `error(<message>)` in its own slot — so a check sees
## exactly how far it got, and a step the host rejected does not hide the rest.
##
## The postgres vocabulary is the platform's `db-value` JSON: the params of the
## round trip's statements are `[{"tag":"int4","value":<id>},
## {"tag":"text","value":"<note>"}]`, and a result comes back as
## `{"columns": …, "rows": …}` (see the README's database section).
dbCommand! : Str => Str
dbCommand! = |args| {
	{ first: verb, second: argument, rest: rest } = splitThree(args)

	match verb {
		"open" =>
			describeOpen!(dbOpen!(argument), "db opened handle", "db open failed")

		"open-mysql" =>
			describeOpen!(dbOpenMysql!(argument), "db mysql opened handle", "db mysql open failed")

		"open-ignite" =>
			describeOpen!(dbOpenIgnite!(argument), "db ignite opened handle", "db ignite open failed")

		"roundtrip" =>
		# roundtrip <table> <address>: the whole data plane in one invocation,
		# on a connection this command opens itself.
			match dbOpen!(rest) {
				Err(err) => "db open failed: ${err}"
				Ok(handle) => dbRoundTrip!(argument, handle)
			}

		_ =>
			"Usage: db open <address> | db open-mysql <address> | db open-ignite <address> | db roundtrip <table> <address>"
	}
}

## An `open` command's reply: the handle the effect answered with, or why it
## failed.
describeOpen! : Try(U32, Str), Str, Str => Str
describeOpen! = |opened, successPrefix, failurePrefix|
	match opened {
		Ok(handle) => "${successPrefix} ${U32.to_str(handle)}"
		Err(err) => "${failurePrefix}: ${err}"
	}

## One `dbExecute!`'s outcome: the affected-row count, or `error(<message>)`.
executeOutcome! : U32, Str, Str => Str
executeOutcome! = |handle, statement, params|
	match dbExecute!(handle, statement, params) {
		Ok(rows) => U64.to_str(rows)
		Err(err) => "error(${err})"
	}

## One `dbQuery!`'s outcome: the result JSON, or `error(<message>)`.
queryOutcome! : U32, Str, Str => Str
queryOutcome! = |handle, statement, params|
	match dbQuery!(handle, statement, params) {
		Ok(result) => result
		Err(err) => "error(${err})"
	}

## A JSON string literal for `text`: the built-in codec's `Json.to_str`, which
## quotes and escapes it (control characters included), so a note with a quote or
## a backslash still makes a params array the encoding can read.
jsonString : Str -> Str
jsonString = |text| Json.to_str(text)

## `INSERT INTO <table> (id, note) VALUES ($1, $2)` with postgres placeholders.
insertStatement : Str -> Str
insertStatement = |table|
	"INSERT INTO ${table} (id, note) VALUES ($1, $2)"

## `SELECT id, note FROM <table> WHERE id = $1`.
selectStatement : Str -> Str
selectStatement = |table|
	"SELECT id, note FROM ${table} WHERE id = $1"

## `SELECT count(*) FROM <table>`: the row count the transaction checks compare.
countStatement : Str -> Str
countStatement = |table|
	"SELECT count(*) FROM ${table}"

## The two params an insert takes, as the postgres `db-value` encoding.
insertParams : U32, Str -> Str
insertParams = |id, note|
	"[{\"tag\":\"int4\",\"value\":${U32.to_str(id)}},{\"tag\":\"text\",\"value\":${jsonString(note)}}]"

## The one param a select-by-id takes.
idParams : U32 -> Str
idParams = |id|
	"[{\"tag\":\"int4\",\"value\":${U32.to_str(id)}}]"

## Runs one transaction on `handle`: begin, insert `id`/`note`, then commit or
## roll back (whichever `end` names). Every step's outcome is in the one string,
## so a failed begin shows itself instead of hiding the insert.
transactionRoundTrip! : U32, Str, U32, Str, Str => Str
transactionRoundTrip! = |handle, table, id, note, end| {
	match dbBegin!(handle) {
		Err(err) => "begin=error(${err})"

		Ok(transaction) => {
			insertOutcome = executeOutcome!(transaction, insertStatement(table), insertParams(id, note))
			endOutcome = match end {
				"commit" =>
					match dbCommit!(transaction) {
						Ok(_) => "ok"
						Err(err) => "error(${err})"
					}

				_ =>
					match dbRollback!(transaction) {
						Ok(_) => "ok"
						Err(err) => "error(${err})"
					}
			}
			"begin=ok txn-insert=${insertOutcome} ${end}=${endOutcome}"
		}
	}
}

## The whole database round trip `db roundtrip <table> <address>` runs, reported
## the way the store round trips are: every step's outcome in one reply. The
## sequence is what makes the transaction checks meaningful — after the rolled
## back transaction the count is unchanged, and the row it inserted is not
## selectable; after the committed one the count has moved and the committed row
## is there.
dbRoundTrip! : Str, U32 => Str
dbRoundTrip! = |table, handle| {
	createOutcome = executeOutcome!(handle, "CREATE TABLE IF NOT EXISTS ${table} (id INT, note TEXT)", "[]")
	insertOutcome = executeOutcome!(handle, insertStatement(table), insertParams(1, "hello db"))
	selectOutcome = queryOutcome!(handle, selectStatement(table), idParams(1))
	countOutcome = queryOutcome!(handle, countStatement(table), "[]")
	rolledBack = transactionRoundTrip!(handle, table, 2, "rolled back", "rollback")
	countAfterRollback = queryOutcome!(handle, countStatement(table), "[]")
	committed = transactionRoundTrip!(handle, table, 3, "committed", "commit")
	countAfterCommit = queryOutcome!(handle, countStatement(table), "[]")
	selectRolledBack = queryOutcome!(handle, selectStatement(table), idParams(2))
	dropOutcome = executeOutcome!(handle, "DROP TABLE IF EXISTS ${table}", "[]")

	"db create=${createOutcome} insert=${insertOutcome} select1=${selectOutcome} count1=${countOutcome} ${rolledBack} count2=${countAfterRollback} ${committed} count3=${countAfterCommit} select2=${selectRolledBack} drop=${dropOutcome}"
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

## The `permission <verb> [args]` commands run. Each verb reports the effect's
## own answer in the one reply, so a check sees exactly how far it got: a JSON
## answer comes back as its JSON, `revoke` as the number of cards it cut, and a
## refusal as the platform's `<case>: <message>` text (`not-permitted: …`,
## `lower-bound-too-broad: …`). The handle-taking verbs are named in the usage
## reply, and a handle this instance never handed out — or one that was already
## consumed — is reported the same way as any other refusal.
permissionCommand! : Str => Str
permissionCommand! = |args| {
	verbToken = splitToken(args)
	{ first: verb, second: handleText, rest: afterHandle } = splitThree(args)
	afterVerb = verbToken.rest

	match verb {
		"principal" => "permission principal: ${getPrincipal!({})}"

		"wallet" =>
			describePermission!(permissionsWallet!({}), "permission wallet", "permission wallet failed")

		"version" =>
			describePermission!(permissionsWalletVersion!({}), "permission version", "permission version failed")

		"modules" =>
			describePermission!(permissionListModules!({}), "permission modules", "permission modules failed")

		"id" =>
			withHandle!(handleText, "Usage: permission id <handle>", |handle|
				describePermission!(permissionId!(handle), "permission id", "permission id failed"))

		"parents" =>
			withHandle!(handleText, "Usage: permission parents <handle>", |handle|
				describePermission!(permissionParents!(handle), "permission parents", "permission parents failed"))

		"expires" =>
			withHandle!(handleText, "Usage: permission expires <handle>", |handle|
				describePermission!(permissionExpiresAt!(handle), "permission expires", "permission expires failed"))

		"inspect" =>
			withHandle!(handleText, "Usage: permission inspect <handle>", |handle|
				describePermission!(permissionInspect!(handle), "permission inspect", "permission inspect failed"))

		"derive" =>
			withHandle!(handleText, "Usage: permission derive <handle> <bounds-json>", |handle|
				describeDerived!(permissionDerive!(handle, afterHandle), "permission derive"))

		"derive-from-wallet" =>
			describeDerived!(permissionDeriveFromWallet!(afterVerb), "permission derive-from-wallet")

		"derive-scope" =>
			describeDerived!(permissionDeriveScope!(afterVerb), "permission derive-scope")

		"derive-narrow" =>
			withHandle!(handleText, "Usage: permission derive-narrow <handle>", |handle|
				permissionNarrowing!(handle))

		"derive-broad" =>
			withHandle!(handleText, "Usage: permission derive-broad <handle>", |handle|
				permissionOverBroad!(handle))

		"derive-scope-narrow" =>
			withHandle!(handleText, "Usage: permission derive-scope-narrow <handle>", |handle|
				permissionScopeNarrowing!(handle))

		"install" =>
			withHandle!(handleText, "Usage: permission install <handle> <holder-json>", |handle|
				describeInstall!(permissionInstall!(handle, afterHandle)))

		"install-self" =>
			withHandle!(handleText, "Usage: permission install-self <handle>", |handle|
				permissionInstallSelf!(handle))

		"install-account" =>
			withHandle!(handleText, "Usage: permission install-account <handle>", |handle|
				permissionInstallAccount!(handle))

		"roundtrip" =>
			permissionRoundTrip!({})

		"revoke" =>
			withHandle!(handleText, "Usage: permission revoke <handle>", |handle|
				describeRevoke!(permissionRevoke!(handle)))

		"release" =>
			withHandle!(handleText, "Usage: permission release <handle>", |handle|
				describeRelease!(permissionRelease!(handle)))

		"validate" =>
			describeValidated!(permissionValidateGrant!(afterVerb))

		_ =>
			"Usage: permission principal | wallet | version | modules | roundtrip | validate <grant-json> | derive-from-wallet <bounds-json> | derive-scope <request-json> | derive-narrow <handle> | derive-broad <handle> | derive-scope-narrow <handle> | id|parents|expires|inspect|revoke|release <handle> | derive <handle> <bounds-json> | install <handle> <holder-json> | install-self <handle> | install-account <handle>"
	}
}

## Runs the body of a `permission` verb that takes a card handle, or answers
## with the verb's usage text when the argument is not a number (which is a
## typo in the command, not a card the host could report on).
withHandle! : Str, Str, (U32 => Str) => Str
withHandle! = |handleText, usage, body|
	match U32.from_str(handleText) {
		Ok(handle) => body(handle)
		Err(_) => usage
	}

## A `permission` verb's JSON answer, prefixed so the reply says which verb it
## came from.
describePermission! : Try(Str, Str), Str, Str => Str
describePermission! = |answer, prefix, failurePrefix|
	match answer {
		Ok(text) => "${prefix}: ${text}"
		Err(err) => "${failurePrefix}: ${err}"
	}

## A derivation's answer: the handle of the card it made. Deriving does not
## install anything — the card is the app's to use, pass on or release.
describeDerived! : Try(U32, Str), Str => Str
describeDerived! = |derived, prefix|
	match derived {
		Ok(handle) => "${prefix}: handle ${U32.to_str(handle)}"
		Err(err) => "${prefix} failed: ${err}"
	}

describeInstall! : Try({}, Str) => Str
describeInstall! = |installed|
	match installed {
		Ok(_) => "permission install: installed"
		Err(err) => "permission install failed: ${err}"
	}

describeRevoke! : Try(U32, Str) => Str
describeRevoke! = |revoked|
	match revoked {
		Ok(count) => "permission revoke: revoked ${U32.to_str(count)} card(s)"
		Err(err) => "permission revoke failed: ${err}"
	}

describeRelease! : Try({}, Str) => Str
describeRelease! = |released|
	match released {
		Ok(_) => "permission release: released"
		Err(err) => "permission release failed: ${err}"
	}

describeValidated! : Try({}, Str) => Str
describeValidated! = |validated|
	match validated {
		Ok(_) => "permission validate: accepted"
		Err(err) => "permission validate failed: ${err}"
	}

## The recipient the card's own grants name: the concrete holder path, taken out
## of the card's inspection (`"recipient":"…"`). A derivation request has to name
## a recipient, and the only one this agent can know is the one its own card
## carries — so the check commands below read it back instead of hard-coding a
## path that depends on the agent id.
firstRecipientOf : Str -> Str
firstRecipientOf = |viewJson|
	match Str.split_first(viewJson, "\"recipient\":\"") {
		Ok({ before: _, after }) =>
			match Str.split_first(after, "\"") {
				Ok({ before, after: _ }) => before
				Err(_) => ""
			}

		Err(_) => ""
	}

## The bounds object a narrowing derivation sends: one `network` grant narrowed
## to a single destination, addressed to whoever the parent card is addressed to.
## The card allows any destination, so this is strictly narrower than its own
## lattice — the derivation the runtime accepts.
narrowingBounds : Str -> Str
narrowingBounds = |recipient|
	"{\"lowerPositive\":[{\"class\":\"network\",\"owner\":\"\",\"recipient\":\"${recipient}\",\"verb\":\"connect\",\"resourceId\":\"example.test:443\"}],\"lowerNegative\":[],\"upperPositive\":[],\"upperNegative\":[],\"lifespan\":{\"iso8601\":\"PT1H\"}}"

## The bounds object an over-broad derivation sends: an `account` grant, which no
## card of this agent's default lattice covers at all. The runtime refuses it
## with `lower-bound-too-broad`, which is the enforcement check — nothing here is
## stubbed.
overBroadBounds : Str
overBroadBounds = "{\"lowerPositive\":[{\"class\":\"account\",\"owner\":\"alice@example.com\",\"recipient\":\"alice@example.com\",\"verb\":\"view\",\"resourceId\":\"\"}],\"lowerNegative\":[],\"upperPositive\":[],\"upperNegative\":[],\"lifespan\":{\"iso8601\":\"PT1H\"}}"

## A narrowing derivation built from the card's own inspection.
permissionNarrowing! : U32 => Str
permissionNarrowing! = |handle|
	match permissionInspect!(handle) {
		Err(err) => "permission narrowing failed: ${err}"

		Ok(view) =>
			describeDerived!(permissionDerive!(handle, narrowingBounds(firstRecipientOf(view))), "permission narrowing")
	}

## A derivation the runtime refuses: an `account` grant, which the card's lattice
## cannot cover.
permissionOverBroad! : U32 => Str
permissionOverBroad! = |handle|
	describeDerived!(permissionDerive!(handle, overBroadBounds), "permission over-broad")

## An invocation-local card derived from the wallet card: the same narrowed
## network grant, but as a `derive-scope` request (which takes concrete grants
## only, so the card's own recipient is what makes it concrete).
permissionScopeNarrowing! : U32 => Str
permissionScopeNarrowing! = |handle|
	match permissionInspect!(handle) {
		Err(err) => "permission scope failed: ${err}"

		Ok(view) =>
			describeDerived!(
				permissionDeriveScope!(
					"{\"parents\":[${U32.to_str(handle)}],\"lowerPositive\":[{\"class\":\"network\",\"owner\":\"\",\"recipient\":\"${firstRecipientOf(view)}\",\"verb\":\"connect\",\"resourceId\":\"example.test:443\"}],\"lowerNegative\":[],\"upperPositive\":[],\"upperNegative\":[]}",
				),
				"permission scope",
			)
	}

## This agent's own holder: the component id it resolves from its own component
## and the agent id it reports as its worker id — the `{"tag":"agent","val":…}`
## shape `getPrincipal!` and `resolveAgentId!` print.
selfHolder! : {} => Str
selfHolder! = |_| {
	componentId = match resolveComponentId!("roc-golem:counter") {
		Ok(id) => id
		Err(_) => "unknown"
	}
	"{\"tag\":\"agent\",\"val\":{\"componentId\":\"${componentId}\",\"agentId\":${jsonString(workerId!({}))}}}"
}

## Installs a derived card back onto this agent, the one target kind the runtime
## supports.
permissionInstallSelf! : U32 => Str
permissionInstallSelf! = |handle|
	describeInstall!(permissionInstall!(handle, selfHolder!({})))

## The installation the runtime refuses before looking at anything else: only
## agent targets are supported, so an `account` holder is `not-permitted`.
permissionInstallAccount! : U32 => Str
permissionInstallAccount! = |handle| {
	componentId = match resolveComponentId!("roc-golem:counter") {
		Ok(id) => id
		Err(_) => "unknown"
	}
	describeInstall!(permissionInstall!(handle, "{\"tag\":\"account\",\"val\":\"${componentId}\"}"))
}

## The whole permission surface in one invocation, every step's outcome in the
## one reply, the way the store and database round trips report theirs. It lists
## the wallet itself (so the handle is always the first card of a fresh
## instance), then runs the sequence that matters for the rules this platform
## adds on top of the interfaces: the borrowing effects leave a handle usable,
## `revoke` spends a handle, a second revoke of the same handle is a clean error
## rather than a trap, `release` gives a card up, and a derivation the card
## cannot cover is the runtime's refusal. A scope card is inspected and released
## but never revoked: it lives only for the invocation that derived it.
permissionRoundTrip! : {} => Str
permissionRoundTrip! = |_| {
	walletReply = permissionsWallet!({})
	handle = match walletReply {
		Ok(cards) => firstHandleOf(cards)
		Err(_) => 0
	}

	walletOutcome = match walletReply {
		Ok(cards) => "wallet: ${U32.to_str(handle)} first, ${walletTagOf(cards)}"
		Err(err) => "wallet failed: ${err}"
	}
	versionOutcome = describePermission!(permissionsWalletVersion!({}), "version", "version failed")
	modulesOutcome = describePermission!(permissionListModules!({}), "modules", "modules failed")

	idOutcome = describePermission!(permissionId!(handle), "id", "id failed")
	parentsOutcome = describePermission!(permissionParents!(handle), "parents", "parents failed")
	expiresOutcome = describePermission!(permissionExpiresAt!(handle), "expires", "expires failed")
	inspectOutcome = describePermission!(permissionInspect!(handle), "inspect", "inspect failed")
	narrowOutcome = permissionNarrowing!(handle)
	broadOutcome = permissionOverBroad!(handle)
	scopeOutcome = permissionScopeNarrowing!(handle)

	revokeOutcome = describeRevoke!(permissionRevoke!(handleOfDerived!(narrowOutcome)))
	revokeAgainOutcome = describeRevoke!(permissionRevoke!(handleOfDerived!(narrowOutcome)))
	scopeReleaseOutcome = describeRelease!(permissionRelease!(handleOfDerived!(scopeOutcome)))
	releaseOutcome = describeRelease!(permissionRelease!(handle))
	releaseAgainOutcome = describeRelease!(permissionRelease!(handle))
	unknownOutcome = describePermission!(permissionInspect!(handle), "unknown inspect", "unknown inspect failed")

	validOutcome = describeValidated!(permissionValidateGrant!(validGrant))
	invalidOutcome = describeValidated!(permissionValidateGrant!(invalidGrant))

	"${walletOutcome} ${versionOutcome} ${modulesOutcome} ${idOutcome} ${parentsOutcome} ${expiresOutcome} ${inspectOutcome} ${narrowOutcome} ${broadOutcome} ${scopeOutcome} ${revokeOutcome} ${revokeAgainOutcome} ${scopeReleaseOutcome} ${releaseOutcome} ${releaseAgainOutcome} ${unknownOutcome} ${validOutcome} ${invalidOutcome}"
}

## The grant the kernel accepts: every part of it is concrete, which is what
## `validate-grant` checks (a slot such as `?agent` is an `invalid-pattern`
## there, because a slot only means something inside a card).
validGrant : Str
validGrant = "{\"class\":\"filesystem\",\"owner\":\"alice@example.com/shop/prod/cart-svc/CartAgent\",\"recipient\":\"alice@example.com/shop/prod/cart-svc/CartAgent\",\"verb\":\"read\",\"resourceId\":\"/data/**\"}"

## The same with a class the kernel does not know.
invalidGrant : Str
invalidGrant = "{\"class\":\"nope\",\"owner\":\"alice@example.com/shop/prod/cart-svc/CartAgent\",\"recipient\":\"alice@example.com/shop/prod/cart-svc/CartAgent\",\"verb\":\"read\",\"resourceId\":\"/data/**\"}"

## The first card handle a wallet listing named (`[{"handle":1,…}, …]`), or 0
## when the listing named none — which every effect reports as an unknown handle.
firstHandleOf : Str -> U32
firstHandleOf = |cardsJson|
	match Str.split_first(cardsJson, "\"handle\":") {
		Ok({ before: _, after }) => handleNumber(after)
		Err(_) => 0
	}

## The number at the start of `after`, which a wallet entry ends with a comma or
## the closing brace (the field order is the encoder's, so both are read).
handleNumber : Str -> U32
handleNumber = |after|
	match Str.split_first(after, ",") {
		Ok({ before, after: _ }) => handleDigits(before)
		Err(_) =>
			match Str.split_first(after, "}") {
				Ok({ before, after: _ }) => handleDigits(before)
				Err(_) => 0
			}
	}

handleDigits : Str -> U32
handleDigits = |text|
	match U32.from_str(text) {
		Ok(handle) => handle
		Err(_) => 0
	}

## The kind the first card of a wallet listing is labelled as (`concrete`,
## `template` or `scope`).
walletTagOf : Str -> Str
walletTagOf = |cardsJson|
	match Str.split_first(cardsJson, "\"tag\":\"") {
		Ok({ before: _, after }) =>
			match Str.split_first(after, "\"") {
				Ok({ before, after: _ }) => before
				Err(_) => "none"
			}

		Err(_) => "none"
	}

## The card handle a `permission <verb>` reply reported, or 0 when the step
## failed — which the consuming effect then reports as an unknown handle, so a
## failed derivation shows itself in the next step instead of hiding it.
handleOfDerived! : Str => U32
handleOfDerived! = |reply|
	match Str.split_first(reply, ": handle ") {
		Ok({ before: _, after }) =>
			match U32.from_str(after) {
				Ok(handle) => handle
				Err(_) => 0
			}

		Err(_) => 0
	}

## The external durable streams commands (`golem:agent/durable-streams`).
## The protocol's state belongs to the app, and that is what these commands
## carry: the producer identity (`streamWriterOptions!`), the sequences they
## append (0, then 1, on one writer) and the checkpoint they read from next
## (`nextOffsetOf` of the batch the read answered with). Every step's outcome
## is in the one reply, so a check sees exactly how far it got — and an error
## the service reported (`not-found: …`) shows itself in the slot it happened
## in instead of hiding the rest.
streamCommand! : Str => Str
streamCommand! = |args| {
	{ first: verb, second: url, rest: value } = splitThree(args)

	match verb {
		"roundtrip" => streamRoundTrip!(url, value)
		"append" => streamAppend!(url, value)
		"read" => streamRead!(url)
		"bytes" => streamBytes!(url, value)
		_ => "Usage: stream roundtrip <url> <first> <second> | stream append <url> <text> | stream read <url> | stream bytes <url> <text>"
	}
}

## The writer options a `stream` command builds for `url`. The producer id is
## this invocation's durable idempotency key: the app owns producer identity,
## and a replay of the invocation hands the same id back, so an append retried
## after a crash repeats the protocol's idempotent request instead of becoming
## a second producer. `producerEpoch` 0 is the first ownership generation.
streamWriterOptions! : Str, Str => Str
streamWriterOptions! = |url, contentType|
	"{\"url\":${jsonString(url)},\"contentType\":${jsonString(contentType)},\"producerId\":${jsonString(idempotencyKey!({}))},\"producerEpoch\":0,\"timeoutMs\":30000}"

## The reader options: the URL, the mode (`json` or `bytes`) and a whole-attempt
## deadline inside the WIT's 1..=300000 ms.
streamReaderOptions! : Str, Str => Str
streamReaderOptions! = |url, mode|
	"{\"url\":${jsonString(url)},\"mode\":\"${mode}\",\"timeoutMs\":30000}"

## One read request: from `offset` (opaque, `-1` for the beginning) over
## `transport`, with the stream's content type left to the service's answer.
streamReadRequest : Str, Str -> Str
streamReadRequest = |offset, transport|
	"{\"checkpoint\":{\"offset\":${jsonString(offset)},\"cursor\":null},\"transport\":\"${transport}\",\"contentType\":null}"

## One append request: a `json` payload is a list of *individually encoded*
## complete values — each element carries the encoded value, quotes included,
## because the host frames the outer array from them — and a `bytes` payload is
## the text's UTF-8 bytes. `sequence` counts requests for the producer and
## `close` asks the service to commit the data and the closure together.
streamAppendRequest : Str, Str, U64, Bool -> Str
streamAppendRequest = |tag, value, sequence, close| {
	payload = match tag {
		"bytes" => "{\"tag\":\"bytes\",\"value\":${jsonString(value)}}"
		# The value is encoded as a JSON string (which is the element text the
		# host frames), and the request is JSON itself, so it is encoded twice:
		# `one` becomes the element `"one"`, and the request text carries it as
		# `"\"one\""`.
		_ => "{\"tag\":\"json\",\"value\":[${jsonString(jsonString(value))}]}"
	}

	"{\"payload\":${payload},\"sequence\":${U64.to_str(sequence)},\"close\":${boolText(close)}}"
}

## One `durableStreamAppend!`'s outcome: the receipt JSON, or `error(<message>)`
## with the platform's rendered `durable-stream-error`.
streamAppendOutcome! : U32, Str, Str, U64, Bool => Str
streamAppendOutcome! = |writer, tag, value, sequence, close|
	match durableStreamAppend!(writer, streamAppendRequest(tag, value, sequence, close)) {
		Ok(receipt) => receipt
		Err(err) => "error(${err})"
	}

## One `durableStreamRead!`'s outcome: the batch JSON, or `error(<message>)`.
streamReadOutcome! : U32, Str, Str => Str
streamReadOutcome! = |reader, offset, transport|
	match durableStreamRead!(reader, streamReadRequest(offset, transport)) {
		Ok(batch) => batch
		Err(err) => "error(${err})"
	}

## A `durableStreamRelease!`'s outcome, in the same slot style.
describeStreamRelease! : Try({}, Str) => Str
describeStreamRelease! = |released|
	match released {
		Ok(_) => "released=ok"
		Err(err) => "released=error(${err})"
	}

## The `offset` of a batch's `next` checkpoint — the token an app advances to
## once it has consumed the batch, and the one it hands the next read. An
## answer that was an error (or carries no checkpoint) reads as `-1`, the
## protocol's "from the beginning", so the next step shows itself instead of
## hiding.
nextOffsetOf : Str -> Str
nextOffsetOf = |batchJson|
	match Str.split_first(batchJson, "\"next\":{\"offset\":\"") {
		Ok({ before: _, after }) =>
			match Str.split_first(after, "\"") {
				Ok({ before, after: _ }) => before
				Err(_) => "-1"
			}

		Err(_) => "-1"
	}

## The whole JSON round trip `stream roundtrip <url> <first> <second>` runs:
## build a writer whose producer id is this invocation's durable idempotency
## key, append the two values as two messages (sequences 0 and 1 on that one
## writer), release it, then build a reader, read from the beginning, read
## again from the checkpoint the first read answered with, and release that.
## The reply carries both append receipts, both batches, the checkpoint the
## second read used (`from2`) and both releases.
streamRoundTrip! : Str, Str => Str
streamRoundTrip! = |url, values| {
	{ first: firstValue, second: secondValue, rest: _ } = splitThree(values)

	writerOutcome = match durableStreamWriter!(streamWriterOptions!(url, "application/json")) {
		Err(err) => "writer error(${err})"

		Ok(writer) => {
			firstAppend = streamAppendOutcome!(writer, "json", firstValue, 0, False)
			secondAppend = streamAppendOutcome!(writer, "json", secondValue, 1, False)
			release = describeStreamRelease!(durableStreamRelease!(writer))
			"append0=${firstAppend} append1=${secondAppend} ${release}"
		}
	}

	readerOutcome = match durableStreamReader!(streamReaderOptions!(url, "json")) {
		Err(err) => "reader error(${err})"

		Ok(reader) => {
			firstRead = streamReadOutcome!(reader, "-1", "catch-up")
			from = nextOffsetOf(firstRead)
			secondRead = streamReadOutcome!(reader, from, "catch-up")
			release = describeStreamRelease!(durableStreamRelease!(reader))
			"read1=${firstRead} from2=${from} read2=${secondRead} ${release}"
		}
	}

	"stream ${writerOutcome} ${readerOutcome}"
}

## `stream append <url> <text>`: one JSON append on a writer this command
## builds and releases, with sequence 0 and no close. The reply is the receipt.
streamAppend! : Str, Str => Str
streamAppend! = |url, value|
	match durableStreamWriter!(streamWriterOptions!(url, "application/json")) {
		Err(err) => "stream writer failed: ${err}"

		Ok(writer) => {
			receipt = streamAppendOutcome!(writer, "json", value, 0, False)
			"stream append=${receipt} ${describeStreamRelease!(durableStreamRelease!(writer))}"
		}
	}

## `stream read <url>`: one read from the beginning in JSON mode, then the
## reader is released. A URL that names no stream comes back as the service's
## own `not-found` error.
streamRead! : Str => Str
streamRead! = |url|
	match durableStreamReader!(streamReaderOptions!(url, "json")) {
		Err(err) => "stream reader failed: ${err}"

		Ok(reader) => {
			batch = streamReadOutcome!(reader, "-1", "catch-up")
			"stream read=${batch} ${describeStreamRelease!(durableStreamRelease!(reader))}"
		}
	}

## `stream bytes <url> <text>`: a byte-stream round trip — append the text's
## bytes and close the stream in one request, then read them back in bytes
## mode, where the batch reports `closed`. A byte stream does not preserve
## append boundaries, which is why the read answers the bytes and not the
## appended messages.
streamBytes! : Str, Str => Str
streamBytes! = |url, value|
	match durableStreamWriter!(streamWriterOptions!(url, "application/octet-stream")) {
		Err(err) => "stream writer failed: ${err}"

		Ok(writer) => {
			receipt = streamAppendOutcome!(writer, "bytes", value, 0, True)
			release = describeStreamRelease!(durableStreamRelease!(writer))
			readOutcome = match durableStreamReader!(streamReaderOptions!(url, "bytes")) {
				Err(err) => "reader error(${err})"

				Ok(reader) => {
					batch = streamReadOutcome!(reader, "-1", "catch-up")
					"read=${batch} ${describeStreamRelease!(durableStreamRelease!(reader))}"
				}
			}

			"stream append=${receipt} ${release} ${readOutcome}"
		}
	}
