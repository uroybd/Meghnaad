# Tasks

- **Reports.** Pick any report, including your own, and add filters by typing (Tab completes) or from the helpers.
  Filters show as removable chips. A report's own filter and default sort are shown above the table.
  Reports whose names start with `_` (such as `_reviewed`, which the tasksh tab defines) are Taskwarrior's hidden helpers,
  so the sidebar and the report box leave them out. When there are any, an eye button beside **Reports** in the sidebar
  shows them all, and your choice is remembered. A hidden report you opened by name stays in the lists while you are on
  it, and typing its name at the prompt always works.
- **Empty columns are left out**, as Taskwarrior leaves them out: a column with nothing in it for any row of the report
  is not drawn (in the web app's table and the Android app's table). `print.empty.columns=yes` keeps them, and
  `rc.print.empty.columns:yes` does it for one command.
- **Sorting.** Click a column header (again to reverse, a third time to reset, Shift-click to add a tie-breaker). It is
  applied as `rc.report.<name>.sort:…`, so it matches what you would type.
- **Grouping.** A `/` on a sort key (`project+/`) puts a gap in the table wherever that column's value changes, as in
  Taskwarrior; here each group also gets a header saying what it has in common (`Project: Home`, or `Project: Home ·
  Outcome: fail` when two columns break the table). The column the table is grouped by shows a small group icon in its
  header. To choose it, hover a column's header and click its **Group** button (again to stop), or on a phone use the
  **Group** picker beside **Sort**. It is the same command underneath: grouping by `project` adds
  `rc.report.next.sort:project+/,urgency-` to the filter, and stopping removes it again. Works with UDAs, and
  `minimal` is grouped by project to begin with.
- **Reading the table.** Finished tasks are tinted instead of struck through: green for completed, red for deleted. IDs are bold, and hovering one shows the task's uuid with a Copy button. Project names are
  split at the dots, each part in its own colour. Tags are pills. Urgency is coloured by level (under 5, to 10, to 15,
  from 15). The `start.active`, `tags.indicator` and `depends.indicator` columns follow `active.indicator`,
  `tag.indicator` and `dependency.indicator`.
- **The detail view** (click a row): all fields, annotations, dependencies you can follow, a coloured status pill, the
  work sessions table (with `journal.time`) and a collapsed **History** of what changed and when (`journal.info`, on by
  default, worded like `task info`). History comes from the task's operation log, so changes made before the last
  snapshot are not in it. It is fetched when you open the section (the hidden `_history <uuid>`), not with the task,
  so `info`, the pages and exports never carry it. Buttons: done, start/stop, edit, delete.
- **Bulk actions.** In the Tasks table, tick the box on a row (the box in the header ticks them all) and, once two or
  more are ticked, a bar appears with **Complete**, **Delete**, **Modify…**, **CLI** and **Clear**. Complete and
  Delete act on every ticked task (Delete asks once, as the row's own button does, unless `confirmation` is off).
  **Modify…** opens an editor of lines, each a field and a value (project, priority, due, wait, scheduled, until,
  repeat, depends, your UDAs, and tags to add or remove): it says what it will do, and applies all the lines to every
  ticked task as one `modify`. An empty value clears the field. **CLI** puts the ticked tasks at the prompt (their
  numbers as a list, `1,3-5`, and the whole uuid of any without a number) and leaves you there to type the rest, for
  example `annotate called them`. A selection only lasts while it is in the table: another filter, or leaving the page,
  ends it. A long selection goes in several commands of 120 tasks, so `undo` takes it back a batch at a time.
- **Links between tasks.** Another task named in a task's *Deps* (or as the parent of a recurring instance) is drawn
  as its short id. Point at it, or focus it with the keyboard, and a small card shows its status, description, project,
  priority, due date and tags, with **Open task**. Where a pointer can hover, clicking the id opens the task at once;
  on a touch screen the first tap shows the card and **Open task** opens it.
- **Around a task, in the detail view.** *Depends on* lists the tasks this one waits for, one line each. Tap a line to
  open a card of that task, fetched then if the page does not already have it, with **Open task**. A task that others
  wait on shows a *Blocking* line; **Show the tasks waiting on this** looks them up (`( status:pending or status:waiting ) depends.has:<uuid>`)
  when you ask, so the detail does not search for them every time. The project there is clickable too, each part of it
  (`Home`, then `Kitchen`): it opens that project on the **Projects** page, with the projects above it expanded and the
  entry highlighted, as a tag chip does on the Tags page.
- **Every task has an address**, `/task/<first 8 characters of its uuid>`, which opens its drawer: the address bar
  follows the drawer, Back closes it, and a reload or a pasted link opens it again. The link button in the drawer
  copies it. Whoever you send it to needs to be able to sign in to the same deployment (it is one person's replica,
  so this is for sharing between your own devices, or with people your Cloudflare Access policy lets in). The 8
  characters are the uuid, not the task number `task 3` uses, because numbers differ between replicas and change as
  tasks finish; a longer prefix works too, and one that matches no task, or several, says so.
- **Add tasks** with the round **+** button that floats at the bottom right of every page, on a desktop as on a phone and in
  the Android app. It opens a box with one line, typed as in the console: `Buy milk due:tomorrow +home project:Home
  priority:H`. The words are the description; `project:`, `due:`, `priority:`, `recur:`, a UDA and `+tag` set the rest. Under
  the line a card shows the task as it will be made, as you type: the description, priority, project, dates, repeat, UDAs,
  dependencies and tags, read by the engine's own `add`, so what the card shows is what is added. A part that is wrong (a
  date that is not one, a priority that is not allowed, a repeat with no due date) is named under the card and Add waits
  until it is fixed. Enter adds. Put a word in quotes to keep it out of the fields (`'read project:x'`). *More fields* opens
  the full form with the line already broken into its fields: project, priority, due/wait/scheduled/until, tags,
  dependencies, UDAs, "start now" and a first note. Project and priority share a row, the four dates (due, wait, scheduled,
  until) share as many as the width allows, and so do repeat and tags. Every date field is a button that opens a panel: type a
  date as Taskwarrior reads one (`tomorrow`, `eow`, `eow-2d`, `2026-10-01`; the date words complete, Tab or a tap takes one), pick
  a keyword, or choose a day and an optional time from the calendar.
  The project and tags fields suggest names as you type (every project and tag you have, not only those on screen): ↑/↓
  to pick, Enter or Tab to take it, Esc to put the list away, or a tap. A tag is suggested on its own, and not again if
  it is already there. You can always type something else: a name nobody has used yet is how a new project or tag is
  made, and the field says so.
- **Annotations and string UDAs can span several lines.** In an annotation box Enter saves and Shift+Enter starts a
  new line; in a UDA field Enter starts a new line and Ctrl/Cmd+Enter saves. How a table shows annotations is up to
  the report's `description` column, as in Taskwarrior 3 (`description`, `description.oneline`, `description.count`,
  `description.desc`).
- **Notes and string UDAs are drawn as Markdown** in the detail view and in the reports (tables and, in the Android app,
  cards): paragraphs (a line break stays a line break), `#` headings, `**bold**`, `*italic*`, `~~strike~~`, `` `code` ``
  and fenced code, `[links](https://…)` and bare web addresses, bullet and numbered lists (nested), `>` quotes and `---`.
  Not tables, images, footnotes or HTML: those are shown as typed, and a link is only made to `http`, `https` or
  `mailto`. Text with none of it is shown as it always was, and the stored text is never changed by being shown.
  **Checklists** (`- [ ] milk`, `- [x] eggs`) have boxes you can click in a string UDA: the app changes that one box in
  the text and saves the whole new text with an ordinary `modify <uda>:<text>`, so the CLI, a hook and the history see
  a normal edit. The boxes in a note are only shown, because Taskwarrior has no way to change a note in place.
- **Dates are shown the way your taskrc says**: `dateformat`, `dateformat.report` (tables), `dateformat.info` (detail
  view), `dateformat.annotation` (notes) and a report's own `dateformat`, with Taskwarrior's tokens (`Y y M m D d H h N
  n S s A a B b V v J j w`). Without one, dates show as `2026-12-25`, plus the time when there is one.

**Summary** lists each project's tasks remaining, their average age, how much is complete and a bar, sub-projects
indented. A project's name opens its tasks in the Tasks view. `summary.all.projects=1` includes finished projects.

**Calendar** draws the months with week numbers, today, weekends, the days that have something due (green; due today
in purple; overdue in red) or scheduled, and holidays. It takes Taskwarrior's arguments (`calendar y`, `calendar due`,
`calendar 3 2027`, `calendar march 2027`) in the page's box, and has buttons for earlier, later, today, a year and
"from the first due date". A filter-shaped word (`project:Work`) narrows which tasks colour the days. Click a day to see
what is due in the `list` report. Holidays are defined in the taskrc as `holiday.<id>.name` and `.date` (or `.start`
and `.end`), in the taskrc itself or in a file it includes (`include holidays/en-US.rc`). **Settings → taskrc → Add
holidays** makes that file for you (below). With `calendar.holidays=full` (what Add holidays turns on), the holidays are
also listed under the months, hovering over one shows its name (several, if there are several), and on a phone a tap shows
it under the months (tap again to hide it). `sparse` only colours the days, as `task calendar` does. On a holiday that is also
due or scheduled, the first tap says which holiday it is and the second opens those tasks (a day with only tasks opens them
at once).

**A due or scheduled date on a holiday is flagged**, wherever you set it, as long as the taskrc defines holidays (whatever
`calendar.holidays` says, since that only chooses how the calendar draws them). The date is worked out first, so `eod`,
`som` or `eow-2d` are checked as the day they come to. In the task form the **Due** or **Scheduled** field gets a yellow
outline and a warning icon; hover it for the reason, or tap it: the panel it opens says it too ("due (eod) is 2026-12-25, a
holiday: Christmas Day"). In the console (and the Android prompt) the date word itself turns yellow when a command that sets
it (`add`, `log`, `modify`, `duplicate`) has it; hover it, or put the cursor in it (a tap does), and the reason shows above
the line. A filter such as `list due:2026-12-25` is not flagged, since it only asks which tasks are due then. The quick add's
preview card flags a date the same way. It is only a warning: nothing is changed or refused.

**Burndown** charts pending, started and done over time with the net fix rate and an estimated completion date, by day,
week, month or year. It counts as Taskwarrior does, quirks included (checked bar by bar). `burndown.cumulative=0`
stops carrying finished tasks forward.
