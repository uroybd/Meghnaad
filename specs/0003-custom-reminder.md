# 0003: Custom Reminder

- **Status:** draft <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-16
- **Layer:**  2 (UI)
- **Front ends:** web, Android, terminal <!-- the ones this touches; say "all" or name the ones left out and why -->
- **Supersedes / related:** <!-- other specs, issues, commits; or none -->

## Why

Let's face it, there's no reminder in taskwarrior, really

## What

A custom reminder system based on an UDA.

1. It will work on an UDA named `remind_in`
2. Just like we handle enabling review feature, this will add the `uda` in a similar flow.
3. Default value for this uda is `default` which will fallback to `meghnaad` settings.
4. Value format:
    1. Can be single value: "10" (in minutes)
    2. Series: "10,20,30"
5: Execution:
    1. Android: Notification on appropriate time.
    2. Web: If tab is open, throw notification.
    3. CLI: When open setup the necessary notifications, similarly refresh or delete them as neccessary on sync and changes.

## Not doing

What is deliberately left out, so it isn't built by accident.

## Taskwarrior parity

Layer 2 only (delete otherwise). What the real `task` 3.5.0 does with the same command line, and where in its C++ that
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
