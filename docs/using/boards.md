# Boards

A **board** is a page of your own made of **sections**, each a number, a table or a chart over the tasks its filter picks.
Open **Boards**, type a name and press **New board**. **Edit** lets you change the layout and sections, and **Save** keeps
them.

## Address

Every board has a **slug**, the last part of its address (`/board/week`): one lower-case word of letters,
digits, `-` and `_`. It is made from the name when the board is created, and you can change it under **Edit** →
**Address**. It must be unique, and if the board is your home page, changing it moves the home page too.

## Save as a copy

(under **Edit**, so it isn't pressed by accident) saves what is on the page as a new board named
"`<name> copy`" and shows it. The board you opened stays as last saved, and the home page stays where it was.

## Where it's kept

In the app's own `settings.json` (key `boards`), not your taskrc, so it follows you to every
device that signs in to the same deployment.

## Not a report

A board is built from what `<filter> export` prints, so it sees every attribute of a task, UDAs
included, and runs nothing of its own.

## Sections

A section has a title, a filter, and one thing to show. Charts are drawn by [Chart.js](https://www.chartjs.org/) on the
web (loaded the first time one is drawn) and by Koala Plot on the phone, in the page's colours. Hover a bar, point or
slice for its figure; on a phone, tap. Axis labels are thinned so none is cut off. The table is the app's own.

| Shows | What it is |
| --- | --- |
| **Number** | One figure: the count of the tasks, or the **sum**, **average**, **smallest** or **largest** of a field |
| **Bar chart** | The same figure for each value of an attribute (`project`, `tags`, `priority`, a UDA) or each period of a date. Category bars lie down; date bars stand up |
| **Line chart** | The same as a line, meant for a date (`end` per week shows what you finish) |
| **Area chart** | A line with the area under it filled |
| **Pie chart**, **Donut chart** | The share of each value in the whole. One series; its slices are the values |
| **Radar chart** | The values around a circle, a shape per series. Good for comparing two or three series over a few values |
| **Polar area chart** | A pie whose slices differ in how far they reach rather than their angle |
| **Heatmap** | A grid of columns, rows and cell strength; see [Heatmaps](#heatmaps) |
| **Table** | Any report (`next`, `list`, one of yours) over the section's filter, drawn as on the Tasks page |

### Heatmaps

A heatmap is like GitHub's grid of commits. It has three parts:

- **Columns** (**Group by**): each week, say, or each project.
- **Rows** (**Rows**): a part of a date that comes round again (the **weekday**, **hour**, **day of the month** or
  **month** of a date, which may be a different date than the columns') or the value of an attribute (project, a UDA).
  Weekdays begin where your week does.
- **Strength** of each cell: the measure (tasks, a sum, an average).

A new heatmap has a column per week the tasks were added and a row per weekday. Cells are squares and the grid sits in
the middle of its card. Hover or tap a cell for its row, column and value; a cell with nothing to measure is faint.

A **series** is a colour, not a row. Each series is its own grid in its own colour, one above the other. **Stacked** adds
them into one grid (named "Success / Failed") whose cells blend the series' colours in the proportions of what each had;
hover or tap one for the total and each series' value.

With **Rows** set to *A row for each series*, the series are the rows of one grid, each in its own colour, and **Stacked**
adds them into one row: a heatmap along the columns only.

## Filters

The filter is the one the Tasks page takes (Tab completes).

- **Board filter.** A board can have its own, which every section adds to as `( board filter ) ( section filter )`, so an
  `or` in one can't spill into the other.
- **Ignore the board's filter.** Tick it in a section's settings (off by default) for a section that should ask about its
  own tasks only.

## Controls

A board can have any number of named **controls**, set in the board's settings. They sit above the board next to its
filter, and filters on the board, a section or a series use them as variables, so one change moves every card. Each has a
**name** (what a filter writes after `$`) and an optional **label**, any text with spaces, which is what the board shows;
with none, the name is shown as a label (`sprint_week` is "Sprint week"). What a control starts as is set with it.

| Kind | Holds | A filter writes | Becomes |
| --- | --- | --- | --- |
| **Date range** | a start and an end | `$name.start`, `$name.end` (and [its counts](#counts-and-the-largest-group)) | the two moments |
| **Date** | one date or moment | `$name` | the moment, as Taskwarrior writes one (`20261009T000000Z`) |
| **Number** | one number | `$name` | the number as typed |
| **Number range** | a least and a most | `$name.min`, `$name.max` | the numbers |
| **Selection** | a value of projects, tags or a UDA with values; or several, with **Several at once**; or an expression of them | `$name`, `$name.asNative` | the values: `project:$name` is `project:Home`, `+$name` is `( +a or +b )`; `$name.asNative` is the term its source writes, the same |
| **Switch** | on or off, and a filter for each | `$name` | the filter in effect |

For a range named `daterange` (`sow` to `eow`):

```
(end.after:$daterange.start and end.before:$daterange.end) or (scheduled.after:$daterange.start and scheduled.before:$daterange.end)
```

and a board of open work, cut by a project picked above it, by tags, by urgency, by a due date and an estimate, and with
a switch for what is next:

```
status:pending project:$project +$tags urgency.above:$urgency.min urgency.below:$urgency.max due.before:$cutoff estimate.above:$least $next
```

Every control is a value, written where the value goes: after an attribute (`due.before:$cutoff`,
`project:$project`, `priority:$pri`), after a `+` (`+$tags`), or alone (`$next`).

### Selections

A selection offers the values open tasks have (`task _unique`): their projects (each with its parents: `FUL` before `FUL.Chores`, picking both, as `project:FUL` does), their tags, or a UDA's. **Offer values of
completed tasks** adds those that only completed tasks have. A UDA can be picked from when it has `uda.<name>.values`,
and offers those of its values in use, in their order. A value with a space is quoted (`project:"My Home"`).

The term `$name` is in (from the space or parenthesis before it to the one after it) is written once for each value
chosen, joined with **Join with**, which is `or` unless set, so a task with any of them is picked:

| Join with | `+$tags` with `a` and `b` | |
| --- | --- | --- |
| `or` (none set) | `( +a or +b )` | any of them |
| `xor` | `( +a xor +b )` | |
| `and` | `+a and +b` | all of them; no parentheses are needed |
| any other text, such as `,` | `+a,b` | the values joined inside the one term |

Any other join makes one value of them, so with `|` a regular expression can pick any of them:
`description~$words` is `description~a|b`.

In place of picking, a selection can be typed: the small button beside it swaps the choices for a field, which starts
from what is picked (`a or b`) and takes any value, one no task has yet too, or an expression of values: `a and (b or
c)`, with `+$tags`, is `( +a and ( +b or +c ) )`. Its words `and`, `or` and `xor` (in lower case, as `task` reads them)
and its parentheses are kept, each other word is a value (quote one with a space, `'My Home'`), and the term is written
for each. Written with a bare `$tags`, an expression's values are terms already: `+a and (+b or +c)`. Swapping back, or
picking a value, clears it. **Typed in at first** opens the selection with the field rather than the choices; one with
an expression always opens with it. Nothing chosen picks every task: the term is then `uuid.any:`, as for an empty
switch.

`$name.asNative` is short for the term the source writes natively: `$project.asNative` is `project:$project`,
`$tags.asNative` is `+$tags`, and for a UDA `$pri.asNative` is `priority:$pri`. Joins and expressions work the same, so
`$project.asNative` with two projects is `( project:A or project:B )`.

### Switches

A switch has a filter for **When on** and one for **When off**, such as `+next` and nothing. An empty one picks every task:
the term is then `uuid.any:`, which every task matches, so `$next or +urgent` and `status:pending $next` still read. A
switch alone in its term, with more than one word (`+a or +b`), is put in parentheses; within a term it is its text
as it is, so on `H` and off `L` make `priority:$level` a switch between two priorities.

### Dates

A date is anything Taskwarrior reads: `sow`, `eod`, `som`, `2026-10-01`, `2026-10-01T09:00`, `eow-2d`, in your time
zone with weeks starting on `weekstart`. A bare date is the start of that day. Each date is a button opening a panel
with a field (date words complete with Tab or a tap), buttons for common words, and a calendar with time. Dates are
written into the filter as the moment they mean, so a value with a space can't break it.

### Names

A name is lower-case letters, digits and `_`, starting with a letter. A label is one line of up to 80 characters. An
unknown name, an unreadable date or number, a least above its most, or a reference the kind doesn't have (`$week` for a
date range, `$urgency.start` for a number range) is an error on the cards that use it, and nothing runs. A `$` followed by
no name (the end of a regular expression, `description~done$`) is left as it is.

### Changing them

Values set above a board last only while the page is open (**Reset** brings the board's own back).
The board's own are changed in its settings, where each control's **Starts as** is the value it has.

### Counts and the largest group

A range also knows its length: `$name.minute_count`, `.hour_count`, `.day_count`, `.week_count`, `.month_count`,
`.quarter_count` and `.year_count` are how many of that period it touches. Use them in a chart's **Keep**: a chart per day
over `sow` to `eow` with **Keep** `$daterange.day_count` keeps 7 days. Periods are counted as a chart cuts them, in your
time zone, and the end isn't in the range (a range ending at midnight stops before it). Monday 5 to Sunday 11 October is
7 days and 1 week; Wednesday 7 to Tuesday 13 is 7 days and 2 weeks. **Keep** takes only a number or one of these counts.

`$name.lg_group.name` is the **largest group** of the range: the step (`day`, `week`, `month` or `year`) a chart over it
is best cut into, the largest that still gives at least two whole periods. A week is days; two weeks or a month are
weeks; two months or a year are months; two years are years; under two days is days. `$name.lg_group.count` is how many
of those the range touches (a week is 7 days; a month is usually 5 weeks). A chart's **Per** offers **Largest group
(currently week)**, one per range, so the chart regroups as the range changes, and **Keep** takes `$name.lg_group.count`
to show exactly the range. The keyword buttons in the date panel say in a tooltip what each means and what it is now.

### A range as the axis

A chart grouped by a date normally runs from the first period any task falls in to the last. **Over** makes the axis a
range instead: **the board's date range** (by name) or **dates of its own** (a **From** and **To**, written like a range's
dates).

- The axis is then every period of the range, **Per** day, week, month and so on, whether or not a task falls in it. A
  task outside the range has no place on it, so the section's filter needn't repeat the range.
- A burndown over a week per day has its seven days. A series that counts what is open at the end of each period
  (**Counts**) stops at today, leaving later periods empty, as `task burndown` stops.
- The periods are the range's, so the chart moves with the dates set above the board, and a chart of the largest group
  of the same range gives exactly its whole periods.

## Measuring and grouping

### Measuring

Measuring works on numbers (`urgency`, a numeric UDA) and durations (a duration UDA such as `2h` or `PT2H`). A
duration figure is shown as Taskwarrior writes one (`2h`, `3d`). A task with no value for the field is left out of a sum,
average or comparison, and a group where none has one shows `–`, not zero. Anything else is refused.

### Grouping

Grouping by an attribute makes a bar per value, and a task with none is counted under `(none)`. A task with several
tags counts once under each, so bars can add up to more than the tasks (the figure above them doesn't).

### Date periods

A date is cut into days, weeks, months, quarters or years in your time zone, with weeks starting on `weekstart`.
An empty period is still drawn, so lines run on and charts have no gaps. At most 2000 periods.

### Order and number

A chart of values puts the largest first; **Order** changes that (smallest first, or by name),
and **Keep** shows only the first few. A date chart is in time order, and there **Keep** shows the latest periods:
`per day` with **Keep** 7 is the last week, ending today.

A chart by the value of an attribute can show only some of its groups, or leave some out, before they are ordered and
kept: **Show only** and **Leave out** take groups with commas between them. Tasks tagged `feature` or `bugfix` that all
carry `jira` and `work` too make a chart by `tags` with **Leave out** `jira, work` show just `feature` and `bugfix`, or
the same with **Show only** `feature, bugfix`. A group is matched by its value or by the name it is shown with.

## Series

A chart can have several series, like Chart.js datasets. Each has a label and a filter (added to the board's and the
section's) and is drawn as its own line or bars on the same axis, with a legend. A pie, donut or polar chart has one
series, whose slices are the values. All series of a chart must measure the same kind of thing (numbers or durations),
and they share the grouping, so a period where one has nothing still has a place for it (zero, or `–` for an average).

### Stacking

**Stacked** piles bars on each other or adds lines up. The first series is at the bottom. **Move up** and **Move down**
change the order, and a series with no colour of its own keeps its colour when it moves. **Add series** puts a new one
at the end and **Remove series** deletes one.

### Colour

Choose one of Taskwarrior's basic colours by name (`red`, `bright blue`, …), drawn in the page's palette so
it follows a light or dark page and your theme, or any `#rrggbb` from the system picker. With none, series take the
palette's blue, red, green, yellow, magenta and cyan in turn.

### A measure of its own

A series can have its own measure, set in the file; otherwise it uses the section's.

### Per period, not pending

Unlike `task burndown`, which counts what is pending at each moment, a chart counts per
period by the date you pick (`end`, `entry`, `due`). So "finished per week, by outcome" is a stacked bar chart with a
series `Succeeded` (`outcome:success or outcome:`, which takes a task with no outcome as a success) and `Failed`
(`outcome:failed`).

### Colours of a UDA's values

A chart grouped by a UDA's value (`priority`, or your own) draws each value in the colour your taskrc gives it,
`color.uda.<name>.<value>` (`color.uda.priority.H=red`; `color.uda.<name>.none` for tasks with no value). That applies
to the slices of a pie, donut or polar chart and the bars of a bar chart of one series, on the web, on the phone and in
the widget. A value with no colour keeps the chart's own, and a colour too faint for the page is darkened or lightened
until it shows. A rule for every value (`color.uda.<name>`) and colours of projects and tags are not used, and several
series and charts over time keep their series' colours.

### Burndown-style series

A series can count **what is open at the end of each period** instead of what happened in it, which is how `task
burndown` counts. In the series' **Counts** menu pick:

- **Pending**: entered, not yet finished, and not started.
- **Started**: started, not yet finished.
- **Done**: finished by then, so it adds up.

In the file, give a series a `span`: `{ "from": "entry", "to": "end", "unless": "start" }` is pending, `{ "from": "start",
"to": "end" }` started and `{ "from": "end" }` done. Any other dates work (open from `scheduled` until `end`).

The chart must be grouped by a date, and its periods run from the first task to the current one. Stack the three to get
Taskwarrior's burndown, and split any with its filter (Done into `outcome:success or outcome:` and `outcome:failed`). It
counts the same as `burndown.weekly` for tasks started after they were entered (checked week by week), except that
Taskwarrior counts only a task still pending as started, and here a task started and then finished is started between
the two.

## The info button

Every card has an **ⓘ** in its corner. It opens a panel with:

- what the card asks (its measure and grouping, or its report),
- the filter each series ran, with **Open in Tasks** to look at those tasks,
- the figure or, for a chart, each value on its axis with the number of tasks behind it and the total over all tasks,
- the tasks the filter got (the first 200, with the columns it measures and groups by).

It is how you check where a number came from.

## Laying out the board

The page is 12 columns wide. In **Edit**:

- Drag a section by its move handle (top right) to move it, and its bottom-right corner to resize. It snaps to the grid.
  Whatever it lands on is pushed down, and everything floats up to the first free row, so there are no gaps or overlaps.
  No section is narrower than two columns.
- Without a mouse, each handle takes the arrow keys (move or resize) and says where the section now is.
- **Add** puts a new section in the first free place. **Settings** (the pencil) slides in a panel with its filter,
  measure and grouping (full width on a phone). The bin removes a section. **Cancel** drops everything since **Edit**.
- **Duplicate** (two overlapping squares) copies a section, with its settings and the title "`<title> copy`", into the
  first free place and selects it.
- A card too narrow for its tools (about 280 px) shows one **⋯** with Information, Move, Settings, Duplicate and Remove.

**On a phone** the sections keep their reading order (top to bottom, left to right) on a grid of 6 columns, or 4 on a small
phone. A number is as wide as its share of the row: four numbers of three columns sit side by side on 4 columns, three to
a row on 6. A table or chart would be cramped in a quarter of a phone, so it stacks as Bootstrap's columns do: a whole
row if it is half the grid or wider, half a row if narrower. Two of the 12 columns is the narrowest, which is one phone
column, so a table or chart has just those two sizes and the corner drag jumps between them. **Edit** there has the same
corner drag (the width is saved in the 12, so it holds on a wide screen), up and down buttons to move a section earlier or
later, and the same side panel.

## The file format

You never have to write it by hand, but you can. A board or section that isn't valid is named at the top of the page and
left out (the rest of `settings.json` is read as usual). Saving writes only the valid ones, so fix the file first if you
want to keep what was left out.

```json
{
  "boardsVersion": 1,
  "boards": [
    {
      "id": "week",
      "name": "My week",
      "description": "What I got done",
      "filter": "project.not:Someday",
      "controls": [
        { "name": "daterange", "label": "This week", "type": "daterange", "start": "sow", "end": "eow" },
        { "name": "project", "type": "select", "source": "project", "completed": true },
        { "name": "next", "type": "boolean", "on": "+next", "off": "" }
      ],
      "sections": [
        { "id": "open", "title": "Open", "x": 0, "y": 0, "w": 3, "h": 2, "type": "number",
          "filter": "status:pending" },
        { "id": "done", "title": "Done per week", "x": 3, "y": 0, "w": 9, "h": 4, "type": "line",
          "filter": "status:completed", "by": { "time": { "field": "end", "step": "week" } } },
        { "id": "outcome", "title": "Finished per week, by outcome", "x": 0, "y": 4, "w": 12, "h": 4, "type": "bar",
          "stacked": true, "filter": "status:completed", "by": { "time": { "field": "end", "step": "week" } },
          "series": [
            { "label": "Succeeded", "filter": "outcome:success or outcome:", "color": "green" },
            { "label": "Failed", "filter": "outcome:failed", "color": "red" }
          ] },
        { "id": "effort", "title": "Estimate by project", "x": 0, "y": 8, "w": 12, "h": 4, "type": "bar",
          "measure": { "fn": "sum", "field": "estimate" }, "by": { "field": "project" }, "limit": 10 },
        { "id": "days", "title": "Done per day, this range", "x": 0, "y": 12, "w": 12, "h": 4, "type": "bar",
          "filter": "status:completed",
          "by": { "time": { "field": "end", "step": "$daterange.lg_group.name", "range": "daterange" } } },
        { "id": "own", "title": "Done in October, per week", "x": 0, "y": 16, "w": 12, "h": 4, "type": "bar",
          "filter": "status:completed",
          "by": { "time": { "field": "end", "step": "week", "range": { "start": "2026-10-01", "end": "2026-10-31T23:59" } } } }
      ]
    }
  ]
}
```

A board saved before there were other controls keeps its date ranges under `ranges` (a `name`, `start` and `end`); they
are read as date-range controls and written back under `controls` when the board is next saved. A control is one object:
`name`, an optional `label`, a `type`, and that kind's fields, which hold what it starts as:

```json
{ "name": "week", "label": "This week", "type": "daterange", "start": "sow", "end": "eow" }
{ "name": "cutoff", "type": "date", "value": "eow" }
{ "name": "least", "type": "number", "value": "2" }
{ "name": "urgency", "type": "numrange", "min": "1", "max": "10" }
{ "name": "project", "type": "select", "source": "project", "values": ["Home"] }
{ "name": "tags", "type": "select", "source": "tags", "multiple": true, "completed": true, "values": ["a", "b"], "join": "and" }
{ "name": "areas", "type": "select", "source": "tags", "expr": "a and (b or c)", "typed": true }
{ "name": "next", "type": "boolean", "on": "+next", "off": "", "value": false }
```

`source` is `project`, `tags` or the name of a UDA; `values` is what is chosen (none: any), `join` what joins them
(none: `or`), `expr` an expression typed in their place, and `typed` that the field to type in is shown first. Dates are
anything Taskwarrior reads; a number may be written as a number or as text.

`x` and `w` are columns (0 to 11, and 2 to 12); `y` and `h` are rows. `measure` is `{ "fn": "count" }` unless it says
otherwise, and `estimate` above is a duration UDA of yours. A chart by a value may have `only` and `except`, lists of
the groups it shows or leaves out.
