# 0003: Custom Reminder

- **Status:** in progress <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-16
- **Layer:**  2 (UI)
- **Front ends:** web, Android, terminal <!-- the ones this touches; say "all" or name the ones left out and why -->
- **Supersedes / related:** the existing reminders (`web/src/lib/notify.ts`, `Notify.kt`, `crates/tui/src/remind.rs`); review's UDA setup (`tasksh.ts`, `review.rs`)

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

- Snooze, or saying a reminder again after it was shown.
- Reminders at a set time of day (`remind_in` is always relative to `due` or `scheduled`).
- Reminders for a task with neither `due` nor `scheduled`.
- Reminders in the browser with no tab open: a page can't wake up in the background, and the Notification Triggers
  API never shipped.
- Terminal app notifications that come after it has closed (launchd, systemd timers, `at`). It notifies while it is
  open.

## Taskwarrior parity

Layer 2: `remind_in` is a plain string UDA, and only the front ends read it. Checked on the real `task` 3.5.0 with a
scratch `TASKRC` and data directory (2026-11-16):

```text
$ task config uda.remind_in.type string; task config uda.remind_in.label 'Remind in'
$ task add one due:tomorrow remind_in:10,20,30     # stored as "remind_in":"10,20,30"
$ task remind_in.has:20 count                       # 1: the usual string filters work
# with uda.remind_in.default=default, every new task stores "remind_in":"default" and shows it in export;
# tasks that already exist don't get it
$ task calc 10      -> 10
$ task calc 10min   -> PT10M
$ task calc 10m     -> P300D    # "m" is months to Taskwarrior, not minutes
$ task calc 1h      -> PT1H
$ task calc 1d      -> P1D
```

So we set no `uda.remind_in.default`: a task without the value uses the Reminders setting, and `default` written by
hand means the same. Units are Taskwarrior's (`tc_core::dates::parse_duration`), so `10m` is ten months, as
`task calc` says; a bare number is minutes. Ten minutes is `10`, `10min`, `10mins`, `10minute` or `10minutes`
(also `PT10M`); `1hr`, `1hour` and `90s` work too.

## Design

**The value.** `remind_in` is a comma-separated list. Each item is a bare number (minutes) or a Taskwarrior duration
(`10min`, `1h`, `2d`, `PT10M`). Empty, missing or `default` means the Reminders setting's lead time. `none` or `0` turns
reminders off for that task (its due and scheduled reminders; the wait-over and digest notices stay). An item that
doesn't parse is skipped; when none parses, the setting applies.

**What it reminds of.** For every offset, one notice at `due - offset` and one at `scheduled - offset`, for each date
the task has. The "due now" notice at the due time stays as it is. For a date with no time (midnight local), the
offsets count back from the Reminders hour for such dates, not from midnight (`30` with the hour at 9 means 8:30). A
waiting task is skipped, as now. Each notice has its own key (`uuid` + `due`/`scheduled` + the date + the offset), so
each is said once and changing the list or the date makes new ones.

This also gives `scheduled` reminders to tasks without `remind_in` (at the setting's lead), which nobody gets today.

**The default lead** stays 15 minutes on every front end, the terminal app included.

**Engine (layer 1 host, small).**

- `_remind` adds `scheduled` and `remind_in` to each task (`crates/tc-core/src/cli/lists.rs`). It is one of our own
  app-only commands, so no Taskwarrior behaviour changes.
- A helper `remind_offsets(value) -> Option<Vec<i64>>` in `tc_core::helpers`, returning seconds: `None` for
  missing/`default`/nothing parsed (use the setting), an empty list for `none`/`0`. It uses `dates::parse_duration`,
  and every front end calls it (web through `helpers.ts`, Android through JNI, the terminal app directly), so none of
  them parses durations itself.

**Turning it on**, as review's setup does (`tasksh.ts`, `review.rs`): the Reminders dialog shows whether the UDA is
defined and offers to add it. That runs `config uda.remind_in.type string` and `config uda.remind_in.label 'Remind in'`
with `rc.confirmation:no`, after asking. It is optional: reminders work without it, using the setting.

**Web** (`notify.ts`, `NotifyDialog.svelte`, `notifier.svelte.ts`): `Reminder` gains `scheduled` and `remind_in`.
`dueNotices` works out the moments as above. The dialog gets the UDA switch. Tab-open only, as now.

**Android** (`Notify.kt`, `NotifyDialog.kt`, `Reminders.kt`): the same port with its tests. The next alarm moment
includes the new ones, so they come with the app closed.

**Terminal app** (`crates/tui`):

- `remind.rs` follows the web's rules, with its tests.
- Reminder settings in `settings.json` under `tui.reminders`: `on`, `lead_minutes` (15), `all_day_hour` (9),
  `wait_over` (true), `desktop` (true). Missing keys use those defaults.
- A Reminders box (like the Export box) opens on `N`. It holds those options, the switch to turn reminders on or off
  (what `N` toggles now), and the `remind_in` UDA setup. The default keys asset (`web/public/keymaps/default.json`)
  changes `app.reminders` to "open the Reminders box".
- Besides the status line and Console page, each notice is also shown as a desktop notification while the app is open:
  `osascript -e 'display notification …'` on macOS, `notify-send` on Linux when it is installed. Nothing is said when
  neither exists, and `desktop: false` turns it off. The text is passed as arguments (no shell); the AppleScript reads
  them from `argv`, so nothing needs escaping.

**Docs.** `docs/using/time-tracking.md` (the UDA, the list, units, scheduled), `docs/android.md`, `docs/terminal.md`
(the box, the settings, desktop notifications), `docs/architecture.md` for the helper.

## Acceptance criteria

Each one checkable, by a test or by looking. The change is done when every box is ticked.

- [ ] A task with `remind_in:10,1h` and a due time gets a notice 1 hour and 10 minutes before it, on the web, on Android
  (app closed) and in the terminal app.
- [x] The same works for `scheduled`, and for a date with no time counted from the Reminders hour.
- [x] No value or `default` uses the setting; `none` or `0` gives no due or scheduled reminders; a bad item is skipped.
- [ ] The Reminders dialog on every front end adds the `remind_in` UDA, after asking.
- [x] The default lead is 15 minutes on every front end.
- [ ] The terminal app keeps its reminder settings in `settings.json` and shows desktop notifications on macOS and Linux
  while open.
- [x] Tests: `remind_offsets` in tc-core; `notify.test.ts`, the Kotlin `Notify` tests and `remind.rs` tests on the same
  cases; the `_remind` shape; the TUI Reminders box.
- [x] Docs: `docs/using/time-tracking.md`, `docs/android.md`, `docs/terminal.md`, `docs/architecture.md`.
- [x] Size: engine and Worker wasm measured before and after

## Open questions

- ~~Should a scheduled date also get a notice at the scheduled time itself?~~ No, only the offsets (as proposed;
  agreed 2026-11-16).
- ~~Should `scheduled` reminders at the setting's lead come for every task with a scheduled date?~~ Yes (as proposed;
  agreed 2026-11-16).

## Implementation notes

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.

- 2026-11-16: Discussed and agreed: no stored default; due and scheduled; offsets on undated dates count from the
  Reminders hour; Taskwarrior units, bare number = minutes; setup like review's; `_remind` gains the fields; terminal
  app gets desktop notifications while open (not after it closes) and the web/Android reminder settings. Default lead
  stays 15 minutes everywhere (changed from 10 the same day). Parity, Design and proposed criteria written. Nothing
  built yet.
- 2026-11-16: Built, on "go ahead" (which also took the proposed criteria and the two open questions as proposed).
  - Engine: `_remind` sends `scheduled` and `remind_in` (`cli/lists.rs`, test `reminders_carry_remind_in`);
    `helpers::remind_offsets` (seconds, latest first, deduped) with its unit test.
  - Web: `notify.ts` (`remindOffsets`, `reached`, `before` for due and scheduled), `notifier.svelte.ts` polls
    `scheduled.any:` too, `remindUda.ts` + the dialog's **Add the remind_in attribute** button; tests in
    `notify.test.ts`, `remindUda.test.ts`, `NotifyDialog.test.ts`.
  - Android: `Notify.kt` (same rules; `nextTrigger` sets the alarm for every offset and reads each `remind_in` once),
    `Reminders.kt` polls `scheduled.any:`, `RemindUda.kt` + the button in `NotifyDialog.kt`; `NotifyTest.kt`,
    `RemindUdaTest.kt`.
  - Terminal app: `remind.rs` (rules, `Settings` from `tui.reminders`, `desktop_command`/`desktop`),
    `app/reminders.rs` (the box on `N`; ↵ twice on its last line writes the UDA), drawn in `ui.rs`; tests in
    `remind.rs`, `app/reminders.rs` and `tests/reminders_box.rs`. Desktop notifications are skipped under `cfg(test)`.
  - Keys asset: `app.reminders` now reads "the reminder options, and turning them on or off" (the web's `N` already
    opened the dialog); copied to Android.
  - Departure: on a date with no time, the setting's lead is not used (only a task's own `remind_in`), since the
    hour's "Due today" notice is already the setting's reminder for those; a scheduled date with no time and no
    `remind_in` gets nothing.
  - Sizes (gzip -9, names dropped): engine 847,611 → 849,674 bytes (+2,063). Worker unchanged: 759,883 bytes before
    and after (built with names; the Worker runs no commands and calls no helpers).
  - Checks: format, lint, cargo test (1039), lua hooks, web (811), svelte-check, scripts (34), wasm checks, Kotlin
    (406) and assembleDebug, mdbook: all pass.
  - Left: see it work on a device (Android alarm with the app closed, its dialog's button) and a desktop
    notification on macOS and Linux; then tick the first, fourth and sixth criteria and set the spec to done.
- 2026-11-16: macOS desktop notification seen by the user: the `osascript` command `desktop_command` builds, with a
  title and a body holding quotes, `&` and a leading `-`, showed as typed. Linux (`notify-send`) is still to be seen
  before the sixth criterion is ticked.
