# Specs

A spec says what a change is for and how we will know it is done, before any code is written. Write one when a change
is bigger than a sentence: it crosses front ends, touches the engine or the sync format, adds a page, or needs a
decision you want written down. Small fixes don't need one.

These files are for working on Meghnaad, not for people using it, so they live outside `docs/` and the documentation
site never sees them. What a finished feature does belongs in the guides (`docs/using/`, the reference pages); a spec
links to them once they exist.

## How to write one

1. Copy [`TEMPLATE.md`](TEMPLATE.md) to `NNNN-short-name.md`, taking the next free number (`0001-board-filters.md`).
2. Fill in what you know. Leave a section as `TBD` rather than guessing, and put anything undecided under
   **Open questions**.
3. Add a row to the table below.
4. Set **Status** to `ready` when you want it built. Then point the agent at it: "implement specs/0001-board-filters.md".

## Status

| Status        | Meaning                                                               |
| ------------- | --------------------------------------------------------------------- |
| `draft`       | Being written; questions open. Not to be built yet.                   |
| `ready`       | Agreed. Can be built as written.                                      |
| `in progress` | Being built. The **Implementation notes** say how far it has got.     |
| `done`        | Built, checked against every acceptance criterion, docs updated.      |
| `dropped`     | Not doing it. The spec stays, with a line saying why.                 |

A spec is never deleted or renumbered: a later one that replaces it says `Supersedes 0003` and the old one is set to
`dropped` with a link forward.

## Index

| No. | Spec | Status | Layer | Front ends | Left |
| --- | ---- | ------ | ----- | ---------- | ---- |
| 0001 | [Field transformation](0001-field-transform.md) | in progress | 2 (UI) | all | The terminal's info view, which 0006 replaces |
| 0002 | [CLI export](0002-meghnaad-cli-export.md) | in progress | 2 (UI) | all | Seen by a person: CSV dates and multiline cells in a spreadsheet; the Android dialog |
| 0003 | [Custom reminder](0003-custom-reminder.md) | in progress | 2 (UI) | all | Seen by a person: the Android alarm with the app closed, its Reminders dialog; a Linux notification |
| 0004 | [Export filter and sort](0004-export-filter-sort.md) | dropped | 2 (UI) | all | |
| 0005 | [Stack a chart by a field](0005-chart-stack-by.md) | in progress | 2 (UI) | all | Seen by a person: stacked charts on the web and Android (the terminal was checked) |
| 0006 | [The terminal app's own task info](0006-tui-task-info.md) | in progress | 2 (UI), one helper | terminal (web, Android: the time log helper) | Seen by a person: a link in the info view opening |
| 0007 | [Select several tasks in the terminal app](0007-tui-bulk-selection.md) | done | 2 (UI) | terminal | Built 2026-11-17 |
| 0008 | [Group separators in the terminal app](0008-tui-group-separators.md) | done | 2 (UI) | terminal | Built 2026-11-17 |
| 0009 | [settings.json, editable, pastable and kept whole](0009-settings-json-kept-whole.md) | done | 2 (UI) | all | Built 2026-11-17 |
| 0010 | [Short uuid for a missing ID, as an option](0010-id-fallback-opt-in.md) | done | 2 (UI), one helper | all | Built 2026-11-17 |
| 0011 | [Git repository as the sync backend](0011-git-sync-backend.md) | in progress | both | web, Android | Everything: started 2026-11-17 |
