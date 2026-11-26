app [main] { pf: platform "../platform/main.roc" }

import pf.Types exposing [Result]
import pf.Golem exposing [
	Agent,
	build,
	bundle3,
	bundle4,
	bundle5,
	bundleWith,
	define,
]
import pf.GolemJson

CoordinatorState : { activeBookings : U64, status : Str }

PricingState : { baseRateCents : U64, currency : Str }

AuditState : { eventsLogged : U64 }

Worker4State : { id : U64 }

Worker5State : { tag : Str }

State : {}

coordinator : Agent(CoordinatorState)
coordinator =
	define(
		{ name: "coordinator", version: "1.0.0", description: "Booking coordinator agent" },
		|_| Ok({ activeBookings: 10.U64, status: "ready" }),
		|st| GolemJson.encode(st),
		|raw| GolemJson.decode(raw),
	)
		|> build

pricingWorker : Agent(PricingState)
pricingWorker =
	define(
		{ name: "pricing-worker", version: "1.0.0", description: "Dynamic pricing worker" },
		|_| Ok({ baseRateCents: 9950.U64, currency: "USD" }),
		|st| GolemJson.encode(st),
		|raw| GolemJson.decode(raw),
	)
		|> build

auditWorker : Agent(AuditState)
auditWorker =
	define(
		{ name: "audit-worker", version: "1.0.0", description: "Audit logging worker" },
		|_| Ok({ eventsLogged: 0.U64 }),
		|st| GolemJson.encode(st),
		|raw| GolemJson.decode(raw),
	)
		|> build

worker4 : Agent(Worker4State)
worker4 =
	define(
		{ name: "worker-4", version: "1.0.0", description: "Worker 4" },
		|_| Ok({ id: 4.U64 }),
		|st| GolemJson.encode(st),
		|raw| GolemJson.decode(raw),
	)
		|> build

worker5 : Agent(Worker5State)
worker5 =
	define(
		{ name: "worker-5", version: "1.0.0", description: "Worker 5" },
		|_| Ok({ tag: "agent-5" }),
		|st| GolemJson.encode(st),
		|raw| GolemJson.decode(raw),
	)
		|> build

# --- 1. Test bundle3 ---

bundled3 : Agent({})
bundled3 = bundle3(coordinator, pricingWorker, auditWorker)

expect List.len(bundled3.agents) == 3

expect {
	match bundled3.agents {
		[a, b, c, ..] =>
			a.metadata.name == "coordinator"
				and b.metadata.name == "pricing-worker"
				and c.metadata.name == "audit-worker"
		_ => False
	}
}

expect {
	match bundled3.agents {
		[a, b, c, ..] =>
			match ((a.init)("{}"), (b.init)("{}"), (c.init)("{}")) {
				(Ok(resA), Ok(resB), Ok(resC)) =>
					resA == "{\"activeBookings\":10,\"status\":\"ready\"}"
						and resB == "{\"baseRateCents\":9950,\"currency\":\"USD\"}"
						and resC == "{\"eventsLogged\":0}"
				_ => False
			}
		_ => False
	}
}

# --- 2. Test bundleWith (Pipeline Chaining) ---

chained3 : Agent({})
chained3 =
	coordinator
		|> bundleWith(pricingWorker)
		|> bundleWith(auditWorker)

expect List.len(chained3.agents) == 3

expect {
	match chained3.agents {
		[a, b, c, ..] =>
			a.metadata.name == "coordinator"
				and b.metadata.name == "pricing-worker"
				and c.metadata.name == "audit-worker"
		_ => False
	}
}

# --- 3. Test bundle4 ---

bundled4 : Agent({})
bundled4 = bundle4(coordinator, pricingWorker, auditWorker, worker4)

expect List.len(bundled4.agents) == 4

expect {
	match bundled4.agents {
		[_, _, _, d, ..] => d.metadata.name == "worker-4"
		_ => False
	}
}

# --- 4. Test bundle5 ---

bundled5 : Agent({})
bundled5 = bundle5(coordinator, pricingWorker, auditWorker, worker4, worker5)

expect List.len(bundled5.agents) == 5

expect {
	match bundled5.agents {
		[_, _, _, _, e, ..] => e.metadata.name == "worker-5"
		_ => False
	}
}

main = bundled3
