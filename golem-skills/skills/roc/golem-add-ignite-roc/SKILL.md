---
name: golem-add-ignite-roc
description: "Querying Apache Ignite from a Roc Golem agent. Use when the user asks to connect to Ignite, use Ignite as a cache or data grid, or run a SQL query against Ignite."
---

# Apache Ignite

> **Status: implemented.**

Ignite 2.x, with its own `open` and its own value vocabulary.

```roc
import pf.Golem exposing [dbExecute!, dbOpenIgnite!, dbQuery!]

match dbOpenIgnite!("ignite://user:password@host:10800") {
	Err(err) => Err("could not connect: ${err}")
	Ok(conn) =>
		match dbQuery!(conn, "SELECT id, note FROM notes", "[]") {
			Err(err) => Err("query failed: ${err}")
			Ok(rows) => Ok({ state, out: rows })
		}
}
```

## Address

`ignite://[user:password@]host[:port][?pool_size=N&tls=true]`, default port 10800.

## Values

Ignite's tags are distinct again — `dbInt` rather than `int4` or `int`:

```roc
dbExecute!(conn, "INSERT INTO notes (id, note) VALUES (?, ?)", "[{\"tag\":\"dbInt\",\"value\":1},{\"tag\":\"dbText\",\"value\":\"hi\"}]")
```

## Notes

- Ignite's `db-column` carries no type, so a result's columns have no `dbType` —
  unlike Postgres and MySQL.
- Transactions: `dbBegin!` / `dbCommit!` / `dbRollback!`, same as the others.
