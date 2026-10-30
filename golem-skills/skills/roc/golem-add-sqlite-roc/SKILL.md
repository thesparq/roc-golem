---
name: golem-add-sqlite-roc
description: "Using SQLite from a Roc Golem agent. Use when the user asks to store data in SQLite, keep a local database file, or run embedded SQL from an agent."
---

# SQLite

> **Status: pending.** This platform does not expose a SQLite interface. Nothing
> below compiles today.

## What exists instead

- **Postgres, MySQL and Ignite** are available — see their skills. Those are
  external databases; an agent connects to one by address.
- **Key-value and blob storage** are available through the environment's own
  stores (`kvGet!` / `kvSet!`, `blobPut!` / `blobGet!`), which covers the
  small-durable-value case SQLite is often reached for.
- **The agent's own filesystem** is durable, so a JSON file is a legitimate small
  store when the data is one agent's.

## Placeholder

The Roc shape for an embedded database has not been agreed. It would most likely be
a `dbOpenSqlite!`-style opener taking the agent's own file path, sharing the
existing `dbQuery!` / `dbExecute!` / transaction surface:

```roc
# <sqlite — not yet available>
# Likely shape: dbOpenSqlite!("/data/app.db") -> Try(U32, Str), then the same
# dbQuery! / dbExecute! / dbBegin! / dbCommit! / dbRollback! the other engines use.
```

Do not invent a different surface in generated code: use Postgres/MySQL/Ignite, or
the key-value store, until this lands.
