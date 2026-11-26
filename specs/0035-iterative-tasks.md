# 0035: Iterative tasks (Taskwarrior 3.6.0)

- **Status:** in progress
- **Created:** 2026-11-26
- **Layer:** both (the engine first; the front ends draw the new fields)
- **Front ends:** all (web, Android, terminal)
- **Supersedes / related:** Taskwarrior task 31bcbeca ("Support for Iterative Task", Meghnaad.TC). Taskwarrior PR
  #4210 (3.6.0); TaskChampion 3.2.0 ("Iterative tasks", TaskChampion PR #729). Lives beside recurrence
  (`crates/tc-core/src/recur.rs`, `docs/using/recurring.md`) and does not replace it.

## Why

Taskwarrior 3.6.0 adds iterative tasks as an experimental feature. Upstream says the database format is stable and that
iterative tasks are meant to replace recurrence in a later release. An iterative task is a single task, not a template
with instances. When it is completed, a new task takes its place with its dates moved on to the next interval. That
removes most of what makes recurrence hard here: masks, `recurrence.limit`, duplicate instances from two replicas,
"modify all pending recurrences?". The interval can also be an RRULE or an English phrase ("every last friday of the
month"), which `recur:` cannot express.

Meghnaad today knows nothing of it. Its engine is pinned to TaskChampion 3.1, where `iterative` is `Status::Unknown`. A
task made by a real `task` 3.6.0 and synced into the web app or the phone is therefore neither pending nor completed.
It drops out of every report, and `done` cannot complete it (to confirm, see Open questions). The terminal app checks
for `task` 3.5.0. Anyone who starts using the feature on the desktop loses those tasks everywhere else.

## What

From the outside, everything follows `man task-iterative` (3.6.0). The parts a person meets:

- **Creating one.** `add Pay rent iter:monthly iter_type:fixed due:2026-12-20`, or add `iter` and `iter_type` to a
  pending or waiting task with `modify`. Both are needed, plus at least one of `due`, `scheduled` or `wait`. The task is
  stored with `status:iterative` and `iter_count:1`.
- **`iter`** accepts four forms, tried in this order:
  1. a raw RRULE (`FREQ=WEEKLY;BYDAY=MO,TH`);
  2. a Taskwarrior duration (`daily`, `weekdays`, `3wk`, `fortnight`, `quarterly`, `2year`);
  3. an ISO-8601 duration (`P2W`, `P3D`, `PT12H`);
  4. English (`"every other day"`, `"monthly on the third friday"`, `"weekly on thursday three times"`,
     `"every day until March 6th, 2027"`).
- **`iter_type`** is `fixed`, `fixed+` or `chained`, or a short form: `fx`; `f+` or `fp`; `ch`. The value is stored as
  typed, so `fx` stays `fx`.
- **Completing it** (`done`, a card's tick, the info view) creates the next task in the series. This happens unless an
  RRULE's `COUNT` or `UNTIL` has run out, which ends the series.
- **Reports and filters.** An iterative task reads like a pending task:
  - it appears in `next`, `list` and the other reports (their default filters are `+PENDING -WAITING`);
  - `+PENDING` matches it unless it is waiting, and a future `wait` hides it like any waiting task;
  - `status:iterative` and `+ITERATIVE` match it, while `status:pending` does not;
  - it can be overdue, blocked and blocking, and it has urgency like a pending task.
- **Ending a series early** is done by deleting the task, or by clearing `iter` (`modify iter:`), which asks first
  ("End the iterative series for task N 'x'?") when `confirmation` is on. The task becomes pending, without
  `iter_type` and `iter_count`.
- **New fields everywhere a task's fields are drawn.**
  - Labels: *Iter*, *Iter type* and *Iter count*, as `task info` names them.
  - The add and edit forms and bulk edit get `iter` and `iter_type` next to Repeat (`recur`).
  - The info view shows all three.
  - `iter_count` is read-only.
  - Columns `iter`, `iter_type` and `iter_count` can be used in reports.
- **The terminal app** accepts `task` 3.6.0 as its backend. Iterative tasks need it there, since the terminal app runs
  the real `task`.

## Not doing

- Turning recurring tasks into iterative ones, or iterative into recurring. That stays the person's choice, as it is in
  `task`.
- Removing or deprecating recurrence. Upstream plans to replace it; we follow when Taskwarrior does.
- Settings of our own (a default `iter_type`, a `recurrence.indicator` for iterative tasks). `task` 3.6.0 has none.
  `iter` takes no part in `recurrence.indicator`, `color.recurring` or `recurrence.limit`.
- A picker or builder for RRULEs in the forms. The field is text with a hint, as `recur` is.
- Showing the series' history (earlier tasks with the same description). No stored field links them, so a series
  cannot be read back.
- Moving the rest of the engine from 3.5.0 to 3.6.0 parity (burndown widths, `projects` nesting, the OR in contexts).
  That is its own change. See Open questions.

## Taskwarrior parity

Checked with the real `task` 3.6.0 (Homebrew) on a scratch `TASKRC` and data directory, 2026-11-26, local zone UTC+6.
The code was read from Taskwarrior v3.6.0 and the published `taskchampion-3.2.0` crate.

**Where it lives.**

- Taskwarrior's C++ does validation, status, filters and display: `Task.cpp` (`is_open`, `is_pending`, `is_waiting`,
  `checkIterative`), `CmdModify.cpp` (`checkConsistency`), `CmdDuplicate.cpp`, `ColIter*.cpp` and `CmdInfo.cpp`.
- Everything that computes a date is in TaskChampion 3.2.0, behind the `iterative-tasks` cargo feature:
  - `src/task/iter.rs` parses `iter`;
  - `src/task/task.rs` holds `set_iterative_status`, `set_iterative_completed` and `spawn_successor`;
  - `src/task/time.rs` holds `local_tz`.
- It depends on `rrule` 0.14 (with `chrono-tz`) and `text2rrule` =0.1.2 for the English forms.

**Storage.**

- An iterative task is an ordinary task map: `status: "iterative"`, `iter` (as typed), `iter_type` (as typed),
  `iter_count: "1"`.
- There is no template, no `parent`, no `mask` and no `imask`.
- `Status::in_working_set` includes `Iterative`, so it gets an ID.
- `DependencyMap` counts it as pending.

```text
$ task add Pay rent iter:monthly iter_type:fixed due:2026-11-20
$ task export
{"id":1,"description":"Pay rent","due":"20261119T180000Z","iter":"monthly","iter_count":1,
 "iter_type":"fixed","status":"iterative",...}
```

**Completing.** `Task::set_status(Completed)` on an iterative task, in one operation group:

1. Compute the next anchor date (below). If the series is not over, create the successor.
2. Set the completed task's `status:completed` and `end`, and remove its `iter` and `iter_type`. It keeps
   `iter_count`.
3. Give the successor a **UUID v5** made from the completed task's UUID. The namespace is
   `ITERATIVE_NAMESPACE = v5(NAMESPACE_URL, "taskchampion-iterative-task")` =
   `6ab813ab-3ff5-56f4-820e-31ada24844af`, and the name is the old UUID's 16 bytes:
   `Uuid::new_v5(&ITERATIVE_NAMESPACE, old.as_bytes())`. Two replicas that complete the same task therefore make the
   same successor, and syncing merges them.
4. The successor copies every property except the following, so it does not carry annotations, dependencies or
   `start`:
   - `status`, `modified`, `end`, `start`, `entry`, `iter_count`, `due`, `scheduled`, `wait`, `until`;
   - every `dep_*` and `annotation_*`.

   It then gets `status:iterative`, `iter_count` = old + 1, `entry` = now, and each of `due`, `scheduled`, `wait` and
   `until` that was set, shifted by the same local wall-clock delta. Where the local time does not exist (spring
   forward), that date uses the raw UTC delta. Where it is ambiguous, the earlier time is used.

**The next date.** The anchor is the highest of `due` > `scheduled` > `wait`, in the local zone. The RRULE is parsed
with any `COUNT` removed, and "after" is strict.

- `fixed`: the first occurrence after the old anchor.
- `fixed+`: the first occurrence after the later of now and the old anchor. Missed intervals are skipped.
- `chained`: the schedule is anchored at *now* (the moment of completion, so the time of day is the completion's),
  and the first occurrence is taken in a later period than now's. The period depends on FREQ: day for DAILY, ISO week
  for WEEKLY, month for MONTHLY, year for YEARLY. If the rule has any `BY*` part, the first occurrence after now is
  taken instead.
- `COUNT`: no successor once `iter_count + 1 > COUNT`.
- `UNTIL` in the past: there is no next occurrence and no successor. This is not an error.

```text
$ task 1 done; task 2 done; task 3 done; task 4 done      (on 2026-11-26 23:27 local, Thursday)
Pay rent  fixed   monthly        due 2026-11-20 -> 2026-12-20         iter_count 2
Bins      fixed+  weekly         due 2026-11-06 -> 2026-11-27 (two missed weeks skipped), scheduled moved alike
Haircut   chained 6wk            scheduled 2026-11-01 -> 2027-01-07 23:27:58 (completion time + 6 weeks)
Standup   fx      every weekday  due 2026-11-27 -> 2026-11-30 (Friday -> Monday)
Physio    fixed   "weekly on thursday three times": after the 3rd is completed, no successor
```

**Errors and questions** (exact text):

```text
Usage Error: Iterative tasks require a 'due', 'scheduled' or 'wait' date.
Usage Error: Iterative tasks require an 'iter_type' of fixed, fixed+ or chained.
A task cannot be both recurring and iterative.                (add with recur and iter; modify recur on one)
You cannot change the status of an iterative task. Clear iter to end the series instead.
You cannot remove the iteration type from an iterative task.
You cannot remove the last of the due, scheduled and wait dates from an iterative task.
The 'iter_type' attribute does not allow a value of 'bogus'.
End the iterative series for task 1 'Pay rent'? (yes/no)     (modify iter: with confirmation on; "no" -> Task not modified.)
Task 5 'Physio' is neither pending nor waiting.              (done on a completed one, message unchanged from 3.5)
```

TaskChampion's own messages for a bad `iter` are `Could not parse iteration value "…"` and `Couldn't validate rrule:
…`.

**Other commands.**

- `duplicate` of an iterative task gives a new iterative task at `iter_count:1`, keeping `iter` and `iter_type`.
- `task info` shows the rows *Iter*, *Iter type* and *Iter count*, and the virtual tag `ITERATIVE` beside `PENDING`.
- `stats` has an `Iterative` row.
- `import` reads a numeric `iter_count` as an integer.
- 3.6.0 has no colour rule and no urgency coefficient for iterative tasks (`rules.cpp` is unchanged).

**Odd behaviour we copy on purpose.**

- `iter_type` is stored as typed (`fx`, not `fixed`).
- A chained task's next date keeps the time of day it was completed, not midnight.
- The `done` message still says "neither pending nor waiting".
- Clearing `iter` also drops `iter_count`.

**The one place we must not copy TaskChampion.** `local_tz()` is `chrono::Local` natively, but **UTC on wasm32**
(`src/task/time.rs`). The browser engine would compute every next date in UTC. With a local zone of UTC+6, "every
weekday" from Friday 00:00 local (Thursday 18:00Z) lands on Friday 18:00Z, which is Saturday local. Upstream's man page
says the date "is computed in the local timezone of the replica that completes the task". For the web app, that zone
is the browser's.

## Design

A proposal; it needs a yes before building.

**Engine (`crates/tc-core`)**

- Bump `taskchampion` 3.1 → 3.2 in `tc-core`, `browser`, `android` and `worker`. Turn on `iterative-tasks` in
  `tc-core` (and so in `browser` and `android`). That gives `Status::Iterative`, the working set and the dependency map
  as upstream has them. The Worker runs no commands, so it keeps the feature off. Measure its size anyway: the version
  bump alone may move it.
- Add `crates/tc-core/src/iter.rs`, ported from TaskChampion 3.2.0's `task/iter.rs` and `set_iterative_completed` /
  `spawn_successor`, the same way `recur.rs` is ported from `recur.cpp`.
  - It is pure: it plans and `cli` applies.
  - It computes in the zone the host gives, not `local_tz()`.
  - TaskChampion's `iter` module is `pub(crate)`, so the parsing is ported too: the Taskwarrior shorthand table and the
    ISO-8601 reading. The RRULE and English forms use `rrule` and `text2rrule` =0.1.2 directly, so we accept exactly
    what `task` accepts.
  - `cli/write.rs` completes an iterative task by applying that plan through `TaskData`. It never calls
    `Task::set_status(Completed)`, which would run TaskChampion's UTC computation on wasm.
  - The successor UUID, the copied and dropped properties and the date shift are exactly as above. A test compares our
    plan with TaskChampion's own `set_status(Completed)` natively, over a table of cases (each type, DST transitions in
    a named zone, COUNT and UNTIL).
- **The zone.** `Clock` holds a fixed `tz_offset`, which is not enough: DST and `BYDAY` rules need a named zone.
  - Add an optional IANA zone name to the clock, as `chrono_tz::Tz`, which `rrule` already pulls in.
  - The page passes the browser's default zone, `Intl.DateTimeFormat().resolvedOptions().timeZone` (decided). It is
    read on every request, as the offset is today, so a laptop that changes zone is followed. The phone passes
    `ZoneId.systemDefault()`.
  - If no zone is given, or the name is unknown, use the fixed offset, through `Etc/GMT±H` where it fits, else UTC.
    Say so in `docs/using/recurring.md`.
- **Validation** is in `add`, `modify`, `import` and `duplicate`, with the messages above:
  - `iter` must parse;
  - `iter_type` must be one of the six spellings;
  - a date must be set;
  - a task cannot have both `recur` and `iter`;
  - no status change while `iter` is set;
  - clearing `iter` goes through the confirmation question as `recurrence.confirmation` questions do today (the
    review/answer path, `api.ts` "follow-up questions").

  `iter` and `iter_type` become built-in attributes. `iter_count` is read-only, as `task` refuses to set it.
- **Filters and reports** (`model.rs`, `filter.rs`, `report.rs`, `cli/dom.rs`):
  - `status:iterative` matches;
  - `+ITERATIVE` is a virtual tag, also listed for completion (`complete.rs`) and in `cli/mod.rs`;
  - `PENDING`, `READY`, `WAITING`, `OVERDUE`, `BLOCKED` and `BLOCKING` treat `iterative` like `pending` (`is_open` /
    `is_pending`);
  - urgency is as for pending;
  - the `iter`, `iter_type` and `iter_count` columns are added;
  - `stats` gets its row;
  - `info` gets its rows and the `ITERATIVE` tag.
- **Hooks.** `task_to_json` / `task_from_json` and `lua/prelude.lua` carry the three fields and `status:iterative`.
  Whether on-add runs for the successor is to be checked against `task` 3.6.0 (Open questions).
- **Size.** `rrule`, `chrono-tz` and `text2rrule` (with `regex`) land in the browser engine and the phone library. Report
  the browser engine's size before and after. The Worker's must not move beyond what the version bump needs.

**Front ends**

- **Web** (`web/src/lib`): `types.ts` gets the fields. The add and edit forms and `bulk.ts` get *Iter* (text; hint
  `monthly, every weekday, FREQ=…`) and *Iter type* (a choice of fixed, fixed+ and chained), shown beside Repeat. The
  add preview (`addPreview.ts`) shows them. The info panel shows the three rows. A card marks an iterative task the way
  it marks a recurring one, using `iter` (to decide).
- **Android** (`app/`): the same, ported with the web tests.
- **Terminal** (`crates/tui`): `task::VERSION` and the help text accept 3.6.x as well as 3.5.0. The form fields and info
  rows match the web's. With 3.5.0 the two fields are hidden, since that `task` would store `iter` as an orphan.
- **Skill** (`skills/meghnaad/SKILL.md`): "task must be 3.5.x" becomes 3.5 or 3.6. Add a line on iterative tasks:
  complete the one task; never create the next one by hand.
- **Docs:**
  - `docs/using/recurring.md` gets an "Iterative tasks" part (or a sibling page), covering what differs from
    recurrence and the timezone note;
  - `docs/taskrc-support.md` notes that there are no new settings;
  - `taskwarrior-gaps.md` gets a line;
  - the README names the feature.

## Acceptance criteria

- [ ] A task made by the real `task` 3.6.0 with `iter`, synced into the web app and the phone, appears in `next`, has
      an ID, and `done` on it creates the same successor (same UUID, fields and dates) that `task` 3.6.0 makes on the
      desktop. Syncing both leaves one task, not two.
- [ ] `add … iter:… iter_type:… due:…` works in the engine for each of the four `iter` forms and each `iter_type`
      spelling. The stored task matches `task` 3.6.0's export field for field (`iter_type` as typed, `iter_count:1`).
- [ ] `done` gives the dates `task` 3.6.0 gives for `fixed`, `fixed+` and `chained`, in a zone with DST (for example
      `America/New_York` across the March and November changes) and in a zone without it (`Asia/Dhaka`). Shown by a
      native test against TaskChampion 3.2.0's `set_status(Completed)`, and by the browser engine in a zone that is not
      UTC.
- [x] `COUNT` and `UNTIL` end the series. Deleting ends it. `modify iter:` asks (with `confirmation` on) and leaves a
      pending task without `iter_type` and `iter_count`.
- [ ] Each error in the parity section is given with the same text, for the same command line.
- [ ] `status:iterative`, `+ITERATIVE`, `+PENDING`, `-WAITING`, `+OVERDUE`, `+BLOCKED` and urgency behave as in
      `task` 3.6.0. `duplicate` starts a new series at 1. `info` and `stats` show the new rows.
- [ ] The successor carries tags, project, priority, UDAs and description, and drops annotations, dependencies and
      `start`.
- [x] `undo` after completing an iterative task removes the successor and reopens the old one, as `task` 3.6.0 does.
- [x] Web, Android and terminal: *Iter* and *Iter type* can be set in the add and edit forms and in bulk edit, and the
      info view shows all three fields. The terminal app runs with `task` 3.6.0.
- [ ] Tests: engine unit tests in `iter.rs` (parsing table, next-date table, shift across DST); end-to-end command tests
      (add, done, modify, delete, duplicate, undo, import/export, errors); `lua_hooks.rs` with the new fields; web unit
      tests for the form and bulk fields; Kotlin ports; a terminal-app test against `task` 3.6.0 on scratch data,
      skipped when it is missing.
- [x] Docs: `docs/using/recurring.md` (or a new `docs/using/iterative.md` in `SUMMARY.md`), `docs/taskrc-support.md`,
      `taskwarrior-gaps.md`, README, `skills/meghnaad/SKILL.md`. `mdbook build docs` is clean.
- [x] Size: browser engine and Worker wasm measured before and after.

## Open questions

- ~~**Parity target.**~~ Decided by uroybd, 2026-11-26: Meghnaad follows Taskwarrior 3.6.0 from now on. It ships
  with the other 3.6.0 tasks tagged `+tw36` (OR rejected in contexts, dashes in `rc.*` DOM references,
  `rc.sync.aws.endpoint_url`). AGENTS.md, the terminal app's `VERSION`, the help text and the skill move from 3.5.0
  to 3.6.0 in the same batch.
- ~~**The zone in the browser.**~~ Decided by uroybd, 2026-11-26: the web app uses the browser's default zone,
  `Intl.DateTimeFormat().resolvedOptions().timeZone`, passed to the engine by name. The phone does the same with
  `ZoneId.systemDefault()`. The engine computes in that zone, as in **Design**. There is no setting to override it,
  and we don't wait on TaskChampion for it. Asking upstream for a way to pass a zone, so the port can go later, is
  still worth doing, but it doesn't block this.
- **What Meghnaad does today** with a synced `status:iterative` task (TaskChampion 3.1 reads it as `Unknown`): hidden
  from reports? refused by `done`? hidden from `export`? Check before building, so the first criterion is known to be a
  fix and not a guess. If it is quietly mishandled now, a small fix could ship first: show it as pending, and refuse
  `done` with a message.
- **Hooks and the successor.** In `task` 3.6.0 the successor is made inside TaskChampion, so on-add probably does not
  run for it, and on-modify sees only the completed task. To check with a scratch hook; we do the same.
- **`undo`.** It is not yet confirmed that `task` 3.6.0's `undo` takes back the successor in the same step. Check it,
  and adjust the criterion to match.
- **Card indicator.** Should an iterative task get the recurring mark (`recurrence.indicator`'s `R`) on cards, a mark of
  its own, or none, as `task`'s reports show none?
- **Experimental upstream.** The man page says the UI and behaviour may change in minor versions. Do we ship it behind
  nothing (as `task` does) or label it experimental in the forms?

## Implementation notes

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.

**2026-11-27.** Built across the engine and the three front ends.

- Engine: TaskChampion 3.2 (`iterative-tasks` on in `tc-core`, off in the Worker), which also needs
  `Server::latest_version_id` (cloud, git, prefetch, the wire's remote, the phone's sync server). `tc_core::iter` ports
  the next-date code and computes in `Clock::zone` (a new IANA name, from the request's `zone`), else the fixed offset.
  `modify.rs` has the rules and messages, with `Change::Iterative` and `Change::EndSeries`. `cli/write.rs` completes
  one without `Task::set_status` and makes the next task (`next_in_series`), and asks before `modify iter:` ends a
  series. Filters, columns, sort, `calc`, `_unique`, completion, `stats`, `info`, export/import, hooks (Rust, Lua,
  the hook shim) and the working set know the fields. `is_open` / `is_pending` replace the `pending` checks as upstream.
- Front ends: Iter and Iter type in the add/edit forms, bulk edit and the quick-add preview, the three info rows, and
  iterative tasks counted as open (done/start, overdue, reminders, active). The terminal app follows 3.6.0, works with
  3.5, and hides the fields there.
- Checked: hooks and undo against `task` 3.6.0 (on-add not run for the next task; one undo takes back both). The next
  task matches TaskChampion's own `set_status(Completed)` property for property (native, mid-January, fixed). A request
  and the real browser engine in `America/New_York` keep midnight across the March change. The terminal app's form
  saved an iterative task through the real `task` 3.6.0 and `done` made the next. Interop with `task` 3.6.0 passes
  (sync both ways, UDAs, recurrence, snapshots); its UDA message check was stale since 158b4c7 and is fixed.
- Sizes (gzip, names dropped): Worker 723,343 → 723,910 bytes (the version bump); browser engine 912,629 →
  1,232,971 (`rrule`, `chrono-tz`, `text2rrule`, `regex`; not avoidable, `rrule` needs `chrono-tz`).
- Left: the first criterion end to end (a `task`-made iterative task synced into the web and phone and completed on
  both sides); the native comparison for `fixed+`, `chained` and DST zones; tests for every error, `+OVERDUE`,
  `+BLOCKED`, urgency, `stats`, and the successor's UDAs, dependencies and `start`. The card indicator and an
  experimental label are not done (cards show the status pill only).

