app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [bundle]
import ReleaseCoordinator
import DiagnosticsWorker

agent = bundle([
	ReleaseCoordinator.agent,
	DiagnosticsWorker.agent,
])
