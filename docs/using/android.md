# The Android app's own

Everything in this guide works in the Android app, in a layout made for a phone. This chapter is what the app has
beyond it. Installing and connecting it is in [The Android app](../android.md).

- **Everything in the web app**: the console, the pages, review, urgency, recurrence, hooks and settings, in a layout
  made for a phone: a menu for the pages and tasks as cards ([On a phone](phone.md)). Review is a
  card with buttons, and `review` in the console opens it too.
- **Works offline.** The engine and tasks are on the phone; changes sync when there is a network.
- **Pull down to sync.** At the top of any page but the console, pulling down syncs and redraws.
- **Any S3-compatible bucket**: Cloudflare R2, AWS S3, MinIO, Backblaze B2. The store must accept conditional writes
  (`If-Match` and `If-None-Match` on upload), which R2, MinIO and current AWS S3 do. Or a git repository, or a
  TaskChampion sync server ([First run](../android.md#first-run)).
- **Export and import** use the system's file pickers: `export` offers the file to keep, and `import` reads one and says
  what it would do before it writes anything.

## Tablets, foldables and keyboards

On a tablet, a foldable or a phone held wide (over 760 dp), a report is the web's table instead of cards:

- The report's own columns, with headers that sort (a long press adds a tie-breaker, like the web's Shift-click) and a
  button to group by the column, a tick box per row, and the row's buttons at the end. Columns take the width of what they
  hold and the description takes the rest. A report with more columns than fit scrolls sideways, with the tick box and
  buttons kept in place.
- Summary is a table, the calendar shows its months side by side, and the burndown grows to fill the width.
- A task's details open as a drawer on the right (560 dp, like the web's), the task form and review as a card in the
  middle, and Settings is no wider than the web's 820 dp.
- The menu isn't over the page: the list of reports is an always-open sidebar and the pages are tabs along the top (their
  names drop away below 1100 dp, and those of Review and Settings below 1400 dp). The floating **+** is the same.

With a keyboard attached, `/` focuses the prompt with its line selected (not while you're typing in
a field), Tab completes, ↑ and ↓ go through the last commands, Ctrl+L clears the console and Esc leaves the prompt. In
the Hooks editor Ctrl+S saves and Tab indents. In the task form Enter saves, and Ctrl+Enter saves from a multi-line field.
**Settings → Keybindings** changes the keys of the phone, the web app and the terminal app, as on the web
([Keybindings](settings.md#keybindings)).

## Reminders and links

### Reminders

Reminders have the same options as the web app (Menu → Reminders), including a task's own `remind_in` times and the
button that adds that UDA, and work with the app closed. The phone wakes the
app when something is due or comes off `wait`, and the notification opens the task. Allow notifications when asked. The
once-a-day digest of what is overdue comes at the hour you chose for dates with no time.

### Task links

The link button in a task's details copies `meghnaad://task/1a2b3c4d` (the first 8 characters of its
uuid, like the web's `/task/1a2b3c4d`). Opening it from a note or another app opens the task, on the phone that has the
task: the link names it and doesn't carry it.

### Sharing a task

The share button in a task's details, beside the link button, hands the task to another app through Android's share
sheet, as one of three texts (always in this order):

- **Short text**, for a chat: the description, then its project, priority, due date and tags as the quick add reads
  them, so the text shared back into Meghnaad fills the same fields:

  ```
  Buy milk
  project:home priority:H due:2026-11-20 +errand +shop
  ```

- **Task JSON**: the object `task <uuid> export` writes for it (not the array around it), which `task import` takes.
- **Task info**: the text `task <uuid> info` prints, at Taskwarrior's usual width (`defaultwidth`), colour off. It is for
  reading; `task import` does not take it.

Mail apps get the description as the subject.

### Page links

Every page has a `meghnaad://` link mirroring the web's addresses: `meghnaad://report/<name>`,
`meghnaad://board/<slug>`, `meghnaad://boards`, `meghnaad://settings/<tab>` (and `meghnaad://settings/taskrc/<path>` for
a taskrc file), the pages (`projects`, `tags`, `summary`, `calendar`, `burndown`, `console`) and
`meghnaad://task/<8 characters>`. A board has a button that copies its link. A link to a report or board that isn't
there says so and opens the home page.

## Boards

The boards you make on the web are in the menu under **Boards**, on 4 columns (6 on a wide phone), each card as wide as
its share of the row, so several numbers sit side by side. The same engine code works them out, so the numbers agree:
counts, sums and averages of numbers and durations, per value of an attribute or per day, week, month, quarter or year,
several series on one chart (stacked or side by side), and series that count pending, started and done at the end of
each period as `task burndown` does.

Boards are in `settings.json`, which can sync with the bucket or the git branch (**Settings → Sync**), so a board saved
on one device appears on the others after the next sync. **Settings → General** picks the page the app opens on: any report or board. **ⓘ** on every card shows its
filter (with **Open in Tasks**), the tasks that filter got and the values on its axis.

### Charts

Bars, lines, areas, pies, donuts and radar are drawn by [Koala Plot](https://github.com/koalaplot/koalaplot-core) (a
polar area chart is drawn as a pie). A heatmap, which Koala Plot has no plot for, is a grid the app lays out itself: one
per series, or one for all when stacked. A tap on a cell says its row, column and value.

### Editing a board

The pencil changes a board on the phone as the web does: its name, description, filter and address, and for each card its
title, type, filter, measure, grouping, order and series (colours, and which `task burndown` state they count). The board
is drawn as it will look.

- Tap a card to pick it, drag its corner to resize it in the phone's columns, and move it earlier or later with the
  arrows above.
- A card's settings open full screen. The engine says whether the draft is valid, and **Save** waits until it is.

## Widgets

Four, each with its own settings (long-press the widget, then its pencil; the same screen opens when you add one). They
redraw after any change, from the app or the widget, and sync by themselves about every half hour.

### Tasks

The tasks of any report (built-in or yours), narrowed by a filter (`project:Home +work`), as many as you
choose, with a title of your own (left empty, it is named after the report: `Most urgent tasks` for `next`). Tap a
circle to mark a task done, the text to open **that task**, **+** to add a task typed as in the console (`Buy milk
due:tomorrow +home`), and the sync button to pull now. Under each task it shows the fields you tick: project, due,
priority, tags, urgency, age, scheduled, wait, recurrence, task number, notes.

### Burndown

Daily, weekly or monthly bars of the tasks a filter picks, with finished at the bottom, then started, then
pending, and how long it would take to clear them at this pace.

### Board card

One card of a board, chosen in the widget's settings: a number drawn large, or a chart (bars, lines,
areas, pies, donuts, radar, heatmaps) from the same numbers as the board, in the widget's light or dark look. A tap
opens the board. A table card isn't drawn (it says so).

### Calendar

A month with today circled and a dot on days with something due (red overdue, amber today, blue after),
for the tasks a filter picks. The arrows go to other months, and the month's name goes back to this one. **A tap on a day
selects it** (a ring marks it) and, when the widget has room, shows that day's due or scheduled tasks: beside the month
when wide, under it when tall, and not at all when small, so resizing is how you choose. Two tasks are listed, each with
its time (or "scheduled") and project, and a tap opens the task. A **Show all** line opens the whole day in the app, as
the date above them does.
