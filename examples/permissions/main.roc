app [agent] { pf: platform "../../platform/main.roc" }

# A third example component whose agent type declares an explicit, narrow
# `initialCard` in `golem.yaml`, so the e2e can prove what the runtime
# *enforces* rather than only what it answers to derivation calls. The card
# replaces the broad default card Golem gives an agent type that declares none,
# so this agent may only read the file the manifest provisions for it: the read
# works, a write outside the granted root is refused, and a network connect to a
# host the card does not name is refused.
#
# It is a component of its own on purpose. This platform declares one agent type
# per app, and the existing examples deliberately keep the default card: the
# counter needs its own filesystem, its tools, agent calls, its quota resource
# and its retry policies, so a narrow card there would have to grant all of that
# back and a mistake would surface as an unrelated check failing.

import pf.Golem exposing [Agent, defineAgent]
import pf.Golem exposing [
	http!,
	permissionId!,
	permissionInspect!,
	permissionRelease!,
	permissionsWallet!,
	readFile!,
	writeFile!,
]

# Agent State. Each probe reports the runtime's answer in its reply, so the state
# only counts the calls — which is what the agent's state serializer needs
# something to carry.
State :: {
	probes : I64,
}

# The manifest provisions this file at `/seed/allow.txt`, and the agent type's
# `initialCard` grants `filesystem(?agent) @ ?agent : read : /seed/**`.
allowedPath = "/seed/allow.txt"

# A path outside the granted root, and a host the card does not name. `.invalid`
# is a reserved TLD that never resolves, so if the runtime did *not* enforce the
# card the `fetch` failure would be a name-resolution error rather than a
# refusal.
deniedPath = "/denied.txt"

deniedUrl = "http://denied.invalid:80/"

agent : Agent(State)
agent = defineAgent({
	init!: |_config|
		{ probes: 0 },

	handleMessage!: |state, message| {
		next = { probes: state.probes + 1 }

		text = match message {
			"read" =>
				match readFile!(allowedPath) {
					Ok(contents) => "permission read: ${contents}"
					Err(err) => "permission read failed: ${err}"
				}

			"write" =>
				match writeFile!(deniedPath, "denied") {
					Ok(_) => "permission write: wrote ${deniedPath}"
					Err(err) => "permission write failed: ${err}"
				}

			"fetch" =>
				match http!("{\"url\":\"${deniedUrl}\",\"method\":\"GET\"}") {
					Ok(responseJson) => "permission fetch: ${responseJson}"
					Err(err) => "permission fetch failed: ${err}"
				}

			"card" => cardReport!({})

			_ => "Usage: read | write | fetch | card"
		}

		{ state: next, replies: [text] }
	},

	handleRequest!: |state, _request|
		{
			state,
			response: { status: 404, headers: [], body: "this agent is not an HTTP router" },
		},

	# This app declares `multimodal: NoMultimodal`, so it registers no such method
	# and this is never called.
	handleMultimodal!: |state, _parts|
		{
			state,
			parts: "[{\"tag\":\"text\",\"value\":{\"text\":\"this agent does not handle multimodal input\",\"language\":null}}]",
		},

	handleToolCall!: |state, toolCall|
		{
			state,
			result: {
				id: toolCall.id,
				success: False,
				output: "{\"error\": \"Unknown tool ${toolCall.name}\"}",
				stdout: "",
				stderr: "",
			},
		},

	metadata: {
		name: "permissions-agent",
		version: "1.0.0",
		description: "An agent whose explicit initialCard the runtime enforces",
		tools: [],
		config: [],
		agentKind: Regular,
		httpMount: None,
		httpHandler: NoHandler,
		multimodal: NoMultimodal,
		openapiProvider: NoOpenApi,
	},

	serializeState: |state|
		Json.to_str({ probes: state.probes }),

	deserializeState: |stateJson| {
		parsed : Try({ probes : Try(I64, [Missing]) }, [InvalidJson(Str)])
		parsed = Json.parse(stateJson)
		match parsed {
			Err(_) => Ok({ probes: 0 })
			Ok(fields) => Ok({ probes: fields.probes ?? 0 })
		}
	},
})

# A `Try(Str, Str)` effect answer as one reply line, so a check sees which verb
# it came from and whether the runtime answered or refused.
answerText : Try(Str, Str), Str => Str
answerText = |answer, prefix|
	match answer {
		Ok(text) => "${prefix}: ${text}"
		Err(err) => "${prefix} failed: ${err}"
	}

# The first card handle out of a wallet listing, as text. A listing hands out a
# fresh handle for every card it lists, so reading one back is what lets the
# `card` command act on the card the runtime just reported whatever handles
# earlier invocations of the instance took.
#
# The entry the runtime answers with spells `handle` before `tag`
# (`[{"handle":1,"tag":"template"}]`), so the number is cut at the comma that
# ends the field — with the closing brace as the fallback if it is ever last.
firstHandleOf : Str -> Str
firstHandleOf = |walletJson|
	match Str.split_first(walletJson, "\"handle\":") {
		Ok({ before: _, after }) => handleNumberOf(after)
		Err(_) => ""
	}

handleNumberOf : Str -> Str
handleNumberOf = |afterHandle|
	match Str.split_first(afterHandle, ",") {
		Ok({ before, after: _ }) => before
		Err(_) =>
			match Str.split_first(afterHandle, "}") {
				Ok({ before, after: _ }) => before
				Err(_) => ""
			}
	}

# The card the runtime issued for this agent type, through the permission
# effects: the wallet lists it (with its kind — a card whose grants carry an
# owner slot is a template), `inspect` shows the grants, `id` names it, and the
# handle is released so the instance does not keep the card alive.
cardReport! : {} => Str
cardReport! = |_|
	match permissionsWallet!({}) {
		Err(err) => "permission card failed: wallet failed: ${err}"

		Ok(walletJson) =>
			match U32.from_str(firstHandleOf(walletJson)) {
				Err(_) => "permission card: no card handle in wallet ${walletJson}"

				Ok(handle) => {
					inspectText = answerText(permissionInspect!(handle), "inspect")
					idText = answerText(permissionId!(handle), "id")
					releaseText =
						match permissionRelease!(handle) {
							Ok(_) => "released"
							Err(err) => "release failed: ${err}"
						}
					"permission card: wallet ${walletJson}, ${inspectText}, ${idText}, ${releaseText}"
				}
			}
	}
