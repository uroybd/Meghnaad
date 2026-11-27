# 0041: Tracked time in `meghnaad export` and in hooks

- **Status:** done <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-27
- **Layer:** 2 (UI), with the hooks' Lua API (ours, not Taskwarrior's)
- **Front ends:** all: `meghnaad export`, the Export dialog (web, phone, terminal), and hooks on every host (Worker,
  static web, phone, `meghnaad-hook` under the real `task`)
- **Supersedes / related:** task f109d79f ("Tracked Time details in export and hooks"); the time log
  (`tc_core::helpers::time_log`, the info panel's Time log); 0029 (`meghnaad export`)

## Why

With `journal.time=on`, a task's info shows its tracked time as a table: each session and a total. Nothing else can
read that. A hook that bills hours, warns when a task runs past its estimate or writes the total into a UDA has to pair
the "Started task" and "Stopped task" notes itself. A script reading `meghnaad export` has to do the same, and both
will get it subtly wrong (the running session, a stop with no start, notes with changed texts). The engine already
pairs them one way for every app. This hands that result to hooks and to the export.

## What

One shape, everywhere, called `time_tracking` (lower case, words joined by `_`, as Taskwarrior names `iter_type` and
`iter_count`):

```json
"time_tracking": {
  "total": 1323482,
  "sessions": [
    { "start": "20261124T090000Z", "end": "20261124T101500Z", "duration": 4500, "description": "wrote the tests\nfixed the parser" },
    { "start": "20261127T080000Z", "end": null, "duration": 1200, "description": "" }
  ]
}
```

- Dates (`start`, `end`) are written as Taskwarrior's `export` writes `entry` or `due` (`20261124T090000Z`, UTC);
  durations (`total`, `duration`) are seconds. The sessions are the ones the info
  panel's Time log shows, in the same order, the running one last, with `end` null and its `duration` counted to now.
- A session's `description` is the annotations written during it (between its start note and its stop note; for the
  running one, since it began), in order, one to a line; empty when there are none. The `journal.time` notes stay what
  makes a session (no new keywords), and the info panel's Time log shows each session's description under it, on every
  front end.
- Every task has it, so totals can be summed over tasks without a check: one with no tracked time, or any task while
  `journal.time` is off, has `{ "total": 0, "sessions": [] }`. A session of 0 seconds is kept, with `duration` 0.

**1. Hooks.** Each task a hook is given has `task.time_tracking`, in Lua's own types: a table with `total` (a duration,
as `d2 - d1` gives one, so `:in_hours()` works) and `sessions`, a list of `{start, finish, duration, description}` where `start`
and `finish` are dates, `duration` a duration and `description` text. `finish` is `nil` for the running session. `end` is a Lua keyword, so it
is `finish` here, as `task.finished` stands for `end`. A task with no tracked time has a zero `total` and no sessions.
It is read-only, like `uuid`: changing it is an error. In `on_modify`, `new.time_tracking` is worked out from the new
task, so a `start` or `stop` shows in it.

```lua
-- Keep a `spent` UDA in hours, from the time log.
function on_modify(old, new)
  new.spent = string.format("%.1f", new.time_tracking.total:in_hours())
end
```

**2. `meghnaad export`.** Today, with none of its options, it is `task <words> export` exactly; any option makes it
reshape that output (JSON with fewer fields, CSV, TSV). That stays. What changes:

- The reshaped output (any option given) has `time_tracking` on each task, after Taskwarrior's own fields.
- A new option, `--rich`, asks for the reshaped output when no other option is given:
  `meghnaad export --rich project:Work` is Taskwarrior's JSON plus `time_tracking`.
- It is a field like the others, so the options that choose fields leave it out: `-F table` (no report column shows it)
  and a list such as `-F description,due`. `-F all` keeps it, and naming it (`-F uuid,time_tracking`) asks for it.
- A new option, `--include-time-tracking`, adds it to whatever `-F` chose: `-F table --include-time-tracking`.
- In CSV and TSV its cell is the total, in seconds; the sessions are left out.
- `meghnaad export --help` says so.

**3. The Export dialog** (over a table, on the web, the phone and the terminal) makes its file with the same reshaping,
so `time_tracking` is one of its fields: in every field, offered to tick among the chosen ones, with the same CSV and
TSV cell.

## Not doing

- No change to `export` itself, in the engine or the real `task`: `task export` and the browser's `export` stay
  Taskwarrior's, with no `time_tracking`. `_rows` gains nothing either.
- No writing time through `time_tracking`, in hooks or anywhere: start, stop and their notes stay the way to track
  time.
- No Timewarrior, and no other source of sessions than the `journal.time` notes.
- No new column for reports.

## Design

Proposed; to confirm before building.

- **The shape, once.** `tc_core::helpers::time_log` already returns `TimeLog { sessions, total }` from a task as
  `export` writes it. Add a serialiser for the shape above (dates as `export` writes them, `tc_core::export::iso`, a session's `seconds` as
  `duration`, `end` null for the running one) beside it, so hooks and the export make the same object.
- **Hooks.** `tc_core::lua_hooks` builds the task table from `task_to_json`: add `time_tracking` there (worked out with
  the config's `journal` markers and the clock the host passes, never a real clock), turn it into Lua dates and
  durations in `lua/prelude.lua`, and have `task_from_json` refuse a change to it as it does for `uuid`. The hosts
  (Worker, `crates/browser-hooks`, `crates/android`) need nothing new. `meghnaad-hook` already reads the taskrc from
  `task`'s `rc:` argument, so it has the markers too. Tests in `crates/tc-core/tests/lua_hooks.rs`.
- **Export.** `tc_core::table_export` reshapes `export`'s JSON: add `time_tracking` to each task (with the taskrc's
  journal markers and the clock's now) before the fields are chosen, so `-F` keeps or drops it like any field, and
  `Choice` gains a switch for `--include-time-tracking`; a CSV or TSV cell is its `total`. `crates/tui/src/export.rs`
  gains `--rich` (no options but that one is the reshaped JSON with every field) and `--include-time-tracking`. The
  Export dialogs (web, Android, terminal) get the field through the same helper and offer it where they offer fields.
  The engine's `export` is untouched.
- **Docs.** `docs/using/hooks.md` (the task table), `docs/using/time-tracking.md`, the Export dialog's guide, the
  `meghnaad export` help and its page, and the skill (`skills/meghnaad`) if it lists the options.

## Acceptance criteria

- [x] A hook sees `task.time_tracking` with `total` and `sessions` matching the info panel's Time log, as Lua dates and
      durations, a running session with `finish` nil, and a zero `total` with no sessions without tracked time or with
      `journal.time` off.
- [x] Changing `time_tracking` in a hook is an error, as `uuid` is.
- [x] `meghnaad-hook` under the real `task` gives the same `time_tracking` as the engine for the same task.
- [x] `meghnaad export --rich <filter>` is Taskwarrior's JSON for those tasks plus `time_tracking`; `meghnaad export
      <filter>` is still `task <filter> export` byte for byte.
- [x] Every task has `time_tracking`: `start` and `end` as `export` writes dates (`end` null while running), `total` and
      `duration` seconds; none tracked is `{ "total": 0, "sessions": [] }`.
- [x] Each session has a `description`, the annotations written during it, one to a line; the Time log shows it on the
      web, the phone and the terminal.
- [x] With any other option, `time_tracking` is a field: kept by `-F all`, by naming it and by
      `--include-time-tracking`, left out otherwise. A CSV or TSV cell is the total.
- [x] The Export dialog on the web, the phone and the terminal offers `time_tracking` and writes it as above.
- [x] Tests: `crates/tc-core/tests/lua_hooks.rs` (the table, read-only, the running session); `table_export` unit tests
      (the field, choosing it, the CSV cell); `model` (descriptions); the TUI's export tests (`--rich`,
      `--include-time-tracking`, untouched plain `export`, against the real `task` 3.6.0); the Export dialogs' tests on
      web and Kotlin; `meghnaad-hook`'s `convert` test.
- [x] Docs: `docs/using/hooks.md`, `docs/using/time-tracking.md`, `docs/using/pages.md` (the Export dialog),
      `docs/terminal.md` (`meghnaad export`), the README, the skill and `meghnaad export --help`.
- [x] Size: the Worker's wasm measured before and after.

## Open questions

- None left. Answered by uroybd on 2026-11-27: a CSV or TSV cell is the total; hooks get Lua dates and durations, the
  export dates as Taskwarrior writes them and seconds for durations; `-F table` leaves it out, and `--include-time-tracking` adds it; the Export dialog gets it too; every
  task has it (a zero total when untracked), and a running session's `end` is null.

## Implementation notes

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.

2026-11-27: Built.

- The shape is made once, `Facts::time_tracking` (`crates/tc-core/src/model.rs`), with a date writer: numbers for hooks,
  `export::iso` for the export. `Session` gained `description` (the notes between its start and stop notes), so the
  Time log helper (`helpers::time_log`) and the three Time log tables show it too.
- Hooks: `hooks::task_to_json` takes the `journal` notes and `now`. The hosts that build the task's JSON pass them:
  `LuaHooks::with_journal` (phone, tests), `RemoteHooks` (browser engine, both flavours, from its taskrc),
  `meghnaad-hook` (`convert::table_of`, from the taskrc `task` names). `lua/prelude.lua` turns it into dates and
  durations and keeps it beside the task (`TRACKED`), so setting `task.time_tracking` reaches `__newindex` and fails.
- Export: `table_export::with_time_tracking` adds it to each task (annotations and `start` read back from `export`'s
  dates, `export::from_iso`); `Choice.time_tracking` is `--include-time-tracking`. serde_json here sorts an object's
  keys, so with every field the file keeps each task's line exactly as `export` wrote it and appends `time_tracking`
  (`to_json_all`), whose keys are written in the order above. `export_file` takes `now` (web, Kotlin and the terminal
  pass their clock); `export_fields` lists `time_tracking`.
- `meghnaad export`: `--rich` and `--include-time-tracking`; with no option it is still `task export` untouched.
- Departure from the first draft, asked for by uroybd while building: each session has a `description`. The
  `[start]`/`[stop]` keywords proposed at the same time were dropped by uroybd in favour of the annotations written
  between the `journal.time` notes.
- Worker (release, gzip -9): 780,353 bytes before, 780,778 after (+425, the Lua library's new text).

