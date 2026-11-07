## Application Building Context

Read the following files in order before implementing or making any architectural decision:

1. `docs/project-overview.md` — what the platform is, who it is for, what is in and out of scope, and what "done" means
2. `docs/architecture.md` — how the pieces fit together, the contracts between them, and the principles that must never be broken
3. `docs/code-standards.md` — implementation rules for Roc, the Rust host, WIT, the Golem manifest, and tooling
4. `docs/ai-workflow-rules.md` — how you work: scoping, splitting, protected files, documentation sync, and the verification checklist
5. `docs/testing.md` — how every change is verified, and what each check does and does not prove
6. `docs/progress-tracker.md` — the current phase, known issues, open questions, and next steps

Then, for the unit you are about to work on, read its spec in `docs/specs/`. The index of all units is `docs/specs/00-build-plan.md`.

### Non-negotiables

The full text is in `docs/ai-workflow-rules.md`. In short:

- **Spec first.** Do not write code until the spec for the unit is agreed.
- **One unit at a time.** No speculative changes, no extra features, no unrelated cleanup.
- **Stop, do not guess.** If a requirement is missing or ambiguous, stop and ask with concrete options.
- **No fake success.** Never write code that reports success for work it did not do. Never report a check as passed unless you actually ran it. If you could not run it, say "not run" and why.
- **Protected files** (listed in `docs/ai-workflow-rules.md`) are never changed without explicit instruction in the current unit.
- **The Roc code must really be compiled.** A green Rust test run alone proves nothing about the Roc side (see `docs/testing.md`).

### Keeping the docs true

Update `docs/progress-tracker.md` after each meaningful change. Add or update a row in `docs/feature-checklist.md` for any new or changed capability.

If an implementation changes the architecture, scope, or standards documented in the context files, update the relevant file **in the same unit**, before moving on. A stale statement in these files is a bug.
