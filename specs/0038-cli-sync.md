# 0038: `meghnaad --sync <line>`

- **Status:** done
- **Created:** 2026-11-27
- **Layer:** 2 (UI)
- **Front ends:** terminal (the command line from a shell)
- **Supersedes / related:** Taskwarrior task b0cf7ad6 ("Option to sync smartly in CLI", Meghnaad.TUI).

## Why

A one-off line from a shell (`meghnaad add …`) never synced, whatever the app's `sync` setting; the task: "behave like
syncing on start, running the rest of the command, then sync again. first will be sync, last will be async as is now.
if the command doesn't modify anything the last sync shouldn't run".

## What

`meghnaad --sync <line>`: `task sync`, waited for; the line, as `meghnaad <line>` runs it; then, when a statement that ran
changes tasks (the app's own list: `add`, `modify`, `done`, …), `task sync` handed to the background as the app's sync at
close is, so the prompt is back at once. A failed sync before is said on standard error and the line runs anyway; a
failed one after is kept for the app to say when it next opens. `meghnaad --sync` alone is a usage error (2).

## Not doing

- `--sync` with `export` or `--dry-run`.

## Design

`run::run(task, args, sync)`; the check is `chain::changes_tasks` (moved from the app, which now uses it too) over
`task _commands`; the sync after is `sync::hand_over`.

## Acceptance criteria

- [x] Synced before, and after only when it changes tasks.
- [x] Tests: `tests/cli_sync.rs`, over a stand-in `task`.
- [x] Docs: `docs/terminal.md`, the skill.
