# 0039: The Iter picker

- **Status:** done
- **Created:** 2026-11-27
- **Layer:** both (the rule is the engine's; the pickers)
- **Front ends:** all (web, Android, terminal)
- **Supersedes / related:** spec 0035 (iterative tasks), 0036 (the Repeat choice).

## Why

`iter` takes four forms, and the richest (an RFC 5545 rule, `FREQ=WEEKLY;BYDAY=MO,FR`) is the one nobody types from
memory. Calendar apps set the same thing with a few choices (uroybd, 2026-11-27, with screenshots of a calendar app's
repeat menu and its Custom recurrence screen).

## What

- In the form, Iter type comes before Iter.
- Iter is a picker, like a date: a button with the rule in words, opening a panel (web), a dialog (phone), or a box over
  the form on `Ctrl+O` (terminal), with two modes:
  - **Taskwarrior**: the periods (`RECUR_PRESETS`) to pick, or anything typed.
  - **Calendar (RRULE)**: Repeats every N day(s)/week(s)/month(s)/year(s); weekly, on which weekdays; monthly, on the
    day of the month or the nth / last weekday, from the task's date; Ends never, on a day (`UNTIL=YYYYMMDD`), or after N
    times (`COUNT`). It starts as every week on the task's weekday (due, else scheduled, else wait, else today).
- A rule opens in the mode it is in; an RRULE the Calendar mode can't show stays in the Taskwarrior mode, as typed.

## Taskwarrior parity

`task` 3.6.0 on scratch data took every rule the builder writes, and made the next task as expected: `BYDAY=FR`,
`INTERVAL=2;BYDAY=MO,FR;UNTIL=20271231`, `UNTIL=…T235959Z`, `BYDAY=3FR`, `BYDAY=-1FR`, `BYMONTHDAY=27;COUNT=5`, `YEARLY`;
`COUNT=1` ends the series.

## Design

`tc_core::iter_rule`: `CalendarRule`, `to_rrule`, `from_rrule` (only what the builder writes), `default_for`,
`month_choices`, `describe`, `describe_iter`; helpers `iter_rule_*` and `describe_iter` for the web and the phone.
Web: `IterField.svelte`, `iterRule.ts`. Phone: `IterInput.kt`, `IterRule.kt`. Terminal: `iterpick.rs`
(`Mode::IterPick`), drawn by `ui::iter_box`.

## Acceptance criteria

- [x] Iter type before Iter, on the three.
- [x] Both modes on the three; the Calendar mode builds the rules above.
- [x] Tests: `iter_rule::tests`, helper tests, `IterField.svelte.test.ts`, `IterRuleTest.kt`, `iterpick::tests`,
  `ctrl_o_on_iter_opens_the_picker_and_its_rule_goes_in_the_field`.
- [x] Docs: `docs/using/iterative.md`, `docs/terminal.md`.

## Implementation notes

**2026-11-27.** Tried by hand in the terminal app (a rule picked, saved, and read back from `task`). The web panel and
the phone dialog are tested and built, not yet seen in a browser or on a phone.
