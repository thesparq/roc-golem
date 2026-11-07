## Spec Prompt

Here is my project overview: @docs/project-overview.md
Here is my architecture: @docs/architecture.md
Here are the standards: @docs/code-standards.md and @docs/ai-workflow-rules.md
I want to implement: unit NN in @docs/specs/00-build-plan.md

Write a detailed spec file for this unit in `docs/specs/` named `NN-short-name.md`, using exactly this structure:

- Goal (1–2 sentences, specific and concrete)
- Design (structural decisions: which of Roc app / platform / host / WIT / manifest / tooling change, and the contract before and after)
- Implementation (broken into numbered sub-sections, each naming the exact files and functions touched)
- Dependencies (units that must be done first, tools or packages to install)
- Verification checklist (each item is a command to run or an observable result, taken from `docs/testing.md`)

Be as specific as possible. If anything is unclear, ask me questions before writing the spec. Do not write the file until I have reviewed the plan and told you to write it.

## Spec implementation Prompt

Read @docs/specs/NN-short-name.md.
Update @docs/progress-tracker.md to mark this unit as in progress.
Create a new branch from `main` using the format `feat/NN-short-name` (for a bug fix use `fix/NN-short-name`), e.g. `feat/01-real-roc-build`.
Implement it exactly as specified.
Do not go beyond the scope of this unit.
Run every item of the unit's verification checklist and show me the real output. If you cannot run an item, say "not run" and why.

## Spec implementation correction Prompt

The [specific element] does not match the spec.
Expected: [what the spec says].
Current: [what was built].
Fix only this. Do not change anything else.

## Spec implementation close Prompt

Implementation is complete and verified.
Mark unit NN complete in @docs/progress-tracker.md and update its status in @docs/specs/00-build-plan.md.
Make sure every context file affected by this unit has been updated.
Push the branch to GitHub.
Then create a PR to `main`. Do not merge it.

## Audit Prompt

Read the six context files listed in @AGENTS.md.
Compare them with the repository as it is now. Do not change any file.
Report, as a table, every statement in the docs that is no longer true, every principle in `docs/architecture.md` that the code currently breaks (with file and line), and every item in `docs/progress-tracker.md` that is out of date.
Mark each finding as "verified by running or reading" or "inferred".

## Ambiguity answer (use when the agent asks a question)

Option [a/b/c] is correct. Record this decision in the relevant context file (and in the unit's spec) before you continue.

## git commands

```bash
git fetch origin
git checkout main
git merge --ff-only origin/main
git branch -d feat/01-real-roc-build
git push origin --delete feat/01-real-roc-build
git checkout -b feat/02-valid-state-envelope main
```
