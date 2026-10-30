# Golem agent skills, for Roc

A set of agent skills that teach a coding agent how to write Golem agents
**in Roc** on this platform. The format is cloned from Golem's own skill set
([`golemcloud/golem` → `golem-skills/`](https://github.com/golemcloud/golem/tree/main/golem-skills)),
so an agent that already knows those skills finds the same shape here.

## Format

One skill is one directory containing one `SKILL.md`:

```
golem-skills/skills/roc/golem-<topic>-roc/SKILL.md
```

The file is YAML frontmatter and a Markdown body. Exactly two frontmatter keys:

```markdown
---
name: golem-<topic>-roc
description: "<What it does, as a gerund phrase>. Use when <the situation>."
---

# <Human-readable title>

<One to three sentences of orientation.>

## <Section>

```roc
<code>
```
```

Conventions, all taken from Golem's set:

- `name` equals the directory name.
- `description` is one quoted sentence or two. It says what the skill does **and**
  when to use it, and always contains `Use when`.
- The body opens with a `#` title, then orientation prose, then sections.
- Code is fenced with the language tag (`roc`, `shell`, `yaml`).
- Skills are short — most are 1–4 KB. A skill answers one question well rather
  than covering a subject exhaustively.

## Status of each skill

Golem's skills document a finished SDK. This platform is not finished, so every
skill here states its status in the body, in a blockquote directly under the
title:

| Marker | Meaning |
| --- | --- |
| `> **Status: implemented.**` | Works on the published platform. The code in the skill is what the examples do, and the end-to-end suite covers the behaviour. |
| `> **Status: partial.**` | Part works. The blockquote says which part, and the rest is written in the shape we intend to ship. |
| `> **Status: pending.**` | Not in the platform yet. The Roc shape shown is the one already agreed — the app-facing surface we intend to build. Treat it as the target API, not as something that compiles today. |
| `> **Status: platform-level.**` | Not application code: a manifest setting, a CLI flag, or the host's behaviour. The skill says where it lives. |

Where a feature has **no agreed shape yet**, the skill says so in words and shows
`<placeholder>`-style stand-ins rather than inventing an API that would later be
wrong. Nothing here should be silently treated as real.

## The roster

45 skills, the set every Golem SDK ships (39 that all six language directories
have, plus the five that five of six have, plus `add-sqlite`). Roc has no
language-specific extras — there is no Roc analogue of `add-rust-crate` or
`add-npm-package`.

| Skill | Status |
| --- | --- |
| `golem-add-agent-roc` | implemented |
| `golem-annotate-agent-roc` | implemented |
| `golem-mark-read-only-roc` | implemented |
| `golem-add-config-roc` | implemented |
| `golem-add-secret-roc` | partial — a secret reaches the app as a value, not as a handle |
| `golem-define-tool-roc` | implemented |
| `golem-call-tool-roc` | pending |
| `golem-tools-middleware-roc` | pending (deliberately unsupported) |
| `golem-add-http-endpoint-roc` | implemented |
| `golem-http-params-roc` | implemented |
| `golem-add-cors-roc` | implemented |
| `golem-add-http-auth-roc` | pending |
| `golem-make-http-request-roc` | implemented |
| `golem-call-another-agent-roc` | implemented |
| `golem-call-from-external-roc` | implemented |
| `golem-fire-and-forget-roc` | implemented |
| `golem-create-agent-instance-roc` | implemented |
| `golem-invoke-agent-roc` | implemented |
| `golem-trigger-agent-roc` | implemented |
| `golem-schedule-agent-roc` | implemented |
| `golem-schedule-future-call-roc` | implemented |
| `golem-recurring-task-roc` | partial — one-shot scheduling only |
| `golem-multi-instance-agent-roc` | implemented |
| `golem-stateless-agent-roc` | pending |
| `golem-streaming-agent-roc` | pending — no method streams |
| `golem-durable-streams-roc` | partial — external streams work, method streams do not |
| `golem-custom-snapshot-roc` | implemented |
| `golem-configure-durability-roc` | implemented, except atomic regions |
| `golem-parallel-workers-roc` | partial — relies on `fork!` |
| `golem-logging-roc` | implemented |
| `golem-file-io-roc` | implemented |
| `golem-add-postgres-roc` | implemented |
| `golem-add-mysql-roc` | implemented |
| `golem-add-ignite-roc` | implemented |
| `golem-add-sqlite-roc` | pending |
| `golem-add-transactions-roc` | implemented |
| `golem-add-webhook-roc` | implemented |
| `golem-wait-for-external-input-roc` | implemented |
| `golem-permission-card-roc` | implemented |
| `golem-quota-roc` | implemented |
| `golem-retry-policies-roc` | implemented |
| `golem-agent-reflection-roc` | pending — id resolution only |
| `golem-add-llm-roc` | pending |
| `golem-enable-otlp-roc` | platform-level |
| `golem-interactive-repl-roc` | platform-level |

## The `common` skills

Golem's set has 33 language-agnostic skills (`common/`) covering the CLI and the
manifest: `golem-new-project`, `golem-build`, `golem-deploy`, `golem-edit-manifest`,
`golem-view-agent-logs`, and so on. They apply here unchanged — a Roc application
has the same `golem.yaml`, the same CLI and the same deployment model — with one
delta:

```shell
# Golem's SDKs build through their own toolchain; Roc agents build with this repo's script.
./tooling/build.sh <app>          # instead of `cargo build` / `npm run build`
./tooling/build.sh all
```

Rather than duplicate 33 files that would drift, take `common/` from the Golem
repository as-is and apply that one substitution. See
[`docs/roc-golem-blueprint.md`](../../docs/roc-golem-blueprint.md) for the
platform itself.
