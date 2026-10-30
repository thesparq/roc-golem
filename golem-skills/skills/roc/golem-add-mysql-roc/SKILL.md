---
name: golem-add-mysql-roc
description: "Querying MySQL from a Roc Golem agent. Use when the user asks to connect to MySQL, run a SQL query against MySQL, or store data in MySQL from an agent."
---

# MySQL

> **Status: implemented.**

Same shape as Postgres, with its own `open` and its own value vocabulary.

```roc
import pf.Golem exposing [dbExecute!, dbOpenMysql!, dbQuery!]

match dbOpenMysql!("mysql://user:pass@host:3306/db") {
	Err(err) => Err("could not connect: ${err}")
	Ok(conn) =>
		match dbQuery!(conn, "SELECT id, note FROM notes", "[]") {
			Err(err) => Err("query failed: ${err}")
			Ok(rows) => Ok({ state, out: rows })
		}
}
```

## Statements

```roc
dbExecute!(conn, "INSERT INTO notes (id, note) VALUES (?, ?)", "[{\"tag\":\"int\",\"value\":1},{\"tag\":\"text\",\"value\":\"hi\"}]")
```

MySQL's `db-value` tags are its own — `int` rather than `int4`, and so on. The
engine a handle was opened on decides the vocabulary, so a params array written for
Postgres will not work here.

## Notes

- Address format: `mysql://user[:password]@host[:port]/db`.
- `dbQuery!` and `dbExecute!` are shared with Postgres and Ignite; only the `open`
  and the value encodings differ.
- Transactions work the same way: `dbBegin!` / `dbCommit!` / `dbRollback!`, see
  `golem-add-transactions-roc`.
