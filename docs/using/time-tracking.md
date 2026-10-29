# Time tracking and reminders

**Time tracking.** Add `journal.time=on` to your taskrc. `start` and `stop` then record "Started task" and "Stopped
task" (the texts are configurable), and a task's detail shows its sessions and a running total. A running task shows a
timer in the header and in the tab title.

**Reminders.** The **bell** in the header turns them on. You get:

- a heads-up before a due date that has a time,
- a morning reminder for dates without one,
- a note when a task comes off `wait`,
- one digest a day of what is overdue.

In the browser the tab has to be open, because a page can't wake up in the background. System notifications also need
HTTPS (or `localhost`) and your permission. In the Android app reminders work with the app closed; the digest comes at
the hour you chose for dates without a time (Menu → Reminders).
