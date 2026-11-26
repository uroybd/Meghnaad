# 0036: One Repeat choice in the task form

- **Status:** done
- **Created:** 2026-11-27
- **Layer:** both (one engine fix; the forms)
- **Front ends:** all (web, Android, terminal)
- **Supersedes / related:** Taskwarrior task 71b0056f ("Tabular Repetition Editor", Meghnaad.UX); spec 0035 (iterative
  tasks).

## Why

A task can be plain, iterative or recurring, never two at once, but the forms offered a Repeat period and the Iter
fields side by side, and left it to the person to empty one before filling the other. Task 71b0056f: "let's have a
dropdown with options for no repeat, iterative and recurring; upon selecting one, the respective fields will be shown.
Applying it will apply, and if switched to another mode, it will clear the other mode's fields. If a user doesn't want
this convenient cleanup, they have the command line anyway. So the cleanup will be crafted as part of the modify
command run by the form."

## What

- The add and edit forms have one **Repeat** choice: *Doesn't repeat*, *Iterative*, *Recurring* (the terminal app:
  `Space` goes to the next). Only the chosen mode's fields show: the period (**Every** in the terminal) for recurring,
  **Iter** and **Iter type** for iterative.
- Saving sends the chosen mode's fields where they changed and clears the other modes' fields the task had, in the same
  `add` or `modify`: iterative to none is `modify iter: iter_type:`. The form says so under Repeat ("Saving ends its
  iterative series …; then it can be made recurring"), and its command line shows it.
- An existing iterative task is not offered Recurring (uroybd, 2026-11-27: "we will block that switch too"): its series
  is ended first, then it can be made recurring. The console still takes `modify iter: iter_type: recur:weekly`.
- A mode chosen but not filled in (no period, no rule) changes nothing.
- A recurring task's Repeat can't be switched (`task` won't take `recur` away); an instance follows its template.
- The terminal app with `task` 3.5 (no iterative tasks) keeps its one Repeat field, as before.

## Not doing

- Bulk edit: it sets one field on many tasks, and keeps its separate Repeat, Iter and Iter type fields.
- A rule builder for `iter` (weekdays, counts); the field takes what `task` takes.

## Taskwarrior parity

On `task` 3.6.0 with scratch data: `modify iter: iter_type: recur:weekly` on an iterative task makes it a recurring
template (and its first instance); `modify iter: iter_type:` makes it plain; on a recurring template `modify recur:`
gives "You cannot remove the recurrence from a recurring task." and `modify iter:… iter_type:…` "A task cannot be both
recurring and iterative."; `modify iter_type:` alone on an iterative task "You cannot remove the iteration type from an
iterative task." The engine refused to make an iterative task recurring (it took only a pending one); fixed in
`modify::becomes_recurring`.

## Design

The rule is ported per front end, with its tests: `web/src/lib/repeat.ts`, `app/…/Repeat.kt`, and `RepeatMode` /
`repeat_changes` in `crates/tui/src/form.rs` (a `Kind::Mode` field; `Form::shown` decides what is drawn and where
`Ctrl+N`/`Ctrl+P` go; `Form::clears` the notice).

## Acceptance criteria

- [x] The Repeat choice shows each mode's fields and only those, on web, Android and terminal.
- [x] Switching clears the other mode's fields in the form's own command; a half-filled mode changes nothing.
- [x] An iterative task made recurring in one `modify` (the console) becomes a template, as in `task` 3.6.0.
- [x] A recurring task's mode is locked, and an iterative task is not offered Recurring.
- [x] Tests: `repeat.test.ts`, `clashViews.test.ts` (the form), `RepeatTest.kt`, the TUI form's tests,
  `an_iterative_task_made_recurring_in_one_modify_is_a_template`.
- [x] Docs: `docs/using/iterative.md` (Repeat in the form), `recurring.md`, `tasks.md`, `terminal.md`.

## Implementation notes

**2026-11-27.** Built on the three front ends. The phone's form state (`EditorState`) has no unit test of its own (it
needs a `Store`); its rule (`Repeat.kt`) has, and the debug app builds. Not tried by hand in a browser or on a phone.
