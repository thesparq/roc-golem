app [agent] { pf: platform "../../platform/main.roc" }

# Release Guardian — a canary-rollout orchestrator written against the typed
# authoring surface in `platform/Golem.roc`. Each method is declared once with
# `method`, bound to a typed handler with `register` / `registerReadOnly`, and
# the platform derives the metadata, the JSON dispatch and the state codecs.
#
# The blueprint's `ReleaseCoordinator` also used permission cards as values,
# secret handles, tool middleware and method streams. This platform does not
# expose those yet (see `docs/golem-1.6-parity.md`), so this app uses what it
# does expose: typed methods, config, a scheduling effect, an approval gate, the
# agent's durable filesystem, and a tool.
#
# The app is three files: this one names the value the platform's `requires`
# looks for (the header must *define* it, so a re-export is a plain binding),
# `Types.roc` carries the wire and state types, and `ReleaseCoordinator.roc`
# builds the agent itself.

import ReleaseCoordinator

agent = ReleaseCoordinator.agent
