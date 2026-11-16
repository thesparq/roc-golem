app [main] { pf: platform "../platform/main.roc" }

import pf.Types exposing [Result, ToolCall]
import pf.Golem exposing [
	Agent,
	Tool,
	StatelessTool,
	agent,
	ephemeral,
	tool,
	effectfulTool,
	effectfulToolJson,
	effectfulStatelessTool,
	param,
	reply,
	readOnly,
	runEffect!,
	now,
]
import pf.Effect exposing [Effect, map]
import pf.Json

BookingState : {
	activeCount : U64,
	lastUpdated : U64,
}

State : BookingState

# --- 1. Pure tool definition ---
getStatusTool : Tool(BookingState)
getStatusTool = tool({
	name: "get_status",
	description: "Returns current booking count",
	params: [],
	execute: |state, _args|
		Ok("active=${Str.inspect(state.activeCount)}"),
})

# --- 2. Effectful tool definition using Effect.map and Golem.now ---
recordBookingTool : Tool(BookingState)
recordBookingTool = effectfulTool({
	name: "record_booking",
	description: "Records a new booking and timestamps it using host now",
	params: [param("guestName", "Guest name")],
	execute: |state, guestName| {
		cnt : U64
		cnt = state.activeCount
		map(
			now,
			|ts|
				Ok({
					state: {
						activeCount: cnt + 1.U64,
						lastUpdated: ts,
					},
					stateChanged: True,
					output: "Booked for ${guestName} at timestamp ${Str.inspect(ts)}",
				}),
		)
	},
})

# --- 3. Canonical Golem.agent with effectful onMessage and mixed tools ---
bookingAgent : Agent(BookingState)
bookingAgent = Golem.agent({
	name: "booking-coordinator",
	initial: { activeCount: 0.U64, lastUpdated: 0.U64 },
	tools: [getStatusTool, recordBookingTool],
	onMessage: |state, message|
		match message {
			"ping" =>
				readOnly(state, "pong")

			"record_manual" => {
				currentTime = runEffect!(now)
				reply(
					{
						activeCount: state.activeCount + 1.U64,
						lastUpdated: currentTime,
					},
					"Manual booking recorded at ${Str.inspect(currentTime)}",
				)
			}

			_ =>
				readOnly(state, "Unknown command")
		},
})

# --- 4. Stateless effectful tool for ephemeral agent ---
lookupTimeTool : StatelessTool
lookupTimeTool = effectfulStatelessTool({
	name: "lookup_time",
	description: "Looks up current host timestamp",
	params: [],
	execute: |_args|
		map(
			now,
			|ts| Ok("Current timestamp: ${Str.inspect(ts)}"),
		),
})

ephemeralAgent : Agent({})
ephemeralAgent = Golem.ephemeral({
	name: "time-oracle",
	tools: [lookupTimeTool],
	onMessage: |msg| {
		ts = runEffect!(now)
		Ok("Oracle answer for '${msg}' at ${Str.inspect(ts)}")
	},
})

# --- Test Assertions ---

erasedBooking =
	match bookingAgent.agents {
		[a, ..] => a
		_ => crash ("Expected bookingAgent to have at least one agent")
	}

# Test 1: Booking Agent Metadata and Tool registration
expect {
	erasedBooking.metadata.name == "booking-coordinator"
		and erasedBooking.metadata.mode == Durable
			and List.len(erasedBooking.metadata.tools) == 2
}

# Test 2: Tool parameter schemas and names
expect {
	getStatusTool.definition.name == "get_status"
		and getStatusTool.definition.description == "Returns current booking count"
			and List.is_empty(getStatusTool.definition.parameters)
}

expect {
	recordBookingTool.definition.name == "record_booking"
		and match recordBookingTool.definition.parameters {
			[p] => p.name == "guestName" and p.required == True and p.paramType == "string"
			_ => False
		}
}

# Test 3: Agent initialization with default and custom state
expect {
	initRes = (erasedBooking.init)("{}")
	initRes == Ok("{\"activeCount\":0,\"lastUpdated\":0}")
}

expect {
	customConfig = "{\"activeCount\":99,\"lastUpdated\":1700000000}"
	initRes = (erasedBooking.init)(customConfig)
	initRes == Ok("{\"activeCount\":99,\"lastUpdated\":1700000000}")
}

# Test 4: Ephemeral Agent Metadata & Tool definition
erasedEphemeral =
	match ephemeralAgent.agents {
		[a, ..] => a
		_ => crash ("Expected ephemeralAgent to have at least one agent")
	}

expect {
	erasedEphemeral.metadata.name == "time-oracle"
		and erasedEphemeral.metadata.mode == Ephemeral
			and List.len(erasedEphemeral.metadata.tools) == 1
}

expect {
	lookupTimeTool.definition.name == "lookup_time"
		and lookupTimeTool.definition.description == "Looks up current host timestamp"
}

expect {
	(erasedEphemeral.init)("{}") == Ok("{}")
}

main = bookingAgent
