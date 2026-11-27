# Iterative tasks

```
add Pay rent iter:monthly iter_type:fixed due:2026-12-20
add Haircut iter:6wk iter_type:chained scheduled:2026-11-01
add Standup iter:"every weekday" iter_type:fixed due:tomorrow
3 modify iter:                          # end the series; the task stays, as a plain one
```

An iterative task is Taskwarrior 3.6.0's other way to repeat: one task at a time, with no template behind it. When you
complete it, the next one of its series is made. Its dates are moved on, and its number in the series (`iter_count`)
goes up by one. As in `task`, it is new and may still change in later releases (`man task-iterative`).

It is an ordinary task in every other way: it has an ID, shows in `next`, `list` and the other reports, can be due,
overdue, waiting, blocked and blocking, and has urgency like a pending task.

## What it needs

- **`iter`**, the rule, in one of four forms, tried in this order:
  1. an RRULE: `FREQ=WEEKLY;BYDAY=MO,TH`, `FREQ=MONTHLY;BYMONTHDAY=1;COUNT=12`;
  2. a Taskwarrior period: `daily`, `weekdays`, `weekly`, `3d`, `2wk`, `fortnight`, `quarterly`, `yearly`;
  3. an ISO 8601 duration: `P2W`, `P3D`, `PT12H`;
  4. English, quoted: `"every other day"`, `"monthly on the third friday"`, `"weekly on thursday three times"`,
     `"every day until March 6th, 2027"`.
- **`iter_type`**: how the next date is found.
  - `fixed` (or `fx`): the rule's next date after this task's. Rent is due on the 20th, whenever you pay it.
  - `fixed+` (`f+`, `fp`): the rule's next date after this task's or now, whichever is later. Missed ones are skipped,
    so taking the bins out late doesn't leave a pile of overdue ones.
  - `chained` (`ch`): counted from when you complete it. With `6wk`, the next haircut is six weeks after the last one
    (at the time of day you completed it). A rule with a `BY` part (`FREQ=WEEKLY;BYDAY=MO`) gives the first of its
    dates after now.

  It is stored as you typed it, so `fx` stays `fx`.
- **A date**: at least one of `due`, `scheduled` and `wait`. The next date counts from `due` if it has one, then
  `scheduled`, then `wait`. Every one of the four that is set (`until` too) moves by the same amount.

`add` and `modify` refuse what `task` refuses, with its words: a rule that can't be read, no type, no date, both `recur`
and `iter`, or a status changed by hand ("Clear iter to end the series instead"). `iter_count` can't be set.

## Completing it

`done` (a card's tick, the info view's button) completes the task and makes the next one, in one step: one `undo`
takes both back.

- The new task has the same description, project, priority, tags and UDAs. It doesn't take the annotations, the
  dependencies or a start time; those stay with the one you completed.
- The completed task keeps its `iter_count` and loses `iter` and `iter_type`, so only one task of a series is open.
- An RRULE's `COUNT` or `UNTIL` ends the series: once it has run out, completing the last one makes nothing.
- The new task's uuid is worked out from the old one's, as TaskChampion does. Two devices that complete the same task
  before syncing make the same next task, and the sync keeps one.
- Hooks see what `task`'s do: `on-modify` for the task completed (with its `iter` still set), and nothing for the new
  one.

## Ending a series

Delete the task, or clear its rule with `modify iter:`. With `confirmation` on, that asks first ("End the iterative series
for task 3 'Pay rent'?"). The task stays, pending, without `iter_type` and `iter_count`; give it an `iter` again and a new
series starts at 1. `duplicate` also starts a new series at 1.

## Filters, columns and fields

- `status:iterative` and `+ITERATIVE` match one. `+PENDING` does too, unless it is waiting, but `status:pending`
  doesn't: that is the status itself.
- An iterative task has a mark after its title, in reports and on cards: Lucide's `iteration-cw` on the web and the
  phone, `↪` in the terminal (a recurring one has `repeat`, `↻`).
- The columns `iter`, `iter_type` and `iter_count` can go in a report. The `status` column shows `iterative` (`I`).
- In bulk edit, **Iter type** and **Iter** are next to Repeat (in the terminal app, `Ctrl+O` on Iter opens its picker
  there too). The info view shows Iter, Iter type and Iter count.
- `stats` has an Iterative row. `info` shows the three rows and the `ITERATIVE` tag.
- There are no settings for it: `recurrence`, `recurrence.limit`, `recurrence.indicator` and `color.recurring` are for
  recurring tasks only.

## Repeat in the form

The add and edit forms (web, phone and terminal) have one **Repeat** choice: *Doesn't repeat*, *Iterative* or
*Recurring*. Each shows its own fields right under it, set apart as the choice's by a bar in the accent down their left
edge (on the web and the phone, in a tinted box too), on every platform. Iter type then Iter for an iterative task, How
often for a recurring one. Tags come after them. In the terminal app `Space` goes to the next choice.

**Iter** is a picker, as a date is: a button that says the rule in words and opens a panel (on the phone, a dialog; in
the terminal app, `Ctrl+O` on the field opens a box over the form). It has two modes:

- **Taskwarrior**: its periods (Every day, Every weekday, Every week, Every 2 weeks, Every month, Every 3 months, Every
  year) to tap, or anything `task` takes, typed: `3d`, `2wk`, `P2W`, `every other day`.
- **Calendar (RRULE)**: as a calendar app sets a repeat. *Repeats every* a number of days, weeks, months or years; for
  weeks, *on* which weekdays; for months, on the day of the month or on its weekday (the fourth Friday, the last
  Friday), from the task's due date (or its scheduled or wait date, else today); and *Ends* never, on a day, or after a
  number of times. It writes an RFC 5545 recurrence rule, the first of the forms `iter` takes:
  `FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,FR;UNTIL=20271231` is every 2 weeks on Monday and Friday, until 2027-12-31.

A rule opens in the mode that wrote it; one the Calendar mode can't show (an RRULE with other parts, English) opens in
the Taskwarrior mode, as typed. Switching modes and back keeps what was typed.

Switching clears the other mode's fields when you save, in the same `modify` (the form's command line shows it, and the
form says so under Repeat):

| From | To | The form runs |
| --- | --- | --- |
| Iterative | Doesn't repeat | `modify iter: iter_type:`: the series ends, the task stays |
| Doesn't repeat | Iterative or Recurring | the new mode's fields only |
| Iterative | Recurring | not offered: end the series first (Doesn't repeat, save), then choose Recurring |
| Recurring | anything | not offered: Taskwarrior won't take a recurring task's `recur` away; delete it to stop it |

A mode you chose but haven't filled in yet (no period, no rule) changes nothing. One instance of a series follows its
template. The console takes what the form doesn't: `modify iter: iter_type: recur:weekly` ends a series and makes the
task a recurring template in one step, as `task` 3.6.0 does, and to keep the other mode's fields, type the change there.

## Time zones

The next date is worked out in your time zone, so a task due at midnight on the 20th comes back at midnight on the 20th,
also across a change of the clocks. `task` uses the computer's zone. The web app uses the browser's, the phone uses the
phone's, and the terminal app runs `task` itself. In the web app and on the phone the zone goes with each command by name
(`Europe/Berlin`). Where a browser gives no name, the app uses the zone's current offset, and a date that crosses a change
of the clocks then lands an hour off.

The terminal app needs `task` 3.6.0 for iterative tasks. With 3.5 it runs as before, without the two fields.
