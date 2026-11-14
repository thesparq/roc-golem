app [main] { pf: platform "../platform/main.roc" }

import pf.Types exposing [Result]
import pf.Golem exposing [
	Agent,
	Method,
	Spec,
	define,
	registerTyped,
	registerReadOnlyTyped,
	build,
	method,
]
import pf.Json

CountState : { n : I64 }

State : CountState

addMethod : Method(I64, I64)
addMethod = method({ name: "add", description: "Adds delta to n", promptHint: "" })

getMethod : Method({}, I64)
getMethod = method({ name: "get", description: "Returns n without modifying state", promptHint: "" })

testSpec : Spec(CountState)
testSpec =
	define(
		{ name: "spec-test", version: "1.0.0", description: "Test agent for spec builder" },
		|_| Ok({ n: 0.I64 }),
		|st| Json.encode(st),
		|raw| Json.decode(raw),
	)
		|> registerTyped(
			addMethod,
			|st, delta| Ok({ state: { n: st.n + delta }, out: st.n + delta }),
			|raw| Json.decode(raw),
			|out| Json.encode(out),
		)
		|> registerReadOnlyTyped(
			getMethod,
			|st, _| Ok(st.n),
			|_| Ok({}),
			|out| Json.encode(out),
		)

testAgent : Agent(CountState)
testAgent = build(testSpec)

# 1. Verify Spec initialization
expect {
	testSpec.name == "spec-test" and testSpec.version == "1.0.0" and testSpec.description == "Test agent for spec builder"
}

# 2. Verify Spec handlers registered
expect {
	List.len(testSpec.handlers) == 2
}

# 3. Verify handler 1 is mutating (readOnly = False)
expect {
	match testSpec.handlers {
		[h1, ..] =>
			h1.declaration.name == "add" and h1.declaration.readOnly == False and h1.declaration.description == "Adds delta to n"
		_ => False
	}
}

# 4. Verify handler 2 is read-only (readOnly = True)
expect {
	match testSpec.handlers {
		[_, h2, ..] =>
			h2.declaration.name == "get" and h2.declaration.readOnly == True and h2.declaration.description == "Returns n without modifying state"
		_ => False
	}
}

# 5. Verify Spec state serialization & deserialization codecs
expect {
	st = { n: 42.I64 }
	serialized = (testSpec.serializeState)(st)
	match (testSpec.deserializeState)(serialized) {
		Ok(deserialized) => deserialized.n == 42.I64
		Err(_) => False
	}
}

# 6. Verify Spec init produces expected initial state
expect {
	match (testSpec.init)("{}") {
		Ok(st) => st.n == 0.I64
		Err(_) => False
	}
}

# 7. Verify built Agent metadata and erased representation
expect {
	List.len(testAgent.agents) == 1
}

expect {
	match testAgent.agents {
		[erased, ..] =>
			erased.metadata.name == "spec-test"
				and erased.metadata.version == "1.0.0"
					and List.len(erased.metadata.methods) == 2
		_ => False
	}
}

# 8. Verify erased init produces serialized initial state
expect {
	match testAgent.agents {
		[erased, ..] =>
			match (erased.init)("{}") {
				Ok(json) => json == "{\"n\":0}"
				Err(_) => False
			}
		_ => False
	}
}

main = testAgent
