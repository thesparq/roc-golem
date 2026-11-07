app [agent] { pf: platform "../../platform/main.roc" }

import pf.Golem exposing [Agent, bundle2]
import ReleaseCoordinator
import DiagnosticsWorker

State : {}

agent : Agent(State)
agent = bundle2(ReleaseCoordinator.agent, DiagnosticsWorker.agent)
