# Recurring tasks

```
add Water plants recur:weekly due:friday
add Pay rent recur:monthly due:eom until:2027-12-31
3 modify recur:2w                      # change the period of a recurring task
```

Periods are Taskwarrior's: `daily` `weekdays` `weekly` `biweekly` `monthly` `quarterly` `yearly`, counts such as `3d`
`2w` `6mo`, and ISO durations like `P1M`. A repeating task shows a repeat icon; its detail view lists its instances, and
an instance links back to it. The recurring task itself is a template: you can edit it but not complete or start it.

- **Recurrence is on by default**, as in Taskwarrior (`recurrence=1`). It is done when Taskwarrior does it: before the
  commands `task commands` flags **Recur** (the reports, `count`, `export`, `ids`, `projects`, the `history`,
  `ghistory` and `burndown` charts and the like, and the page's own reads), and not before one that changes a task.
  Then the app creates the instances that are due (`recurrence.limit=N` keeps N upcoming ones, default 1) and
  expires what is past its `until`: an instance it has just made is deleted at once, and the template's mask is left as
  it is. Taskwarrior itself advises that when several clients sync, one is primary and the others set `recurrence=0`,
  because two that each make an instance before they have synced make two. Here that does not happen between this
  app's own clients (the page, another tab, the phone): an instance's uuid comes from its template and its index, so
  they all make the same task and the replicas merge it into one. A real `task` on the same data does not know this
  trick (its instances have random uuids), so if you also run the CLI, set `recurrence=0` in its taskrc, or turn it off
  here with `recurrence=off`. With it off the app still shows and edits recurring tasks and reads instances made
  elsewhere.
- **Editing one task of a series** follows `recurrence.confirmation`: `prompt` (the default) asks whether to change
  all pending recurrences or only this task; `yes` always changes the whole series; `no` only the task you edited.
  As in Taskwarrior, from an instance `modify`, `append`, `prepend` and `annotate` go to the template and every pending
  instance (a `modify` carries its dates and `recur` too, so `modify due:friday` on one instance moves them all), and
  from a template only `modify` goes to its instances. `denotate`, `start`, `stop` and `done` never do.
- **Deleting** a template, or an instance, asks in the same way ("Do you want to delete all pending recurrences of this
  same task?"): `yes` deletes the template and its pending instances (from an instance, the whole series), `no` only the
  task named.
- A template's **mask** has a letter for each instance (`-` pending, `+` done, `X` deleted, `W` waiting). It follows
  `modify`, `start`, `done` and `delete` of an instance, and not a note or an `append`, as in Taskwarrior.
- A date can be given as another date of the same task, with a length of time added or taken off, and is read left to
  right: `add Review due:friday wait:due-1d recur:weekly`, or `modify scheduled:due-2h`. A date that the task does not
  have yet, or that comes later on the line, cannot be used.
- The filters `imask`, `mask` and `rtype` work as in Taskwarrior (`imask:0`, `rtype:periodic`, `mask.any:`); a task with
  no `imask` counts as 0.
- **Only periodic recurrence is acted on.** A template carries a recurrence type, `rtype`: `periodic` (every period from
  the due date, the default and the only thing the app and `task` 3.5.0 create) or `chained` (the next instance starts
  when the last one ends). Taskwarrior 3.5.0 knows the word `chained` but never acts on it, and does not let you set
  it, so neither does this app: a template that arrives with `rtype:chained` (an import, or another client) is kept and
  exported as it was, and its instances are made on the periodic schedule, as `task` makes them.
- `recurrence.indicator` (default `R`) is what the `recur.indicator` column shows.
