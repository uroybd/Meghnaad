# 0016: Share a task out of the Android app

- **Status:** in progress <!-- draft | ready | in progress | done | dropped -->
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
   - **Task info**: the text of `task <uuid> info`, as Taskwarrior prints it (its usual width), with no colour. It is for
     reading only: `task import` takes JSON, and refuses this text (`expected '{' or '[' at position 1`).
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

Agreed 2026-11-18; the Info text changed the same day (below), with the person's choice.

- **The button and the choice.** An `IconAction("share-2", …)` next to the link one in `DetailScreen.kt`, opening a
  small menu (a `DropdownMenu`, as `MoreMenu` has) with *Short text*, *Task JSON*, *Task info*.
- **The texts** (`Share.kt`).
  - Short text: a pure function in `commonMain`, `shortShare(row, offsetAt): String`, from the `Row` the panel already
    holds (description, project, priority, due, tags).
  - JSON: the engine's answer to `<uuid> export`, run as the app runs other reads (not shown in the console history),
    with its one object taken out of the array and written back out pretty-printed (`taskJsonOf`).
  - Info: the engine has no text for `info` (it answers a structured row for the screens), so `tc-core` gains
    `info_text` (`crates/tc-core/src/info_text.rs`), reached through `tc_core::helpers` as `info_text`: a port of
    `CmdInfo::execute` with libshared's `Table::render`, `extractLine` and number `format`, checked against the real
    `task` 3.5.0. It takes the task as `_rows` writes it, its `_history`, the open tasks it is blocked by and blocking
    (each `{id, description}`, which the app asks for with `( status:pending or status:waiting ) <uuids> _rows` and
    `… depends.has:<uuid> _rows`), the config (with the urgency coefficients), now, the zone and the width (0:
    `defaultwidth`). The urgency breakdown is the engine's own urgency terms (`urgency::breakdown`, sharing its terms
    with `urgency_with`). No new command.
- **Sending it.** `FileHost.share(subject, text)` (the host that already hands files to other apps), which
  `ActivityFiles` answers with `Intent.createChooser(Intent(ACTION_SEND).setType("text/plain")…)`. Where there is no
  host (the desktop preview), the text is copied instead.

## Acceptance criteria

- [ ] The info panel has a share button beside the link one, offering Short text, Task JSON and Task info.
- [x] Short text is the description, then a line of `project:` `priority:` `due:` `+tag`, leaving out the ones the task
      lacks (and the line when it lacks all four); shared back in (0015), the quick add's preview shows the same
      project, priority, due and tags.
- [ ] Task JSON is the object `task <uuid> export` gives for that task, without the array; shared into Meghnaad (0015)
      it opens the form with the same fields; `task import` of it with the real `task` 3.5.0 works.
- [x] Task info is the plain text of `task <uuid> info`, with no escape codes.
- [ ] Each opens Android's share sheet with the text, and the description as the subject.
- [ ] Tests: `shortShare` in `androidUnitTest` (all fields, none, tags only, due with and without a time, a long
      description); both flavours read back by 0015's `sharedAdd`.
- [x] Docs: `docs/android.md` (sharing a task).

## Open questions

None left.

Decided (2026-11-18, by the person):

- Short text's second line is Taskwarrior syntax, so it imports again through the quick add.
- Short text includes `due`.
- Task JSON is a bare object. Checked with the real `task` 3.5.0 on scratch data: `task import` of it adds the task
  with its fields and the same uuid (so importing where that task exists updates it).
- Task info is the text exactly as `task <uuid> info` prints it, at Taskwarrior's usual width, colour off.
- A description that reads as syntax (`Call +bob about due:`) is left as it is: shared back in, the quick add's preview
  shows what it was read as, as 0015 does for any text.
- The design above is agreed.
- The menu is always in the same order: Short text, Task JSON, Task info. Nothing is remembered.

## Implementation notes

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.

### 2026-11-18

Built.

- **Engine** (`crates/tc-core`): `info_text.rs` (new), the `info_text` helper in `helpers.rs`, `urgency::breakdown` and
  `Term` (the built-in terms and the user's coefficients split out of `urgency_with`, which still sums them in
  `Task::urgency_c`'s order, so no urgency changes), `calc::fmt_g` (C++'s `%g` at any precision; `fmt_real` is
  `fmt_g(x, 6)`), and `Facts::rtype` now written in a row when the task has one (`info` shows *Recurrence type*).
  The row and the history are read from their JSON by hand, not with `serde(Deserialize)`: the derived readers cost
  about 6 KB of the engine's gzipped size.
- **Tests**: `tests/info_text.rs` compares the helper's text with what the real `task` 3.5.0 printed
  (`tests/fixtures/info_text/`, unedited, made on a scratch taskrc with a duration, a date and a label-less UDA and a
  tag coefficient): a task with every part (annotation, wait, scheduled, start, blocked by and blocking, UDAs, a long
  description wrapping, the urgency breakdown, the history), a blocked and a blocking task, a recurring parent and an
  instance. `cli_exec.rs` feeds the engine's own `info` and `_history` to it. Kotlin: `ShareTest` (commonTest: the
  short text, read back through the add preview, and the JSON) and `ShareStoreTest` (androidUnitTest, the real engine:
  all three forms, blocked by the open dependency only).
- **App**: `Share.kt` (new), the menu in `DetailScreen.kt`, `FileHost.share` and `ActivityFiles.share`.
- **Sizes** (release, debug names dropped as the deploy does): the browser engine 868,682 → 883,646 bytes gzipped
  (+15.0 KB, 1.7%), most of it `info`'s layout. The Worker links none of it: 2,728,848 bytes before and after, gzipped
  821,328 → 821,350.
- **Found on the way, not fixed** (engine parity, for the gaps list): the engine computes no `LATEST` virtual tag, so
  `info`'s *Virtual tags* lack it where `task` shows it; the history of a task added in one command lists its
  attributes in a different order from `task`'s (`Entry`, `Status` first); the engine's working-set IDs for tasks added
  back to back are not always in the order they were added (seen in `ShareStoreTest`, now written not to depend on it).
- **Limits**: the text uses one zone offset (the phone's now) for every date, so a date across a daylight-saving change
  is an hour off; wide characters (CJK, emoji) count as one column where `task` counts two.
- **Left**: seen by a person on a phone (the button, the menu, the share sheet and its subject); the JSON read back by
  0015 once it is built; the tests criterion names `androidUnitTest` for `shortShare`, which is in `commonTest` (it
  needs no engine), and 0015's `sharedAdd`, which does not exist yet.
