---
name: golem-add-transactions-roc
description: "Grouping database statements into a transaction from a Roc Golem agent. Use when the user asks for a transaction, atomic writes across several statements, or a rollback on failure."
---

# Transactions

> **Status: implemented.**

A transaction is a host-side handle. Begin it, run statements through it, then
commit or roll back — either consumes the handle.

```roc
import pf.Golem exposing [dbBegin!, dbCommit!, dbExecute!, dbOpen!, dbRollback!]

transfer! : State, Str => Try({ state : State, out : Str }, Str)
transfer! = |state, amount| {
	match dbOpen!("postgresql://user:pass@host:5432/db") {
		Err(err) => Err("connect failed: ${err}")
		Ok(conn) =>
			match dbBegin!(conn) {
				Err(err) => Err("begin failed: ${err}")
				Ok(txn) =>
					match dbExecute!(txn, "UPDATE accounts SET balance = balance - $1 WHERE id = $2", params) {
						Err(err) =>
							# Spend the handle so it does not leak.
							ignoreEffect(dbRollback!(txn))
							Err("debit failed: ${err}")
						Ok(_) =>
							match dbExecute!(txn, "UPDATE accounts SET balance = balance + $1 WHERE id = $2", params) {
								Err(err) => {
									ignoreEffect(dbRollback!(txn))
									Err("credit failed: ${err}")
								}
								Ok(_) =>
									match dbCommit!(txn) {
										Err(err) => Err("commit failed: ${err}")
										Ok(_) => Ok({ state, out: "transferred" })
									}
							}
					}
			}
	}
}
```

## Details

- `dbBegin!(conn)` answers `Try(U32, Str)` — the transaction handle.
- Statements run through the **transaction** handle, not the connection's.
- `dbCommit!` and `dbRollback!` each **consume** it. Using it again is an error
  (`unknown ... handle`), not a silent no-op — so structure the branches to spend
  it exactly once.
- A handle lives in the instance that opened it. Do not put it in state and expect
  it to survive a restart in a usable form; open, use and close inside one
  invocation.
- A transaction that is never committed counts as rolled back when the handle is
  dropped, but rely on the explicit call.

## Notes

- Roc has no exceptions, so "roll back on failure" is the `match` on each step, as
  above. A helper that takes a list of statements keeps the nesting shallow.
- Roc's `?` operator does not exist; write the `match` out.
