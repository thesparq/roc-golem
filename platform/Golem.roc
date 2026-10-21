# This module is imported by applications (`import pf.Golem`), so it still uses
# the legacy module-header syntax: in the pinned nightly (2026-10-19-601b54c)
# the new type-module format crashes the compiler when an application imports
# the module's type through the platform (`exposes`). The internal modules
# (Types, Effect) have been migrated; revisit Golem once the upstream compiler
# bug is fixed. The matching CI tolerance for the deprecation warning lives in
# .github/workflows/ci.yaml.
module [
	Agent,
	defineAgent,
	logTrace!,
	logDebug!,
	logInfo!,
	logWarn!,
	logError!,
	workerId!,
	createWebhook!,
	awaitWebhook!,
	rpc!,
	rpcFireAndForget!,
	rpcSchedule!,
	rpcScheduleCancelable!,
	rpcCancel!,
	parseAgentId!,
	makeAgentId!,
	setPersistence!,
	http!,
	websocketConnect!,
	websocketSend!,
	websocketReceive!,
	websocketClose!,
	sleep!,
	now!,
	getConfig!,
	newQuotaToken!,
	quotaReserve!,
	quotaCommit!,
	quotaSplit!,
	quotaMerge!,
	getRetryPolicies!,
	getRetryPolicy!,
	resolveRetryPolicy!,
	nextRetryDelay!,
	setRetryPolicy!,
	removeRetryPolicy!,
	trap!,
	idempotencyKey!,
	fork!,
	oplogCommit!,
	resolveAgentId!,
	resolveComponentId!,
	kvGet!,
	kvSet!,
	kvDelete!,
	kvExists!,
	blobCreateContainer!,
	blobPut!,
	blobGet!,
	blobHas!,
	blobList!,
	blobDelete!,
	readFile!,
	writeFile!,
	appendFile!,
	fileExists!,
	fileSize!,
	deleteFile!,
	makeDir!,
	listDir!,
	durableStreamReader!,
	durableStreamRead!,
	durableStreamWriter!,
	durableStreamAppend!,
	durableStreamRelease!,
	dbOpen!,
	dbOpenMysql!,
	dbOpenIgnite!,
	dbQuery!,
	dbExecute!,
	dbBegin!,
	dbCommit!,
	dbRollback!,
	getPrincipal!,
	permissionsWallet!,
	permissionsWalletVersion!,
	permissionId!,
	permissionParents!,
	permissionExpiresAt!,
	permissionInspect!,
	permissionDerive!,
	permissionDeriveFromWallet!,
	permissionDeriveScope!,
	permissionInstall!,
	permissionRevoke!,
	permissionRelease!,
	permissionListModules!,
	permissionValidateGrant!,
]

import Effect
import Types exposing [LogLevel, PersistenceMode, ToolDefinition, ToolCall, ToolResult, Metadata, HttpRequest, HttpResponse]

Agent(state) : {
	init! : Str => state,
	handleMessage! : state, Str => { state : state, replies : List(Str) },
	# A router's request handler. Only called when the app declares
	# `httpHandler: HandlesRequests`; every other agent type can answer with a
	# plain `Err`-style response.
	handleRequest! : state, HttpRequest => { state : state, response : HttpResponse },
	# The multimodal method's handler. Only called when the app declares
	# `multimodal: HandlesMultimodal`; every other agent type can answer with the
	# same kind of envelope it was handed. `parts` is the JSON envelope of the
	# caller's parts — `[{"tag":"text","value":{"text":…,"language":…}},
	# {"tag":"binary","value":{"bytes":[…],"mimeType":…}}]` — and the answer is
	# the same envelope, which is what the caller's parts become (see the README's
	# "Multimodal values").
	handleMultimodal! : state, Str => { state : state, parts : Str },
	handleToolCall! : state, ToolCall => { state : state, result : ToolResult },
	metadata : Metadata,
	serializeState : state -> Str,
	deserializeState : Str -> Try(state, Str),
}

defineAgent : Agent(state) -> Agent(state)
defineAgent = |agent|
	agent

logTrace! : Str => {}
logTrace! = |msg|
	Effect.log!(Trace, msg)

logDebug! : Str => {}
logDebug! = |msg|
	Effect.log!(Debug, msg)

logInfo! : Str => {}
logInfo! = |msg|
	Effect.log!(Info, msg)

logWarn! : Str => {}
logWarn! = |msg|
	Effect.log!(Warn, msg)

logError! : Str => {}
logError! = |msg|
	Effect.log!(Error, msg)

workerId! : {} => Str
workerId! = |_|
	Effect.getWorkerId!({})

## Creates a webhook for this agent and returns its callback URL: a POST to the
## URL completes the promise this effect created. The URL is the handle the app
## stores in its own state; `awaitWebhook!` resolves it back to the promise.
createWebhook! : {} => Try(Str, Str)
createWebhook! = |_|
	Effect.createWebhook!({})

## Waits for the POST to `url` (from `createWebhook!`) and returns its body as
## text.
awaitWebhook! : Str => Try(Str, Str)
awaitWebhook! = |url|
	Effect.awaitWebhook!(url)

rpc! : Str, Str, Str => Try(Str, Str)
rpc! = |targetWorker, fnName, payload|
	Effect.rpcInvoke!(targetWorker, fnName, payload)

## Triggers a remote method and returns its invocation identity right away
## (`{"agentId": …, "idempotencyKey": …}`) instead of waiting for the reply.
rpcFireAndForget! : Str, Str, Str => Try(Str, Str)
rpcFireAndForget! = |targetWorker, fnName, payload|
	Effect.rpcFireAndForget!(targetWorker, fnName, payload)

## Schedules a remote method for `delayMillis` from now and returns the
## scheduled invocation's identity, in the same shape as `rpcFireAndForget!`.
rpcSchedule! : Str, Str, Str, U64 => Try(Str, Str)
rpcSchedule! = |targetWorker, fnName, payload, delayMillis|
	Effect.rpcSchedule!(targetWorker, fnName, payload, delayMillis)

## The same as `rpcSchedule!`, but returns a handle for `rpcCancel!`, which
## cancels the invocation as long as it has not run yet.
rpcScheduleCancelable! : Str, Str, Str, U64 => Try(U32, Str)
rpcScheduleCancelable! = |targetWorker, fnName, payload, delayMillis|
	Effect.rpcScheduleCancelable!(targetWorker, fnName, payload, delayMillis)

## Cancels a scheduled invocation that has not run yet. The handle comes from
## `rpcScheduleCancelable!` and can be used once.
rpcCancel! : U32 => Try({}, Str)
rpcCancel! = |handle|
	Effect.rpcCancel!(handle)

## Splits an agent id into `{"agentType": …, "constructor": …,
## "phantomId": …}` — the same resolution `rpc!` does on its target.
parseAgentId! : Str => Try(Str, Str)
parseAgentId! = |agentId|
	Effect.parseAgentId!(agentId)

## Builds an agent id from an agent type name and its constructor parameter,
## the inverse of `parseAgentId!`.
makeAgentId! : Str, Str => Try(Str, Str)
makeAgentId! = |agentTypeName, constructorParam|
	Effect.makeAgentId!(agentTypeName, constructorParam)

setPersistence! : PersistenceMode => {}
setPersistence! = |mode|
	Effect.setPersistenceMode!(mode)

http! : Str => Try(Str, Str)
http! = |reqJson|
	Effect.httpRequest!(reqJson)

websocketConnect! : Str => Try(U32, Str)
websocketConnect! = |url|
	Effect.wsConnect!(url)

websocketSend! : U32, Str => Try({}, Str)
websocketSend! = |handle, msg|
	Effect.wsSend!(handle, msg)

websocketReceive! : U32 => Try(Str, Str)
websocketReceive! = |handle|
	Effect.wsReceive!(handle)

websocketClose! : U32 => Try({}, Str)
websocketClose! = |handle|
	Effect.wsClose!(handle)

sleep! : U64 => {}
sleep! = |millis|
	Effect.sleepMillis!(millis)

now! : {} => U64
now! = |_|
	Effect.nowMillis!({})

getConfig! : Str => Try(Str, Str)
getConfig! = |key|
	Effect.getConfig!(key)

## Asks the host for a quota token on the resource `resourceName`, which the
## manifest declares under `resourceDefaults.<environment>.<resourceName>`.
## `expectedUse` is the amount this token's reservations are sized for: it tells
## the host how much credit to schedule, and consumes nothing by itself. The
## token lives host-side; the number returned is the handle the other quota
## effects work with.
newQuotaToken! : Str, U64 => U32
newQuotaToken! = |resourceName, expectedUse|
	Effect.newQuotaToken!(resourceName, expectedUse)

## Reserves `amount` from the token's allocation. `Ok` is the handle of a
## reservation for `quotaCommit!`; `Err` is the rejection a `reject`ing resource
## answered with, including its estimate of the wait for capacity when it has
## one. A `throttle`ing resource waits here instead, and a `terminate`ing one
## fails the invocation.
quotaReserve! : U32, U64 => Try(U32, Str)
quotaReserve! = |token, amount|
	Effect.quotaReserve!(token, amount)

## Commits what the reserved work actually used: less than was reserved returns
## the unused capacity, more charges the excess to the token as debt, and a
## reservation that is never committed counts as zero. A handle is spent once.
quotaCommit! : U32, U64 => Try({}, Str)
quotaCommit! = |reservation, used|
	Effect.quotaCommit!(reservation, used)

## Splits `childExpectedUse` off the token into a second one. Both stay in this
## agent instance — quota tokens cannot travel between agents in this platform.
quotaSplit! : U32, U64 => Try(U32, Str)
quotaSplit! = |token, childExpectedUse|
	Effect.quotaSplit!(token, childExpectedUse)

## Merges `other` back into the token, consuming it. The two tokens have to name
## the same resource.
quotaMerge! : U32, U32 => Try({}, Str)
quotaMerge! = |token, other|
	Effect.quotaMerge!(token, other)

## Every retry policy active for this agent, as a JSON array of named policies
## (`[{"name": …, "priority": …, "predicate": {"nodes": […]},
## "policy": {"nodes": […]}}, …]`). The trees are the host's flattened form —
## the root is `nodes[0]` and children are indices into the same list — with
## durations as nanosecond counts; the README's "Quotas and retry" section spells
## the shape out.
getRetryPolicies! : {} => Str
getRetryPolicies! = |_|
	Effect.getRetryPolicies!({})

## One policy by name, the same JSON as above — or `""` when no policy has that
## name.
getRetryPolicy! : Str => Try(Str, Str)
getRetryPolicy! = |name|
	Effect.getRetryPolicy!(name)

## The first policy whose predicate matches `verb` / `nounUri` with the given
## properties (a JSON object of strings, integers and booleans, `{}` for none),
## as policy JSON — or `""` when none matches. A predicate the host cannot
## evaluate comes back as `Err`, without failing the invocation.
resolveRetryPolicy! : Str, Str, Str => Try(Str, Str)
resolveRetryPolicy! = |verb, nounUri, propertiesJson|
	Effect.resolveRetryPolicy!(verb, nounUri, propertiesJson)

## Advances the host's retry evaluator once, after a failed attempt: `policyJson`
## is a policy from above (`""` takes the agent's configured default),
## `stateJson` the `state` the previous decision returned (`[]` starts a
## sequence), `elapsedNanos` the time since the first attempt, and `contextJson`
## `{"verb": …, "nounUri": …, "properties": {…}}`. The answer is
## `{"state": […], "delayNanos": …}`, with `null` for a delay when the policy is
## exhausted (a zero delay still allows another attempt).
nextRetryDelay! : Str, Str, U64, Str => Try(Str, Str)
nextRetryDelay! = |policyJson, stateJson, elapsedNanos, contextJson|
	Effect.nextRetryDelay!(policyJson, stateJson, elapsedNanos, contextJson)

## Adds or overwrites a named policy, taking the JSON `getRetryPolicies!`
## prints. The host records it in the agent's oplog, so it outlives a restart.
setRetryPolicy! : Str => Try({}, Str)
setRetryPolicy! = |policyJson|
	Effect.setRetryPolicy!(policyJson)

## Removes a named policy (also recorded in the oplog). A policy the manifest
## declared comes back on the next deployment; this removes the agent's own copy.
removeRetryPolicy! : Str => {}
removeRetryPolicy! = |name|
	Effect.removeRetryPolicy!(name)

## Fails the current invocation with `reason` and never returns. The host turns
## the call into a trap, so the failure is permanent and uncaught: Golem does not
## retry it, and the app has no result to branch on. A condition the app should
## handle belongs in its own error handling instead (`Try`, a `Err` reply, …).
trap! : Str => {}
trap! = |reason|
	Effect.trap!(reason)

## A durable key for an external write, as a UUID string. Golem derives it from
## the position of this call in the invocation's oplog, so a replay of the same
## invocation (a retry, or recovery after a crash) hands the same call the key the
## earlier attempt used — which is what makes a repeated write duplicate-safe at
## the peer. Pass it to whatever the peer treats as an idempotency key; a header
## alone is not a guarantee. Two calls in one invocation are two positions, so
## each gets its own key.
idempotencyKey! : {} => Str
idempotencyKey! = |_|
	Effect.idempotencyKey!({})

## Forks the agent at this point in the invocation: both copies continue from
## here, and each gets its own result — `Ok("original:<phantom-id>")` on the copy
## the caller is invoking, `Ok("forked:<phantom-id>")` on the new one. The phantom
## id is the same on both sides and is what the forked copy's agent id carries;
## the fork is durable, so neither copy replays the work that led to it twice.
## The two copies are separate agents after this point — they can rejoin through a
## promise or talk over `rpc!`.
fork! : {} => Try(Str, Str)
fork! = |_|
	Effect.fork!({})

## Blocks until the oplog is durable on `replicas` replicas (or on all of them,
## when the deployment has fewer). It is a replication barrier for this agent's
## own history: not a transaction and no guarantee about an external system.
oplogCommit! : U8 => {}
oplogCommit! = |replicas|
	Effect.oplogCommit!(replicas)

## The agent id for `agentName` in the `componentReference` component, as
## `{"componentId": …, "agentId": …}`. The reference is the component's name as
## the manifest declares it (`"roc-golem:counter"`); the name is an agent id
## string such as `counter-agent("c-1")`, and it is not checked for existence. A
## reference that names no deployed component is reported as `Err`.
resolveAgentId! : Str, Str => Try(Str, Str)
resolveAgentId! = |componentReference, agentName|
	Effect.resolveAgentId!(componentReference, agentName)

## The component id of a deployed component, as a UUID string. The reference is
## the component's name as the manifest declares it (`"roc-golem:counter"`); a
## reference that names no deployed component is reported as `Err`.
resolveComponentId! : Str => Try(Str, Str)
resolveComponentId! = |componentReference|
	Effect.resolveComponentId!(componentReference)

## The key-value store (`wasi:keyvalue/eventual`): buckets of key-value pairs,
## shared by the whole environment rather than owned by one agent. Values go in
## and come back as lossy-UTF-8 text — bytes are the store's wire type and this
## platform carries them the way `http!` carries a body — and a bucket is made
## by the first write to it, so there is nothing to open first.
##
## `kvGet!` answers with the value's text, and with the JSON literal `null`
## when the bucket holds no such key. That is how a missing key is told apart
## from an empty value (an empty value reads as `""`); the one value it cannot
## tell apart is stored text that is itself `null`, which `kvExists!` resolves.
kvGet! : Str, Str => Try(Str, Str)
kvGet! = |bucket, key|
	Effect.kvGet!(bucket, key)

## Stores `value` (as UTF-8 bytes) at `key` in `bucket`, replacing whatever was
## there. The bucket comes into existence with the first write.
kvSet! : Str, Str, Str => Try({}, Str)
kvSet! = |bucket, key, value|
	Effect.kvSet!(bucket, key, value)

## Removes `key` from `bucket`. Deleting a key that is not there is not an
## error.
kvDelete! : Str, Str => Try({}, Str)
kvDelete! = |bucket, key|
	Effect.kvDelete!(bucket, key)

## Whether `bucket` holds `key` — the direct check, without reading the value.
kvExists! : Str, Str => Try(Bool, Str)
kvExists! = |bucket, key|
	Effect.kvExists!(bucket, key)

## The blobstore (`wasi:blobstore`): containers of named objects, shared by the
## whole environment. Object bodies go in and come back as lossy-UTF-8 text,
## the convention above; container and object names are plain strings, so no
## handle reaches the app.

## Creates `name` if the container does not exist yet. Calling it on one that
## is already there leaves the container and its objects alone, so an app can
## ensure its container with this one call.
blobCreateContainer! : Str => Try({}, Str)
blobCreateContainer! = |name|
	Effect.blobCreateContainer!(name)

## Stores `value` as the object `object` in `container`, replacing an object
## with that name. The container has to exist (`blobCreateContainer!`).
blobPut! : Str, Str, Str => Try({}, Str)
blobPut! = |container, object, value|
	Effect.blobPut!(container, object, value)

## The whole object as lossy-UTF-8 text. A container or object that is not
## there is reported as `Err`.
blobGet! : Str, Str => Try(Str, Str)
blobGet! = |container, object|
	Effect.blobGet!(container, object)

## Whether `container` holds an object with that name.
blobHas! : Str, Str => Try(Bool, Str)
blobHas! = |container, object|
	Effect.blobHas!(container, object)

## The object names in `container` as a JSON array of strings (`[]` when the
## container is empty). The order is the store's own — the interface promises
## none — so an app that needs a stable one sorts the array.
blobList! : Str => Try(Str, Str)
blobList! = |container|
	Effect.blobList!(container)

## Deletes the object. Deleting one that is not there is not an error.
blobDelete! : Str, Str => Try({}, Str)
blobDelete! = |container, object|
	Effect.blobDelete!(container, object)

## The agent's own filesystem (`wasi:filesystem`): files the agent reads and
## writes live in its sandbox, one filesystem per agent, and every write is
## recorded in the agent's oplog, so it is replayed with the invocation that made
## it and the files survive restarts, snapshots and agent updates. The
## manifest's per-agent `files:` list provisions the initial files; HTTP mounts
## *serve* files out of the same filesystem, which is a different thing (a mount
## answers requests without invoking the agent).
##
## Paths are resolved against the agent's root: a leading `/` names the root (the
## spelling a manifest `files:` target uses), so `/seed/a.txt` and `seed/a.txt`
## are the same file; `.` and `..` normalize inside the root; and a path that
## would step above the root fails with `escapes the agent's filesystem root`.
## File contents travel as lossy-UTF-8 text, the convention `http!` uses for
## bytes, and no file handle reaches the app — every effect opens the file it
## names and closes it again.

## The whole file as text. A file that is not there (or cannot be read) is
## reported in the `Err` channel with the host's error code, such as
## `no-entry`.
readFile! : Str => Try(Str, Str)
readFile! = |path|
	Effect.readFile!(path)

## Creates the file (truncating what was there) and writes the text's UTF-8
## bytes. Missing parent directories are an error; `makeDir!` creates them one
## level at a time.
writeFile! : Str, Str => Try({}, Str)
writeFile! = |path, value|
	Effect.writeFile!(path, value)

## Writes the text's bytes at the end of the file, creating it when it is not
## there yet.
appendFile! : Str, Str => Try({}, Str)
appendFile! = |path, value|
	Effect.appendFile!(path, value)

## Whether anything is there under the path — a directory or a file, a symlink
## followed to what it points at. A path whose parent does not exist answers
## `False`, not `Err`.
fileExists! : Str => Try(Bool, Str)
fileExists! = |path|
	Effect.fileExists!(path)

## The size of the file in bytes (for a directory, the size of the directory's
## own record; for a symlink, the length of the path it holds).
fileSize! : Str => Try(U64, Str)
fileSize! = |path|
	Effect.fileSize!(path)

## Removes the file. A path that is not a file at all — a directory, or nothing
## — is reported (`is-directory`, `no-entry`) rather than silently ignored.
deleteFile! : Str => Try({}, Str)
deleteFile! = |path|
	Effect.deleteFile!(path)

## Creates one directory. Its parent has to exist already, so this is `mkdir`,
## not `mkdir -p`; creating one that exists is reported as `exist`.
makeDir! : Str => Try({}, Str)
makeDir! = |path|
	Effect.makeDir!(path)

## The entries of a directory as a JSON array of `{"name": …,
## "type": …}`, sorted by name so two listings of an unchanged directory read
## the same. `type` is `"file"`, `"dir"`, `"symlink"`, or `"other"` for the
## kinds the interface leaves unclassified (devices, fifos, sockets). `[]` is an
## empty directory.
listDir! : Str => Try(Str, Str)
listDir! = |path|
	Effect.listDir!(path)

## External durable streams (`golem:agent/durable-streams`): a reader and a
## writer for a service that speaks the Durable Streams protocol — a stream
## hosted by another service, addressed by URL. The host owns the HTTP, the
## framing and (for an authenticated stream) the credential; each effect is one
## protocol attempt, recorded in the agent's oplog like every other host call,
## so a replay is answered from the recording instead of reaching the service
## again.
##
## **The app owns the state.** A checkpoint (a server-generated `offset` plus an
## optional `cursor`), a producer identity (`producerId` + `producerEpoch`), the
## next `sequence` and the retry policy are the app's to carry, exactly as the
## official SDKs document; nothing here retries, advances a checkpoint or
## remembers a pending append for the app. `-1` starts a read from the service's
## beginning and `now` asks it to resolve the current tail once during catch-up
## (the checkpoint tokens are opaque: pass them back unchanged).
##
## Both resources are host-side; the app carries them as `U32` handles, the way
## it carries a WebSocket or a permission card. `durableStreamRead!` and
## `durableStreamAppend!` borrow the handle — the reader or writer stays usable
## for the next call — and `durableStreamRelease!` gives it up. Roc holds a
## plain number, so the host cannot see when an app is done; release what you do
## not keep. Releasing a writer never rolls back an append.
##
## The structured values travel as JSON in the WIT shape with camelCase names
## (`retry_json`'s convention); the README's "Durable streams" section spells
## every shape out. A batch's `payload` is bytes, and bytes travel as
## lossy-UTF-8 text (the `http!` convention), so `payloadBytes` — the byte
## length the host received — is how an app notices that the text lost
## something. An error arrives as `"<kind>: <message>"` (`not-found: …`,
## `rate-limited: …`), with `(retryAfterMs=…)`, `(producerEpoch=…)` and
## `(expectedSequence=…)` appended when the error carries them.
##
## Authentication is the one gap: the constructors take the WIT's optional
## `secret` and this platform passes none, because it has no secret-handle
## surface yet (`getConfig!` hands an app a secret's *value*, not a handle). An
## authenticated stream is a follow-up (see the README).

## Builds a reader for `url`: `{"url": …, "mode": "json"|"bytes",
## "timeoutMs": …}`, where the deadline is the whole attempt's and must be 1
## through 300000 ms. `Ok` is the handle `durableStreamRead!` and
## `durableStreamRelease!` work with; `Err` is a text this effect could not read
## (its shape, its mode, or a deadline outside the range). Construction records
## no HTTP call, so it does not reach the service — and it does not create the
## stream: a URL that does not exist yet is reported by the first read.
## `json` mode reads the service's JSON messages as the raw batch text; `bytes`
## mode reads a byte stream (whose HTTP batch boundaries are not preserved).
durableStreamReader! : Str => Try(U32, Str)
durableStreamReader! = |optionsJson|
	Effect.durableStreamReader!(optionsJson)

## One read attempt on the reader behind `handle`:
## `{"checkpoint": {"offset": …, "cursor": …|null},
## "transport": "catch-up"|"long-poll"|"sse", "contentType": …|null}`. The
## checkpoint is where to continue from (`{"offset": "-1", "cursor": null}`
## for the beginning); `contentType` pins the stream's media type, which SSE
## requires, and must agree with the reader's mode. `Ok` is the batch:
## `{"payload": …, "payloadBytes": N, "contentType": …, "next": {"offset":
## …, "cursor": …|null}, "upToDate": …, "closed": …}` — the payload as text
## plus the byte count, and `next` the checkpoint the app advances to once it
## has consumed the batch. `upToDate` says the read reached the service's tail
## and `closed` that the stream ended; an empty payload is not the end unless
## `closed` is true. An error is never the end of the stream.
durableStreamRead! : U32, Str => Try(Str, Str)
durableStreamRead! = |handle, requestJson|
	Effect.durableStreamRead!(handle, requestJson)

## Builds a writer for `url`: `{"url": …, "contentType": …, "producerId":
## …, "producerEpoch": …, "timeoutMs": …}`. The producer identity is the app's
## to choose and to keep (the epoch is the ownership generation: a higher epoch
## at sequence 0 fences older ones for the same id); the deadline rule is the
## reader's, and the epoch stays at or below 2^53-1. The writer only carries
## the identity — retaining it advances no sequence by itself.
durableStreamWriter! : Str => Try(U32, Str)
durableStreamWriter! = |optionsJson|
	Effect.durableStreamWriter!(optionsJson)

## One append attempt on the writer behind `handle`:
## `{"payload": {"tag": "json", "value": ["…", …]},
## "sequence": N, "close": …}` for JSON values — each element is one complete
## encoded value, and the host frames the outer array — or
## `{"payload": {"tag": "bytes", "value": "…"}, …}`, which appends the
## text's UTF-8 bytes. `sequence` counts *requests* for this producer, not
## messages or bytes, and it is the app's to advance: the same sequence,
## payload and `close` flag sent again is the protocol's idempotent retry of an
## uncertain append, and new data takes the next sequence. Empty data is only
## valid with `close` true, which commits the data and the closure together.
## `Ok` is the receipt `{"nextOffset": …|null, "epoch": …, "sequence": …,
## "closed": …}` — a duplicate acknowledgement may omit the offset — and the
## writer stays usable for the next sequence or for the retry.
durableStreamAppend! : U32, Str => Try(Str, Str)
durableStreamAppend! = |handle, requestJson|
	Effect.durableStreamAppend!(handle, requestJson)

## Gives a reader or writer handle up: the resource is dropped and the handle is
## freed for reuse. A handle that was never allocated, or was already released,
## is reported (`unknown durable stream handle N`), never a use-after-free.
durableStreamRelease! : U32 => Try({}, Str)
durableStreamRelease! = |handle|
	Effect.durableStreamRelease!(handle)

## Databases (`golem:rdbms`): three engines — postgres, MySQL and Apache Ignite
## 2.x — with one `open` per engine and a shared statement surface. A connection
## and a transaction are host-side resources the app carries as `U32` handles:
## `dbQuery!` and `dbExecute!` take either a connection's or a transaction's
## handle, and `dbCommit!` / `dbRollback!` consume the transaction's.
##
## Statements and values travel as JSON. The params are a JSON array of
## `db-value` encodings in the opened engine's own vocabulary, and `dbQuery!`
## answers `{"columns": […], "rows": […]}` with each column's ordinal, name,
## WIT type and the database's own type name. The README's database section
## spells the encoding out, hand-mapped cases included (a UUID is its canonical
## text; a case that carries no payload has just its tag, `{"tag": "null"}`).
##
## The three interfaces are synchronous host calls, and Golem records them in
## the agent's oplog like every other host call, so a replay is answered from
## the recorded calls rather than reaching the database again. `query-stream` is
## not exposed: the effects hand an app text.

dbOpen! : Str => Try(U32, Str)
dbOpen! = |address|
	Effect.dbOpen!(address)

## The same for MySQL. The address is `mysql://user[:password]@host[:port]/db`.
dbOpenMysql! : Str => Try(U32, Str)
dbOpenMysql! = |address|
	Effect.dbOpenMysql!(address)

## The same for Apache Ignite 2.x. The address is
## `ignite://[user:password@]host[:port][?pool_size=N&tls=true]`; the default
## port is 10800.
dbOpenIgnite! : Str => Try(U32, Str)
dbOpenIgnite! = |address|
	Effect.dbOpenIgnite!(address)

## Runs `statement` and returns its result as JSON:
## `{"columns": [{"ordinal": 0, "name": "id", "dbType": {"tag": "int4"},
## "dbTypeName": "INT4"}, …], "rows": [[{"tag": "int4", "value": 1}, …], …]}`.
## A row is the JSON array of its values in column order (the WIT `db-row`'s
## single `values` field is that array), and Ignite's columns carry no type
## because its `db-column` does not.
##
## `params` is a JSON array of `db-value` encodings — `[]` for a statement
## without parameters — in the vocabulary of the engine the handle was opened
## on: `[{"tag": "int4", "value": 1}, {"tag": "text", "value": "a"}]` for
## postgres, `[{"tag": "int", "value": 1}]` for MySQL,
## `[{"tag": "dbInt", "value": 1}]` for Ignite. A params text the encoding
## cannot read is reported in the `Err` channel, like a host error.
##
## A statement that is not a query (a `CREATE`, an `INSERT`, …) belongs in
## `dbExecute!`; a `query` runs whatever the database accepts and hands back
## whatever columns it answered with.
dbQuery! : U32, Str, Str => Try(Str, Str)
dbQuery! = |handle, statement, params|
	Effect.dbQuery!(handle, statement, params)

## Runs `statement` and returns the number of affected rows. The params are the
## same JSON array `dbQuery!` takes.
dbExecute! : U32, Str, Str => Try(U64, Str)
dbExecute! = |handle, statement, params|
	Effect.dbExecute!(handle, statement, params)

## Starts a transaction on the connection and returns its handle: statements
## run through that handle until `dbCommit!` or `dbRollback!` consumes it. A
## handle is spent once — using it again is reported, not replayed.
dbBegin! : U32 => Try(U32, Str)
dbBegin! = |handle|
	Effect.dbBegin!(handle)

## Commits the transaction and consumes its handle.
dbCommit! : U32 => Try({}, Str)
dbCommit! = |handle|
	Effect.dbCommit!(handle)

## Rolls the transaction back and consumes its handle.
dbRollback! : U32 => Try({}, Str)
dbRollback! = |handle|
	Effect.dbRollback!(handle)

## Permissions (`golem:permissions`) and the principal
##
## Golem decides what an agent may do with *permission cards*: each card is an
## unforgeable capability listing grants (`class(owner) @ recipient : verb :
## resource-id`), the set of cards an agent holds is its *wallet*, and the
## runtime checks the wallet before an operation that needs one. These effects
## expose the wallet an app already holds, the inspection/derivation/
## installation/revocation operations the official SDKs expose, and the
## kernel's own introspection — they do not create authority: a card can only
## be narrowed, and an operation the cards do not allow is the runtime's
## refusal, reported in the effect's `Err`.
##
## A `permission-card` is host-side; the app carries it as a `U32` handle, the
## way it carries a WebSocket or database handle. Ownership is explicit:
##
## * `permissionId!` / `permissionParents!` / `permissionExpiresAt!` /
##   `permissionInspect!` / `permissionDerive!` / `permissionDeriveScope!`
##   **borrow** the handle — the card stays and can be used again,
## * `permissionRevoke!` and `permissionInstall!` **consume** it (the interface
##   takes ownership), so using the same handle twice is an `Err`
##   (`unknown permission card handle N`), never a use-after-free,
## * `permissionRelease!` gives a card up without using it as authority: the
##   card is dropped and the handle is freed. Roc holds a plain number, so the
##   host cannot see when an app is done with a card — a handle that is neither
##   consumed nor released stays in the instance for as long as it lives, so
##   release what you do not keep.
##
## The structured values travel as JSON in the WIT shape with camelCase names
## (`retry_json`'s convention). The README's "Permissions and the principal"
## section spells every shape out; the short version: a grant is
## `{"class":…,"owner":…,"recipient":…,"verb":…,"resourceId":…}`, a
## bounds object is `{"lowerPositive":[…],"lowerNegative":[…],
## "upperPositive":[…],"upperNegative":[…],"lifespan":{"iso8601":"P30D"}}`
## (or `null`/absent), a `deriveScope!` request adds `"parents":[<handle>,…]`
## and takes no lifespan, a holder is
## `{"tag":"account"|"app"|"agent","val":…}`, and a `permission-error`
## arrives as `"<case-name>: <message>"` (`not-permitted: …`,
## `lower-bound-too-broad: …`).

## The principal the runtime handed this invocation, as JSON:
## `{"tag":"anonymous"}`, `{"tag":"golem-user","val":{"accountId":…}}`,
## `{"tag":"agent","val":{"componentId":…,"agentId":…}}` (an RPC caller),
## or `{"tag":"oidc","val":{…}}`. The answer is the JSON literal `null` when
## the runtime handed none: a tool invocation, which the tool ABI carries no
## principal for.
getPrincipal! : {} => Str
getPrincipal! = |_|
	Effect.getPrincipal!({})

## Every card installed in the caller's wallet, as a JSON array of
## `{"tag":"concrete"|"template"|"scope","handle":N}`. A `template` card's
## grants carry slot variables (`?agent`, `?env`) that resolve for whoever ends
## up holding it; `scope` is the card the runtime lent this invocation, when it
## lent one. Each entry's handle is a card this instance now holds, so it is the
## app's to use — and to release.
permissionsWallet! : {} => Try(Str, Str)
permissionsWallet! = |_|
	Effect.permissionsWallet!({})

## The wallet's version, as `{"walletIdHash":[<bytes>],"generation":N}`. It
## changes whenever the set of installed cards does, which is how an app can
## tell that its wallet moved without listing it.
permissionsWalletVersion! : {} => Try(Str, Str)
permissionsWalletVersion! = |_|
	Effect.permissionsWalletVersion!({})

## The card's stable identifier, as a JSON string (its canonical UUID text).
permissionId! : U32 => Try(Str, Str)
permissionId! = |handle|
	Effect.permissionId!(handle)

## The card's parents in the derivation DAG, as a JSON array of UUID strings
## (`[]` for a card derived from nothing).
permissionParents! : U32 => Try(Str, Str)
permissionParents! = |handle|
	Effect.permissionParents!(handle)

## When the card stops being valid, as
## `{"seconds":…,"nanoseconds":…}` — or `null` for a card that never expires
## (for a scope card, the earliest expiry of its root cards).
permissionExpiresAt! : U32 => Try(Str, Str)
permissionExpiresAt! = |handle|
	Effect.permissionExpiresAt!(handle)

## The card's whole grant lattice, as
## `{"lowerPositive":[…],"lowerNegative":[…],"upperPositive":[…],
## "upperNegative":[…]}`. Inspection needs the `card:inspect` permission, so a
## card the caller may hold but not read is reported rather than opened.
permissionInspect! : U32 => Try(Str, Str)
permissionInspect! = |handle|
	Effect.permissionInspect!(handle)

## Derives a narrower child card from the card behind `handle`. The bounds JSON
## says which lower- and upper-positive grants to retain, which negatives to
## add, and how long the child lives:
## `{"lowerPositive":[…],"lowerNegative":[…],"upperPositive":[…],
## "upperNegative":[…],"lifespan":{"iso8601":"P30D"}}` (`lifespan` may be
## absent or `null`). A child can only narrow its parent: a grant the parent
## does not cover is refused (`lower-bound-too-broad` /
## `upper-bound-too-broad`). `Ok` is the handle of the derived card — it is *not*
## installed anywhere, and the parent is only borrowed.
permissionDerive! : U32, Str => Try(U32, Str)
permissionDerive! = |handle, boundsJson|
	Effect.permissionDerive!(handle, boundsJson)

## The same, derived from a card the caller's own wallet already holds instead
## of one named parent. The runtime picks the source card; a request no card in
## the wallet covers is refused.
permissionDeriveFromWallet! : Str => Try(U32, Str)
permissionDeriveFromWallet! = |boundsJson|
	Effect.permissionDeriveFromWallet!(boundsJson)

## Derives an invocation-local card from the cards whose handles the request
## names: `{"parents":[<handle>,…],"lowerPositive":[…],"lowerNegative":[…],
## "upperPositive":[…],"upperNegative":[…]}`, with no lifespan (the card lives
## exactly as long as the invocation that derived it, and cannot be installed or
## revoked). The requested grants must be concrete — a slot variable in a
## scope request is `invalid-pattern` — and the parents are only borrowed.
permissionDeriveScope! : Str => Try(U32, Str)
permissionDeriveScope! = |requestJson|
	Effect.permissionDeriveScope!(requestJson)

## Installs the card behind `handle` onto `holder`, a JSON
## `{"tag":"agent","val":{"componentId":…,"agentId":…}}` (the shape
## `resolveAgentId!` and `getPrincipal!` print). The card is consumed by the
## call, so the handle is spent whether or not the installation succeeded. Only
## agent targets are supported today: an `account` or `app` holder is refused.
permissionInstall! : U32, Str => Try({}, Str)
permissionInstall! = |handle, holderJson|
	Effect.permissionInstall!(handle, holderJson)

## Revokes the card behind `handle` and every card derived from it, consuming
## the handle. `Ok` is how many cards the revocation cut, the card itself
## included. Revoking a card that is already revoked is `card-revoked`, not a
## success.
permissionRevoke! : U32 => Try(U32, Str)
permissionRevoke! = |handle|
	Effect.permissionRevoke!(handle)

## Gives a card handle up: the card is dropped (which releases the runtime's own
## entry for it) and the handle is freed for reuse. A handle that was never
## allocated, or was already consumed by `permissionRevoke!` /
## `permissionInstall!` / an earlier `permissionRelease!`, is reported.
permissionRelease! : U32 => Try({}, Str)
permissionRelease! = |handle|
	Effect.permissionRelease!(handle)

## Every resource class the permission kernel knows about, as a JSON array of
## `{"className":…,"verbs":[…]}`, in the order the kernel listed them. This is
## what tells an app which classes and verbs a grant may name.
permissionListModules! : {} => Try(Str, Str)
permissionListModules! = |_|
	Effect.permissionListModules!({})

## Checks one grant — a JSON `pattern-grant` — against the kernel's schema: its
## class, owner, recipient, verb and resource-id pattern. `Ok` means the kernel
## accepts it; anything it rejects comes back in the `Err` channel
## (`unknown-resource-class: …`, `invalid-verb: …`, `invalid-pattern: …`).
permissionValidateGrant! : Str => Try({}, Str)
permissionValidateGrant! = |grantJson|
	Effect.permissionValidateGrant!(grantJson)
