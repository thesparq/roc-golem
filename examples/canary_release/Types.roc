## The release coordinator's types: the state the platform snapshots, and the
## typed inputs and outputs its methods exchange.
##
## A headerless file whose top-level type is named after the file is a Roc "type
## module": every name inside the block below is importable from the other files
## in this app (`import Types exposing [State, ...]`). The platform groups its own
## types the same way (`platform/Types.roc`).
Types := {}.{
	## The rollout's own record. `events` is the durable log the read-only status
	## method reports, so the app's history lives in the state the platform
	## snapshots rather than anywhere else.
	State : {
		service : Str,
		canary : Str,
		weight : U64,
		events : List(Str),
		region : Str,
	}
	StartInput : { version : Str, weight : U64 }
	StartOutput : { version : Str, weight : U64 }
	DecisionInput : { approved : Bool, reviewer : Str }
	RollbackInput : { reason : Str }
	Status : { service : Str, canary : Str, weight : U64, events : List(Str), region : Str }
}
