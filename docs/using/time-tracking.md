# Time tracking and reminders

**Time tracking.** Add `journal.time=on` to your taskrc. `start` and `stop` then record "Started task" and "Stopped
task" (the texts are configurable), and a task's detail shows its sessions and a running total (the terminal app: the **Time log** tab of a task's
info). Every app pairs those notes into sessions the same way. A running task shows a
timer in the header and in the tab title.

**Reminders.** The **bell** in the header turns them on. You get:

- a heads-up before a due or scheduled date that has a time (15 minutes by default),
- a morning reminder for due dates without one,
- a note when a task comes off `wait`,
- one digest a day of what is overdue.

In the browser the tab has to be open, because a page can't wake up in the background. System notifications also need
HTTPS (or `localhost`) and your permission. On the static site a tab that locks (after the idle minutes, or with
**Lock now**) keeps reminding from the tasks it had when it locked: the locked page says how many. It keeps only what a
reminder needs (each task's description and dates), in that tab alone, until it is unlocked or closed; it fetches
nothing while locked, so a task finished meanwhile on another device can still be reminded of. Clicking such a reminder
opens the unlock form, and the task once unlocked. A tab opened while every tab is locked reminds of nothing until
unlocked. In the Android app reminders work with the app closed; the digest comes at
the hour you chose for dates without a time (Menu → Reminders).

A reminder goes away once its task is done or deleted, wherever you did it: the app takes it away the next time it
looks at the tasks. On the phone an alarm syncs first (for a few seconds at most), so a task done on another device isn't
announced.

**A task's own reminders.** Give a task `remind_in` to choose when it reminds you, instead of the heads-up setting:

```text
task add Call the bank due:tomorrow 15:00 remind_in:1h,10
task 12 modify remind_in:none
```

- It is a list, separated by commas, of times before the task's `due` and `scheduled` dates. A bare number is minutes;
  anything else is a duration as Taskwarrior writes it: `10min`, `1h`, `2d`, `1w`, `PT10M`. Careful: `10m` is ten
  **months**, as `task calc 10m` says. Write `10` or `10min` for ten minutes.
- Each time is said once: with `1h,10` you hear an hour before and again ten minutes before.
- For a date with no time, the times count back from the hour you chose for such dates (`30` with 9:00 is 8:30, `1d`
  is 9:00 the day before).
- `none` or `0` turns off the reminders before that task's dates; "due now", the wait and the digest still come. No
  value, or `default`, uses the setting. An item that doesn't read as a time is skipped.
- It is a UDA, so Taskwarrior needs it defined to keep it on a task: the Reminders dialog (and the terminal app's
  Reminders box) has **Add the remind_in attribute**, which writes `uda.remind_in.type=string` and its label with
  `config`, as review's setup does. The same line works with the real `task` once your taskrc has it.
