app [main] { pf: platform "../platform/main.roc" }

import pf.Types exposing [Result]
import pf.Golem exposing [
	Agent,
	define,
	build,
	bundle,
	bundle2,
	erase,
]
import pf.Json

AlphaState : { name : Str }

BetaState : { score : I64 }

State : {}

agentA : Agent(AlphaState)
agentA =
	define(
		{ name: "agent-alpha", version: "1.0.0", description: "Alpha agent" },
		|_| Ok({ name: "alpha" }),
		|st| Json.encode(st),
		|raw| Json.decode(raw),
	)
		|> build

agentB : Agent(BetaState)
agentB =
	define(
		{ name: "agent-beta", version: "2.0.0", description: "Beta agent" },
		|_| Ok({ score: 100.I64 }),
		|st| Json.encode(st),
		|raw| Json.decode(raw),
	)
		|> build

bundled : Agent({})
bundled = bundle2(agentA, agentB)

# 1. Verify bundle2 concatenates two agents
expect {
	List.len(bundled.agents) == 2
}

# 2. Verify agent metadata order and identity
expect {
	match bundled.agents {
		[a, b, ..] =>
			a.metadata.name == "agent-alpha"
				and a.metadata.version == "1.0.0"
					and b.metadata.name == "agent-beta"
						and b.metadata.version == "2.0.0"
		_ => False
	}
}

# 3. Verify independent initialization
expect {
	match bundled.agents {
		[a, b, ..] =>
			match ((a.init)("{}"), (b.init)("{}")) {
				(Ok(resA), Ok(resB)) =>
					resA == "{\"name\":\"alpha\"}" and resB == "{\"score\":100}"
				_ => False
			}
		_ => False
	}
}

# 4. Verify erase produces Agent({}) with preserved metadata
expect {
	erasedA = erase(agentA)
	match erasedA.agents {
		[a, ..] => a.metadata.name == "agent-alpha"
		_ => False
	}
}

# 5. Verify bundle on list of erased agents
expect {
	listBundled = bundle([erase(agentA), erase(agentB)])
	List.len(listBundled.agents) == 2
}

main = bundled
