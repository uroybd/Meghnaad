# Tasks

## Reports

Pick any report, including your own, and add filters by typing (Tab completes) or from the helpers. Filters show as
removable chips, and the report's own filter and default sort are shown above the table.

Reports whose names start with `_` (such as `_reviewed`, which the tasksh tab defines) are Taskwarrior's hidden helpers,
so the sidebar and report box leave them out. When there are any, an eye button beside **Reports** shows them, and the
app remembers your choice. A hidden report you opened by name stays listed while you are on it, and typing its name at
the prompt always works.

**Empty columns are left out**, as Taskwarrior leaves them: a column with nothing in it for any row isn't drawn, in the
web table and the Android one. `print.empty.columns=yes` keeps them (`rc.print.empty.columns:yes` for one command).

## Sorting and grouping

### Sorting

Click a column header: again to reverse, a third time to reset, Shift-click to add a tie-breaker. It is
applied as `rc.report.<name>.sort:…`, so it matches what you would type.

### Grouping

A `/` on a sort key (`project+/`) puts a gap in the table wherever that column's value changes, as in
Taskwarrior. Here each group also gets a header saying what it has in common (`Project: Home`, or `Project: Home ·
Outcome: fail` for two columns). Hover a column header and click **Group** to group by it (again to stop); on a phone
use the **Group** picker beside **Sort**. Underneath it is the same command: grouping by `project` adds
`rc.report.next.sort:project+/,urgency-` to the filter. It works with UDAs, and `minimal` starts grouped by project.

## Reading the table

- Finished tasks are tinted, not struck through: green for completed, red for deleted.
- IDs are bold, and hovering one shows the uuid with a Copy button.
- Project names are split at the dots, each part in its own colour. Tags are pills.
- Urgency is coloured by level (under 5, to 10, to 15, from 15).
- The `start.active`, `tags.indicator` and `depends.indicator` columns follow `active.indicator`, `tag.indicator` and
  `dependency.indicator`.
- Dates follow your taskrc: `dateformat`, `dateformat.report` (tables), `dateformat.info` (detail), `dateformat.annotation`
  (notes) and a report's own `dateformat`, with Taskwarrior's tokens (`Y y M m D d H h N n S s A a B b V v J j w`).
  Without one, dates show as `2026-12-25`, plus the time when there is one.

## The detail view

Click a row. It shows all fields, annotations, dependencies you can follow, a coloured status pill, the work sessions
(with `journal.time`) and a collapsed **History** of what changed and when (`journal.info`, on by default, worded like
`task info`). History comes from the task's operation log, so changes before the last snapshot aren't in it. It is
fetched when you open the section (the hidden `_history <uuid>`), so `info`, the pages and exports never carry it.
Buttons: done, start/stop, edit, delete.

### Links between tasks

A task named in *Deps* (or as the parent of a recurring instance) shows as its short id. Point at it, or focus it with
the keyboard, for a card with its status, description, project, priority, due date and tags, and **Open task**. With a
mouse, clicking the id opens the task; on a touch screen the first tap shows the card.

### Dependencies

**Depends on** lists the tasks this one waits for. Tap a line for a card of that task. A task others wait on shows a
*Blocking* line; **Show the tasks waiting on this** looks them up when you ask (`( status:pending or status:waiting )
depends.has:<uuid>`).

### Projects

The project is clickable, part by part (`Home`, then `Kitchen`). It opens that project on the **Projects** page, expanded
and highlighted, as a tag chip does on the Tags page.

### Task addresses

Every task has an address, `/task/<first 8 characters of its uuid>`, which opens its drawer. Back closes it, and a reload
or pasted link opens it again. The link button in the drawer copies it.

Whoever you send it to must be able to sign in to the same deployment, so it suits your own devices and people your
Cloudflare Access policy lets in. It uses the uuid, not the number `task 3` uses, because numbers differ between replicas
and change as tasks finish. A longer prefix works; one that matches no task, or several, says so.

## Adding tasks

The round **+** button at the bottom right of every page (desktop, phone and the Android app) opens a one-line box typed
as in the console: `Buy milk due:tomorrow +home project:Home priority:H`. The words are the description; `project:`,
`due:`, `priority:`, `recur:`, a UDA and `+tag` set the rest. Put a word in quotes to keep it out of the fields
(`'read project:x'`).

As you type, a card under the line shows the task as it will be made, read by the engine's own `add`, so what it shows is
what is added. A part that is wrong (a date that isn't one, a priority that isn't allowed, a repeat with no due date) is
named under the card, and Add waits until it is fixed. Enter adds.

**More fields** opens the full form with the line already broken into fields: project, priority, due, wait, scheduled,
until, repeat, tags, dependencies, UDAs, "start now" and a first note.

- Each date field opens a panel. Type a date as Taskwarrior reads one (`tomorrow`, `eow`, `eow-2d`, `2026-10-01`; the
  date words complete with Tab or a tap), pick a keyword, or choose a day and optional time from the calendar.
- Project and tags suggest every name you have, not just those on screen: ↑/↓ to pick, Enter or Tab to take it, Esc to
  close the list. You can always type something new; that is how a new project or tag is made, and the field says so.
- Annotations and string UDAs can span lines. In an annotation box Enter saves and Shift+Enter starts a new line. In a
  UDA field Enter starts a new line and Ctrl/Cmd+Enter saves.

## Notes and Markdown

Notes and string UDAs are drawn as Markdown in the detail view and in reports (tables and, on Android, cards):
paragraphs, `#` headings, `**bold**`, `*italic*`, `~~strike~~`, `` `code` `` and fenced code, `[links](https://…)` and bare
web addresses, nested bullet and numbered lists, `>` quotes and `---`. Not tables, images, footnotes or HTML; those show
as typed, and links are made only for `http`, `https` and `mailto`. Showing text never changes what is stored.

**Checklists** (`- [ ] milk`, `- [x] eggs`) have boxes you can click in a string UDA. The app changes that box and saves
the whole text with an ordinary `modify <uda>:<text>`, so the CLI, a hook and the history see a normal edit. Boxes in a
note are only shown, because Taskwarrior has no way to change a note in place.

How a table shows annotations is up to the report's `description` column, as in Taskwarrior 3 (`description`,
`description.oneline`, `description.count`, `description.desc`).

## Bulk actions

Tick rows in the Tasks table (the header box ticks all). With two or more ticked, a bar appears:

- **Complete** and **Delete** act on every ticked task. Delete asks once, as a row's button does, unless `confirmation`
  is off.
- **Modify…** opens lines of field and value (project, priority, due, wait, scheduled, until, repeat, depends, your UDAs,
  and tags to add or remove). It says what it will do and applies every line to every ticked task as one `modify`. An
  empty value clears the field.
- **CLI** puts the ticked tasks at the prompt (their numbers as a list like `1,3-5`, and the full uuid of any without
  one) and leaves you to type the rest, such as `annotate called them`.
- **Clear** drops the selection.

A selection lasts only while it is in the table: another filter, or leaving the page, ends it. A long selection goes in
batches of 120 tasks, so `undo` takes it back a batch at a time.

## Summary

Each project's tasks remaining, their average age, how much is complete and a bar, with sub-projects indented. A
project's name opens its tasks in the Tasks view. `summary.all.projects=1` includes finished projects.

## Calendar

Months with week numbers, today, weekends, holidays, and the days with something due (green; today in purple; overdue
in red) or scheduled. It takes Taskwarrior's arguments (`calendar y`, `calendar due`, `calendar 3 2027`, `calendar march
2027`) in its box, and has buttons for earlier, later, today, a year and "from the first due date". A filter-shaped word
(`project:Work`) narrows which tasks colour the days. Click a day to see what is due in the `list` report.

**Holidays** are defined in the taskrc as `holiday.<id>.name` and `.date` (or `.start` and `.end`), in the taskrc itself
or a file it includes (`include holidays/en-US.rc`). **Settings → taskrc → Add holidays** makes that file for you. With
`calendar.holidays=full` (which Add holidays turns on), holidays are also listed under the months, hovering shows the
name, and on a phone a tap shows it (tap again to hide). `sparse` only colours the days, as `task calendar` does. On a
holiday that is also due or scheduled, the first tap names the holiday and the second opens the tasks.

**A due or scheduled date on a holiday is flagged** wherever you set it, as long as the taskrc defines holidays. The date
is worked out first, so `eod`, `som` or `eow-2d` are checked as the day they come to.

- In the task form the **Due** or **Scheduled** field gets a yellow outline and a warning icon. Hover or tap it for the
  reason ("due (eod) is 2026-12-25, a holiday: Christmas Day").
- In the console (and Android prompt) the date word turns yellow when a command that sets it (`add`, `log`, `modify`,
  `duplicate`) has it. Hover it, or put the cursor in it, to see the reason.
- The quick add card flags it the same way. A filter such as `list due:2026-12-25` is not flagged, since it only asks
  which tasks are due then.

It is only a warning: nothing is changed or refused.

## Burndown

Pending, started and done over time, with the net fix rate and an estimated completion date, by day, week, month or
year. It counts as Taskwarrior does, quirks included (checked bar by bar). `burndown.cumulative=0` stops carrying
finished tasks forward.
