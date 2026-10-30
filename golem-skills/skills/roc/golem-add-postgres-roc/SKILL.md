---
name: golem-add-postgres-roc
description: "Querying PostgreSQL from a Roc Golem agent. Use when the user asks to connect to Postgres, run a SQL query, or store data in a relational database from an agent."
---

# PostgreSQL

> **Status: implemented.**

The database interfaces are synchronous host calls recorded in the invocation log,
so a replay answers from the recording instead of reaching the database again.

## Open a connection

```roc
import pf.Golem exposing [dbExecute!, dbOpen!, dbQuery!]

handleQuery! : State, Str => Try({ state : State, out : Str }, Str)
handleQuery! = |state, sql| {
	match dbOpen!("postgresql://user:pass@host:5432/db") {
		Err(err) => Err("could not connect: ${err}")
		Ok(conn) =>
			match dbQuery!(conn, sql, "[]") {
				Err(err) => Err("query failed: ${err}")
				Ok(rows) => Ok({ state, out: rows })
			}
	}
}
```

The handle is a `U32`. Keep it in state to reuse the connection across
invocations, or open per invocation for a short-lived one.

## Queries vs statements

```roc
dbQuery!(conn, "SELECT id, note FROM notes", "[]")          # returns JSON
dbExecute!(conn, "INSERT INTO notes (id, note) VALUES ($1, $2)", "[{\"tag\":\"int4\",\"value\":1},{\"tag\":\"text\",\"value\":\"hi\"}]")
```

`dbQuery!` answers `{"columns": [...], "rows": [...]}`; each row is an array of
`db-value` encodings in column order:

```json
{"columns":[{"ordinal":0,"name":"id","dbType":{"tag":"int4"},"dbTypeName":"INT4"}],
 "rows":[[{"tag":"int4","value":1}]]}
```

`dbExecute!` answers the number of affected rows.

## Parameters are typed

The params argument is a JSON array of `db-value` encodings, in the opened engine's
own vocabulary — Postgres uses lower-camel tags (`int4`, `text`, `timestamptz`).
`"[]"` for a statement with no parameters.

## Notes

- Address format: `postgresql://user[:password]@host[:port]/db`.
- An address Golem cannot parse comes back as an `Err` (a connection failure), not
  a trap.
- `query-stream` is not exposed: a query hands the app text.
- Passwords in the address are plain text in the manifest; see
  `golem-add-secret-roc` for the caveat.
