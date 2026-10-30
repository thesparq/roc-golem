---
name: golem-file-io-roc
description: "Reading and writing files in a Roc Golem agent's own durable filesystem. Use when the user asks to write a file, read a config file, keep state on disk, or serve a file over HTTP."
---

# File I/O

> **Status: implemented.**

Every agent has its own durable filesystem. Writes are recorded in the invocation
log, so the files survive restarts, snapshots and agent updates.

## The effects

```roc
import pf.Golem exposing [
	appendFile!, deleteFile!, fileExists!, fileSize!, listDir!, makeDir!, readFile!, writeFile!,
]

writeFile!("/notes/today.txt", "hello")
appendFile!("/notes/today.txt", "\nand again")
contents = readFile!("/notes/today.txt")      # Try(Str, Str)
size = fileSize!("/notes/today.txt")          # Try(U64, Str)
there = fileExists!("/notes/today.txt")       # Try(Bool, Str)
ignoreEffect(makeDir!("/notes"))              # one level; parent must exist
listing = listDir!("/notes")                  # JSON: [{"name":..,"type":..}]
```

## Paths

A leading `/` names the agent's root — the same spelling a manifest `files:` target
uses — so `/seed/a.txt` and `seed/a.txt` are the same file. `.` and `..` normalise
inside the root; a path that would step above it fails with
`escapes the agent's filesystem root`.

## Contents are text

File contents travel as lossy UTF-8 text, the same convention `http!` uses for
bytes: there is no file handle in the app, and every effect opens, uses and closes
the file it names. For a binary payload, encode it (base64, or JSON) rather than
relying on the text round trip.

## Serving a file over HTTP

A mount can serve what the agent writes:

```roc
filesystemBindings: [
	Exact({ publicPath: ["dashboard.html"], filePath: "/dashboard.html" }),
]
```

`GET /<prefix>/<id>/dashboard.html` then answers with the file's current contents,
without invoking the agent.

## Provisioning initial files

The manifest can put files in place before the agent starts:

```yaml
agents:
  my-agent:
    files:
      - sourcePath: examples/my_app/seed/hello.txt
        targetPath: /seed/hello.txt
```

## Notes

- `makeDir!` is `mkdir`, not `mkdir -p`: create parents one level at a time.
- `deleteFile!` on a directory is reported (`is-directory`), not silently ignored.
- `listDir!` is sorted by name, so two listings of an unchanged directory compare
  equal.
