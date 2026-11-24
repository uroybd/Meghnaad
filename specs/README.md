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

| Status        | Meaning                                                           |
| ------------- | ----------------------------------------------------------------- |
| `draft`       | Being written; questions open. Not to be built yet.               |
| `ready`       | Agreed. Can be built as written.                                  |
| `in progress` | Being built. The **Implementation notes** say how far it has got. |
| `done`        | Built, checked against every acceptance criterion, docs updated.  |
| `dropped`     | Not doing it. The spec stays, with a line saying why.             |

A spec is never deleted or renumbered: a later one that replaces it says `Supersedes 0003` and the old one is set to
`dropped` with a link forward.

## Index

| No.  | Spec                                                                                                                   | Status      | Layer                 | Front ends                                   | Left                                                                                              |
| ---- | ---------------------------------------------------------------------------------------------------------------------- | ----------- | --------------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| 0001 | [Field transformation](0001-field-transform.md)                                                                        | in progress | 2 (UI)                | all                                          | The terminal's info view, which 0006 replaces                                                     |
| 0002 | [CLI export](0002-meghnaad-cli-export.md)                                                                              | done        | 2 (UI)                | all                                          | Checked by uroybd, 2026-11-20                                                                     |
| 0003 | [Custom reminder](0003-custom-reminder.md)                                                                             | in progress | 2 (UI)                | all                                          | Seen by uroybd: the Android alarm with the app closed, its Reminders dialog; a Linux notification |
| 0004 | [Export filter and sort](0004-export-filter-sort.md)                                                                   | dropped     | 2 (UI)                | all                                          |                                                                                                   |
| 0005 | [Stack a chart by a field](0005-chart-stack-by.md)                                                                     | done        | 2 (UI)                | all                                          |                                                                                                   |
| 0006 | [The terminal app's own task info](0006-tui-task-info.md)                                                              | in progress | 2 (UI), one helper    | terminal (web, Android: the time log helper) | Seen by uroybd: a link in the info view opening                                                   |
| 0007 | [Select several tasks in the terminal app](0007-tui-bulk-selection.md)                                                 | done        | 2 (UI)                | terminal                                     | Built 2026-11-17                                                                                  |
| 0008 | [Group separators in the terminal app](0008-tui-group-separators.md)                                                   | done        | 2 (UI)                | terminal                                     | Built 2026-11-17                                                                                  |
| 0009 | [settings.json, editable, pastable and kept whole](0009-settings-json-kept-whole.md)                                   | done        | 2 (UI)                | all                                          | Built 2026-11-17                                                                                  |
| 0010 | [Short uuid for a missing ID, as an option](0010-id-fallback-opt-in.md)                                                | done        | 2 (UI), one helper    | all                                          | Built 2026-11-17                                                                                  |
| 0011 | [Git repository as the sync backend](0011-git-sync-backend.md)                                                         | done        | both                  | web, Android                                 | Checked by uroybd, 2026-11-20                                                                     |
| 0012 | [Hook secrets bound to their hosts](0012-hook-secrets-bound-to-hosts.md)                                               | in progress | both                  | all                                          | Seen by uroybd: the secrets' hosts on the Android screen                                          |
| 0013 | [The web app as static files, on a git branch](0013-static-web-app.md)                                                 | in progress | both                  | web                                          | Left: the template on real Pages, browser engine tests for the git path, `docs/using/`            |
| 0014 | [Sync without holding up the screen](0014-sync-without-blocking.md)                                                    | done        | both                  | web, Android                                 | Checked by uroybd, 2026-11-20                                                                     |
| 0015 | [Share text into the Android app to add a task](0015-android-share-into.md)                                            | draft       | 2 (UI)                | Android                                      | Being built                                                                                       |
| 0016 | [Share a task out of the Android app](0016-android-share-task.md)                                                      | in progress | 2 (UI), one helper    | Android                                      | Built; seen by uroybd on a phone; the JSON read back waits on 0015                                |
| 0017 | [Reminders keep coming while the static web app is locked](0017-reminders-while-locked.md)                             | in progress | 2 (UI)                | web (static)                                 | Seen by uroybd in a browser                                                                       |
| 0018 | [Trust this device (the static web app)](0018-trust-this-device.md)                                                    | done        | 2 (UI)                | web (static)                                 | Done 2026-11-19                                                                                   |
| 0019 | [Unlock the static web app with a passkey](0019-passkey-unlock.md)                                                     | in progress | 2 (UI)                | web (static)                                 | Checked by uroybd: Firefox with Bitwarden, a Chromium browser                                     |
| 0020 | [Keys run functions, with vim-like motions](0020-key-functions.md)                                                     | done        | 2 (UI), shared keymap | terminal, web, phone                         | Done 2026-11-19; settings.json versioned (2), commands described in keymaps/commands.json         |
| 0021 | [The taskrc tab as a small workspace](0021-taskrc-workspace.md)                                                        | done        | 2 (UI)                | web, Android                                 | Done 2026-11-19                                                                                   |
| 0022 | [A PIN on a trusted device](0022-pin-lock.md)                                                                          | in progress | 2 (UI)                | web (static)                                 | Built 2026-11-19; to check by uroybd: the PIN screen in a browser                                 |
| 0023 | [The web app's settings reads in one place, its largest files split, and pages loaded on demand](0023-web-refactor.md) | in progress | 2 (UI)                | web                                          | All four steps built; left: a throttled check by uroybd, step 1's size             |
| 0024 | [Android performance: a screen never waits on a sync](0024-android-boards-speed.md)                                    | in progress | host                  | Android                                      | Fixed and measured on the emulator; boards and a phone left                        |
| 0025 | [Commands are what keys run; actions are presets of them](0025-commands-are-the-base.md)                               | done        | 2 (UI), shared keymap | web, Android, terminal                       | Contextual names; sort suffixes + - -- = /                                                        |
| 0026 | [Kanban](0026-kanban.md)                                                                                               | done        | 2 (UI)                | all                                          | Built on web, Android and terminal; not committed                                                 |
| 0027 | [Settings of one device, over everyone's](0027-device-settings.md)                                                     | done        | 2 (UI)                | web, Android                                 | Shipped in 0.8.32                                                                                 |
| 0028 | [This device as one tab of Settings](0028-this-device-tab.md) | done | 2 (UI) | web, Android | Replaces 0027's switch |
| 0029 | [Meghnaad for AI agents: a skill, `meghnaad skill`, `--dry-run`](0029-ai-skill.md) | done | 2 (UI) | terminal | Done 2026-11-24; plugin install from GitHub to check after a push |
| 0030 | [Conditional transformations](0030-conditional-transformations.md) | done | 2 (UI), shared transform helper | all | Built |
| 0031 | [Agents in the web app (WebMCP tools and llms.txt)](0031-agents-in-the-web-app.md) | done | 2 (UI) | web | Built; not tried in a browser with WebMCP on |
