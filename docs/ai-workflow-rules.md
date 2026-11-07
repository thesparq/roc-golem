# AI Workflow Rules

These are instructions for any AI coding agent working on this project. Follow them strictly. They are rules, not suggestions.

## Overall Approach

- **Follow the project context files as the source of truth.** Before writing any code, read `project-overview.md`, `architecture.md`, `code-standards.md` and `testing.md`, then the current `progress-tracker.md`. Every line of code must align with the architecture and its principles.
- **Work spec-first.** Every unit of work begins with a clear specification of what changes, why, and where. Do not write code before the specification is agreed.
- **Deliver incrementally.** Break work into the smallest units that can be verified independently. Each unit must leave the repository in a buildable, testable state. Never batch unrelated changes.
- **End each unit at a clean boundary.** After a unit, everything that built before still builds, nothing that passed before fails, and no half-finished work is left behind.
- **Be honest about what you ran.** Report only what you actually executed. A check you could not run is reported as "not run", with the reason (for example: no Roc compiler available, WebAssembly target not installed, no network). Never describe code as working because it looks right.

## Scoping Rules

- **Work on one unit at a time.** A unit is one well-defined change: one effect, one entry-point behavior, one build step, one example fix, one document. Do not drift into adjacent concerns.
- **Do not make speculative changes.** If a change is not required by the current unit, do not make it. Do not "clean up" unrelated code. Do not refactor unless the unit explicitly requires it.
- **Do not add features that were not requested.** If the spec says an effect takes three arguments, implement three. Do not add optional parameters you expect to need later.
- **If you see a problem outside the current scope, report it.** Say: "I noticed X might cause Y. Should I address this now or file it for later?" Do not act on it without explicit instruction. Add it to the known issues in `progress-tracker.md` only when told to.

## Splitting Work

Break a unit into smaller units when any of these is true:

- The change spans more than two boundaries among: Roc app/examples, platform (`platform/`), host (`host/`), WIT or manifest (`wit/`, `golem.yaml`), and tooling or CI (`tooling/`, `.github/`).
- The implementation requires creating or modifying more than three files (not counting tests and docs).
- It changes an effect's signature on more than one side of the ABI (Roc declaration, Roc wrapper, Rust export) and also adds new behavior.
- The unit's spec is longer than half a page.
- Any part of the unit has an unclear requirement that blocks the rest.

When splitting, present the sub-units as a numbered list and ask for approval before proceeding. Implement each sub-unit as an independent step that builds and passes verification.

## Handling Missing or Ambiguous Requirements

- **Stop immediately.** Do not guess. Do not assume a reasonable default. Do not proceed with incomplete information.
- **State exactly what is missing.** Say: "The specification does not define X. Options are: (a) ..., (b) ..., or (c) .... Which should I use?"
- **Do not invent requirements.** If a rule is not stated (for example: what should an effect return when Golem refuses the call?), do not implement any behavior. Flag it and wait for a decision.
- **If the ambiguity blocks only part of the unit,** isolate the blocked portion, implement the rest, and mark the blocked part with a comment containing `TODO(SPEC)` and the open question.
- **Record each decision.** When I answer an open question, write the decision into the relevant context file and the unit's spec before continuing.

## No Fake Success

This project's main risk is code that looks finished but does nothing. Therefore:

- Never write an effect, stub, mock or example that reports success for work it did not do on the real target.
- If something cannot be done on `wasm32` with the current `wit/world.wit`, return an error that says "unsupported" and name the missing import. Do not fake it. Raise a question about adding the import.
- A test that passes only because the fake agent (`guest_bridge.rs`) gives the expected answer is **not** evidence about Roc code. Label such tests as host tests.
- Never edit an expected value in a test to match what the code now produces without saying so and explaining why the old value was wrong.

## Protected Files

The following must never be modified without explicit, direct instruction in the current unit:

- `wit/world.wit`, and anything in `wit/deps/**` (vendored upstream WIT).
- `golem.yaml`. Deployment manifests change only when a unit requires it and after confirmation.
- `host/Cargo.toml` and `host/Cargo.lock`, `tooling/pack_platform/Cargo.toml` and `tooling/pack_platform/Cargo.lock`. Dependency and feature changes need approval.
- `.github/workflows/*`. CI and release changes need approval.
- `tooling/pack_platform/**` (release packaging and checksums).
- The fixed ABI names: the four `roc__main_*` symbols in `platform/main.roc` and the Rust host, and the names and signatures of existing `rocFx*` exports (together with their declarations in `platform/Effect.roc`). They may be changed only by a unit whose purpose is that change, and only on both sides at once.
- The public SDK names exported from the `module [...]` header of `platform/Golem.roc`. Removing or renaming one breaks every consumer.
- `LICENSE`.
- Published releases and tags. Never move, delete or re-create a tag.

## Documentation Synchronization

- **Documentation and implementation are a single unit.** When a unit adds or changes behavior, update the relevant context file in the same step.
- **Which file to update for which change:**
  - New or changed contract between Roc and the host (envelopes, entry points, state rules) → `architecture.md` ("The Roc-to-host contract")
  - New, renamed or removed effect → `architecture.md` (effects count), `project-overview.md` (effects table), the README, and both skill files
  - New or changed principle, or a deviation fixed → `architecture.md` (Principles and "Known deviations"), and `progress-tracker.md`
  - New scope, goal or success criterion → `project-overview.md`
  - New coding rule or file-organization change → `code-standards.md`
  - New or changed capability, effect or example behavior → `feature-checklist.md` (add or update a row)
  - New or changed check, or a check that now exists → `testing.md` (the matrix)
  - New, started or finished unit → `docs/specs/00-build-plan.md` and `progress-tracker.md`
  - New release → README snippet (URL and hash) and `progress-tracker.md`
- **If a change invalidates a statement in a context file, you must correct it.** Leaving stale documentation is a bug.
- **Keep documentation concrete.** Replace vague phrases with specific names, counts and paths. If the code says `rocFxCreateWebhook`, the docs say `rocFxCreateWebhook`.
- **Keep `progress-tracker.md` short.** It is a status page, not a diary. When it grows beyond about 150 lines, move finished sections to `docs/temp/progress-archive.md` and keep a one-line pointer.

## Verification Checklist

Before marking a unit complete, verify every item. If any item fails, fix it before proceeding. For each item, state how you checked it and show the real command output. Commands and what they prove are in `docs/testing.md`.

1. **Format and lint:** `cargo fmt --manifest-path host/Cargo.toml --all -- --check` and `cargo clippy --manifest-path host/Cargo.toml --all-targets -- -D warnings` are clean. If Roc code changed, `roc fmt` leaves no diff.
2. **Roc compiles:** `roc check` ends with 0 errors and 0 warnings of our own for `platform/main.roc` and for every example touched. If the Roc compiler is not available in your environment, write "not run: no Roc compiler" and do not mark this item complete.
3. **Host tests:** `cargo test --manifest-path host/Cargo.toml --all-targets` passes.
4. **WebAssembly build:** the host builds for `wasm32-unknown-unknown` in release mode with `--no-default-features`, and `wasm-tools validate` accepts the result. If the target is not installed, write "not run" and why.
5. **Real build of affected examples:** `./tooling/build.sh <example>` compiles and links real Roc code and produces a component that validates. Different examples produce different components.
6. **No regressions:** everything that worked before still works. If a change could affect existing behavior, explain why it does not.
7. **Principles preserved:** review the Principles in `architecture.md`. Confirm the change does not break any, and does not add a new deviation. If it would, stop and raise the conflict.
8. **ABI parity:** every `rocFx*` in `host/src/lib.rs` is declared in `platform/Effect.roc` and wrapped in `platform/Golem.roc` (and the reverse). Run the parity check from `docs/testing.md`.
9. **Real-versus-mock parity:** every effect you touched has a real `wasm32` branch (or an explicit "unsupported" error), and no mock claims success the real path would not.
10. **State and envelope safety:** if the change touches an envelope, state encoding or an example's `serializeState` / `deserializeState`, the round-trip test (quotes, backslashes, newlines, non-ASCII) passes through real Roc.
11. **Documentation updated:** the relevant context files (see Documentation Synchronization) reflect the change. No stale or contradictory statements remain. README snippets still compile.
12. **Clean boundary:** the change leaves no partially implemented functions, commented-out code, debug printing, unused definitions, or TODO markers (other than approved `TODO(SPEC)`).
13. **Explicit sign-off.** State: "Unit complete. Built successfully. Verified items: [list with how each was checked]. Not run: [list with reasons]. Ready for next unit." If anything is in the "not run" list, the unit is **not** marked complete in `progress-tracker.md`; it is marked "implemented, awaiting verification".
