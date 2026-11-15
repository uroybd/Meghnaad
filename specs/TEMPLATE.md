# NNNN: Title

- **Status:** draft <!-- draft | ready | in progress | done | dropped -->
- **Created:** YYYY-MM-DD
- **Layer:** 1 (Taskwarrior/TaskChampion, `crates/tc-core`) | 2 (UI) | both
- **Front ends:** web, Android, terminal <!-- the ones this touches; say "all" or name the ones left out and why -->
- **Supersedes / related:** <!-- other specs, issues, commits; or none -->

## Why

What problem this solves and for whom. One or two paragraphs; the reason, not the design.

## What

What changes for the person using it, described from the outside: what they type, click or see. Include the command
lines, screens and addresses (`route.ts` `Place`) involved.

## Not doing

What is deliberately left out, so it isn't built by accident.

## Taskwarrior parity

Layer 1 only (delete otherwise). What the real `task` 3.5.0 does with the same command line, and where in its C++ that
lives. Paste the output of the real `task`, run on a scratch `TASKRC` and data directory, for the cases that matter.
Note anything odd that we copy on purpose.

## Design

How it fits the architecture (`AGENTS.md`, `docs/architecture.md`): which crates and files, any new helper in
`tc_core::helpers`, new settings or routes (`tc_core::webapi`), anything stored in the bucket. Leave this short or
`TBD` if you'd rather the agent propose it; it will write its proposal here before building.

## Acceptance criteria

Each one checkable, by a test or by looking. The change is done when every box is ticked.

- [ ] …
- [ ] Tests: … <!-- which unit / end-to-end / Kotlin / TUI tests prove it -->
- [ ] Docs: … <!-- docs/using/…, taskrc-support.md, architecture.md, README, pages.md address table -->
- [ ] Size: engine and Worker wasm measured before and after <!-- delete if neither is touched -->

## Open questions

- …

## Implementation notes

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.
