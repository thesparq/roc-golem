---
name: golem-schedule-future-call-roc
description: "Scheduling a cancellable future invocation in a Roc Golem agent. Use when the user asks to cancel a scheduled call, keep a handle to a timer, or schedule work that can be called off."
---

# Cancellable Scheduling

> **Status: implemented.**

`rpcScheduleCancelable!` schedules a call and returns a **handle**, which
`rpcCancel!` spends to call it off. The handle lives in host state, so it must be
carried in the agent's own state to survive a restart.

```roc
import pf.Golem exposing [rpcCancel!, rpcScheduleCancelable!, workerId!]

setTimer = |state| {
	match rpcScheduleCancelable!(workerId!({}), "escalate", "{}", 60000) {
		Ok(handle) => { ..state, timer: handle }
		Err(_) => { ..state, timer: 0 }
	}
}

cancelTimer = |state| {
	if state.timer == 0 {
		Ok({ state, out: "no timer" })
	} else {
		match rpcCancel!(state.timer) {
			Ok(_) => Ok({ state: { ..state, timer: 0 }, out: "cancelled" })
			Err(err) => Err("cancel failed: ${err}")
		}
	}
}
```

## Details

- `rpcScheduleCancelable!` answers `Try(U32, Str)` — the handle is a plain number.
- `rpcCancel!` answers `Try({}, Str)` and **spends** the handle. Cancelling twice
  is an error, not a no-op, so clear it from state when you use it.
- A handle exists only in the instance that created it. Do not pass it to another
  agent; it will not resolve there.
- Cancelling works only while the invocation has not run yet.

## The approval-gate pattern

A release wants a rollback timer that is cancelled on approval:

```roc
approve = |state, input| {
	state2 = if input.approved { cancelTimer(state) |> ignoreResult } else { state }
	# ...
}
```

Store `0` for "no timer" and check it, because Roc has no nullable handle.
