# Time tracking and reminders

- Add `journal.time=on` to your taskrc. `start` and `stop` then record "Started task" / "Stopped task" (the texts are
  configurable), and the detail view shows a sessions table with a running total. A running task shows a timer in the
  header and the tab title.
- The **bell** turns on reminders: a heads-up before a timed due date, a morning reminder for dates without a time,
  tasks coming off `wait`, and one digest a day for what's already overdue. In the browser reminders need the **tab to be
  open** (it can't wake up in the background), and system notifications need HTTPS or `localhost` plus your permission. In
  the Kotlin Android app they work with the app closed: the phone wakes the app at the moment something is due, and the
  digest comes at the hour you chose for dates with no time (Menu → Reminders; the options are the same, kept on the
  phone).
