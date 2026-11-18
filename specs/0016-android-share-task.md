# 0016: Share a task out of the Android app

- **Status:** draft <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-18
- **Layer:** 2 (UI)
- **Front ends:** Android (web and terminal left out; see **Not doing**)
- **Supersedes / related:** 0015 (share text into the Android app), which reads the JSON flavour back

## Why

The task info panel can copy a link to a task (`meghnaad://task/…`), which only means something to another Meghnaad. To
send a task to a person in a chat, to a notes app, or to another Taskwarrior user, it has to be text they can read or
use, sent through Android's share sheet.

## What

1. **A share button** on the task info panel (`DetailScreen.kt`), beside *Copy a link to this task*, with Lucide's
   `share-2` icon.
2. Tapping it offers three flavours, then opens Android's share sheet with the chosen one as plain text:
   - **Short text**, for a chat: the description, then a line in Taskwarrior syntax with the project, priority, due
     and tags the task has, so it reads well and shared back into Meghnaad (0015) the quick add fills the same fields.
     For example:

     ```
     Buy milk
     project:home priority:H due:2026-11-20 +errand +shop
     ```

     `due` is written as an ISO date in the phone's zone (`2026-11-20`, or `2026-11-20T17:00` when it has a time),
     which the engine reads whatever the `dateformat`. A task with none of the four is the description alone.
   - **Task JSON**: Taskwarrior's own JSON for the task, the object `task <uuid> export` prints, as a bare object
     (not the array of one around it). Another Meghnaad reads it back with 0015; another Taskwarrior user can
     `task import` it.
   - **Task info**: the text of `task <uuid> info`, as the engine prints it, with no colour.
3. The share sheet's subject (`EXTRA_SUBJECT`, used by mail apps) is the task's description.
4. It works for any task the panel shows: pending, waiting, completed, deleted, recurring.

## Not doing

- Sharing several tasks (a bulk action), or a report.
- Sharing as a file (`.json` or `.txt` attachment, `EXTRA_STREAM`): text only.
- Choosing which fields the short text shows, or a template for it.
- A share button on the web app (the browser's `navigator.share` exists, but the web has its own copy/export) or the
  terminal app.
- A new engine command: JSON and info come from `export` and `info`, which exist.

## Design

Proposed, not agreed yet.

- **The button and the choice.** An `IconAction("share-2", …)` next to the link one in `DetailScreen.kt`, opening a
  small menu (the app's existing dropdown, as `MoreMenu`) with *Short text*, *Task JSON*, *Task info*.
- **The texts.**
  - Short text: a pure function in `commonMain`, `shortShare(row, offsetAt): String`, from the `Row` the panel already
    holds (description, project, priority, due, tags), tested in `androidUnitTest`.
  - JSON: the engine's answer to `<uuid> export`, run as the app runs other reads (not shown in the console history),
    with its one object taken out of the array and written back out pretty-printed.
  - Info: the engine's answer to `rc.color=off <uuid> info` (or however the app turns colour off for a read), as text.
- **Sending it.** An `expect`/`actual` (or a `Store` hook set by `MainActivity`, as the clipboard and notifications are)
  that starts `Intent.createChooser(Intent(ACTION_SEND).setType("text/plain").putExtra(EXTRA_TEXT, …).putExtra(EXTRA_SUBJECT, …))`.
  The desktop preview copies the text to the clipboard instead.
- **Nothing in `tc-core`**, so neither wasm grows.

## Acceptance criteria

- [ ] The info panel has a share button beside the link one, offering Short text, Task JSON and Task info.
- [ ] Short text is the description, then a line of `project:` `priority:` `due:` `+tag`, leaving out the ones the task
      lacks (and the line when it lacks all four); shared back in (0015), the quick add's preview shows the same
      project, priority, due and tags.
- [ ] Task JSON is the object `task <uuid> export` gives for that task, without the array; shared into Meghnaad (0015)
      it opens the form with the same fields; `task import` of it with the real `task` 3.5.0 works.
- [ ] Task info is the plain text of `task <uuid> info`, with no escape codes.
- [ ] Each opens Android's share sheet with the text, and the description as the subject.
- [ ] Tests: `shortShare` in `androidUnitTest` (all fields, none, tags only, due with and without a time, a long
      description); both flavours read back by 0015's `sharedAdd`.
- [ ] Docs: `docs/android.md` (sharing a task).

## Open questions

- **Info's width.** `task info` wraps to the terminal width; on a phone in a chat, a narrow fixed width
  (`rc.defaultwidth=…`) or no wrapping?
- **Remember the last flavour** and offer it first, or always the same order?
- **A description that reads as syntax** (`Call +bob about due:`): shared back in, the quick add reads those words as
  attributes. Leave it to the preview, as 0015 does for any text, or quote such a description?

Decided (2026-11-18, by the person):

- Short text's second line is Taskwarrior syntax, so it imports again through the quick add.
- Short text includes `due`.
- Task JSON is a bare object.

## Implementation notes

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.
