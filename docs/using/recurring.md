# Recurring tasks

```
add Water plants recur:weekly due:friday
add Pay rent recur:monthly due:eom until:2027-12-31
3 modify recur:2w                      # change the period of a recurring task
```

Periods are Taskwarrior's: `daily` `weekdays` `weekly` `biweekly` `monthly` `quarterly` `yearly`, counts such as `3d`
`2w` `6mo`, and ISO durations like `P1M`. A repeating task shows a repeat icon, and its detail lists its instances, each
linking back. The recurring task itself is a template: you can edit it, but not complete or start it.

## How instances are made

Recurrence is on by default (`recurrence=1`). Before the commands that Taskwarrior flags **Recur** (reports, `count`,
`export`, `ids`, `projects`, the `history`, `ghistory` and `burndown` charts, and the page's own reads) the app creates
the instances that are due and expires those past their `until`. It does not do this before a command that changes a
task. `recurrence.limit=N` keeps N upcoming instances (default 1).

Taskwarrior advises that when several clients sync, one is primary and the others set `recurrence=0`, because two that
each make an instance before syncing make two. This app's own clients (the page, other tabs, the phone) avoid that: an
instance's uuid comes from its template and its index, so they all make the same task and the replicas merge it. A real
`task` on the same data doesn't know the trick, so if you also run the CLI, set `recurrence=0` in its taskrc, or turn it
off here with `recurrence=off`. With it off, the app still shows and edits recurring tasks and reads instances made
elsewhere.

## Editing and deleting a series

- **Editing** follows `recurrence.confirmation`: `prompt` (default) asks whether to change all pending recurrences or
  just this task, `yes` changes the whole series, `no` only the task you edited.
- From an instance, `modify`, `append`, `prepend` and `annotate` go to the template and every pending instance. A
  `modify` carries dates and `recur` too, so `modify due:friday` on one instance moves them all. From a template only
  `modify` reaches the instances. `denotate`, `start`, `stop` and `done` never do.
- **Deleting** a template or an instance asks the same way. `yes` deletes the template and its pending instances (from
  an instance, the whole series), `no` only the task you named.
- A template's **mask** has a letter per instance: `-` pending, `+` done, `X` deleted, `W` waiting. It follows
  `modify`, `start`, `done` and `delete` of an instance, not a note or an `append`.

## Details

- A date can be given relative to another date of the same task, read left to right: `add Review due:friday
  wait:due-1d recur:weekly`, or `modify scheduled:due-2h`. A date the task doesn't have yet, or that comes later on the
  line, can't be used.
- The filters `imask`, `mask` and `rtype` work as in Taskwarrior (`imask:0`, `rtype:periodic`, `mask.any:`). A task
  with no `imask` counts as 0.
- `recurrence.indicator` (default `R`) is what the `recur.indicator` column shows.

### Only periodic recurrence is acted on

A template's `rtype` is `periodic` (every period from the due date, and all that the app and `task` 3.5.0 create) or
`chained` (the next starts when the last ends). Taskwarrior 3.5.0 knows the word `chained` but never acts on it, and
doesn't let you set it, so neither does this app. A template that arrives with `rtype:chained` (an import, or another
client) is kept and exported as it was, and its instances are made on the periodic schedule, as `task` makes them.
