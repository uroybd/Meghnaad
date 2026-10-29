# Boards

A **board** is a page of your own made of **sections**, each a number, a table or a chart over the tasks its filter picks.
Open **Boards**, type a name and press **New board**; **Edit** turns the layout and the sections into something you can change,
and **Save** keeps it. Every board has a **slug**, the last part of its address (`/board/week`): one lower case word of
letters, digits, `-` and `_`, made from the name when the board is created and yours to change under **Edit** → **Address**.
It cannot be the same as another board's, and if the board is your home page, changing it moves the home page with it.
**Save as a copy** (under **Edit**, in the board's settings, so it is not pressed by accident) saves what is on the page as a new
board named "`<name> copy`", at an address made from that name, and shows it; the board you opened stays as it was last saved, and
the home page stays where it was. A board is kept in the app's own `settings.json` (key `boards`), not in your taskrc, so it follows you
to every device that signs in to the same deployment. A board is not a Taskwarrior report: it is built from what
`<filter> export` prints, so it sees every attribute of a task, your UDAs included, and runs nothing of its own.

**A section** has a title, a filter, and one of these to show. The charts are drawn by [Chart.js](https://www.chartjs.org/)
(loaded the first time a board draws one), with the page's colours; hovering a bar, point or slice shows its figure. On the phone they are drawn by Koala Plot, and a tap on a bar or a point shows its
label in full and its figure; the labels along an axis are thinned so none is cut off. The table is the app's own:

| Shows | What it is |
| --- | --- |
| **Number** | One figure: the count of the tasks, or a **sum**, **average**, **smallest** or **largest** of a field |
| **Bar chart** | The same figure for each value of an attribute (`project`, `tags`, `priority`, a UDA), or for each period of a date. Bars of a category lie down; bars of a date stand up |
| **Line chart** | The same, drawn as a line; it is meant for a date (`end` per week shows what you finish) |
| **Area chart** | A line with the area under it filled |
| **Pie chart**, **Donut chart** | The share of each value in the whole. One series, and its slices are the values; hover a slice for its share |
| **Radar chart** | The values around a circle, a shape for each series: good for comparing two or three series over a handful of values |
| **Polar area chart** | A pie whose slices differ in how far they reach, not in their angle |
| **Heatmap** | A grid with three things in it, as GitHub's grid of commits has: the columns (**Group by**: each week, say, or each project), the rows (**Rows**), and how strong each cell is (the measure: tasks, a sum, an average). The rows are a part of a date that comes round again (the **weekday**, the **hour**, the **day of the month** or the **month** of a date, which may be another date than the columns'), or the value of an attribute (project, a UDA); the weekdays begin where your week does. A new one is a column for each week the tasks were added and a row for each weekday. A **series** is a colour, not a row: each series is a grid of its own in its own colour, one above the other, and **Stacked** adds them into one grid (named "Success / Failed"), whose cells are in the colours of the series they are made of, in the proportions of what each had; hover or tap one to see the total and each series' own value. The cells are squares, and the grid sits in the middle of its card. Hover or tap a cell for its row, column and value; a cell with nothing to measure is faint. With **Rows** set to *A row for each series* the series are the rows of the one grid, each row in its series' colour, and **Stacked** adds them into a single row: a heatmap along the columns only |
| **Table** | Any report (`next`, `list`, one of yours) over the section's filter, drawn as on the Tasks page |

- **The filter** is the same one the Tasks page takes (Tab completes). A board can have a filter of its own, which every section
  adds to: `( board filter ) ( section filter )`, so an `or` in one cannot spill into the other. A section that should ask about
  its own tasks only has **Ignore the board's filter** ticked in its settings (off by default), and then the board's filter is
  not added to it.
- **Date ranges.** A board can have any number of named ranges, each a **start** and an **end** (both needed), in the board's
  settings. They sit above the board next to its filter, and a filter on the board (the board's, a section's or a series')
  uses them as `$name.start` and `$name.end`, so one change moves every card:
  `(end.after:$daterange.start and end.before:$daterange.end) or (scheduled.after:$daterange.start and scheduled.before:$daterange.end)`
  for a range named `daterange` (`sow` to `eow`). A date is anything Taskwarrior reads: `sow`, `eod`, `som`, `2026-10-01`,
  `2026-10-01T09:00`, `eow-2d`, in your time zone and with weeks starting on the day `weekstart` says; a bare date is the start
  of that day, as in `due.before:2026-10-07`. Each is a button that opens a panel with a field to type it in (the date words
  complete as you type, Tab or a tap takes one), buttons for the common words and a calendar day and time. The dates are written
  into the filter as the moment they mean, so a value with a space cannot break it. A name that is not one of the board's ranges,
  or a date that cannot be read (or an end before its start), is an error on the cards that use it, and nothing is run. The dates
  typed above a board are only for as long as the page is open (nothing is saved, **Reset** brings the board's own back); the
  board's own are changed in its settings. A name is lower case letters, digits and `_`, starting with a letter, and is shown as a
  label (`sprint_week` is "Sprint week").
  A range also says how long it is: `$name.minute_count`, `.hour_count`, `.day_count`, `.week_count`, `.month_count`,
  `.quarter_count` and `.year_count` are how many of that period it touches (the singular, as in "day count"). They are for a
  chart's **Keep**: a chart per day over `sow` to `eow` with **Keep** `$daterange.day_count` keeps 7 days. A period is counted as
  a chart cuts them, in your time zone and with weeks starting on the day `weekstart` says, and the end is not in the range
  (a range ending at midnight stops before it), so Monday 5 to Sunday 11 October is 7 days and 1 week, and Wednesday 7 to
  Tuesday 13 is 7 days and 2 weeks. **Keep** takes only a number or one of these counts; the editor names a range that is not
  the board's. The variables are also read in the series' filters, as in any other filter of the board.
  `$name.lg_group.name` is the **largest group** of the range: the step (`day`, `week`, `month` or `year`) a chart over it is best
  cut into, the largest that still gives at least two whole periods (a week is days, two weeks or a month are weeks, two months or a
  year are months, two years are years, and anything shorter than two days is days), and `$name.lg_group.count` is how many of those
  the range touches (a week is 7 days; a month is usually 5 weeks). A chart's **Per** has it as **Largest group (currently week)**,
  one for each range (with the range's name when there are several), so the chart regroups as the range changes, and **Keep** takes
  `$name.lg_group.count` to show exactly the range. The keyword buttons in the date panel say in a tooltip what each means and what
  it is now.
- **A date range as the axis.** A chart grouped by a date goes, by default, from the first period any task falls in to the last.
  **Over** makes the axis a range instead: **the board's date range** (by its name) or **dates of its own** (a **From** and a **To**,
  written as a range's dates are). The axis is then every period of the range, **Per** day, week, month and so on, whether or not
  a task falls in it, and a task outside the range has no place on it (the section's filter need not repeat the range). A burndown
  over a week per day has its seven days; and a series that counts what is open at the end of each period (**Counts**) stops at
  today, leaving the periods still to come empty, as `task burndown` stops. The periods are the range's (the same as its counts
  above, so a range ending at midnight stops before it), the range moves with the dates typed above the board, and a chart of the
  largest group (**Per**) of the same range gives exactly its whole periods.
- **Measuring** a field works on numbers (`urgency`, a numeric UDA) and durations (a duration UDA, as `2h` or `PT2H`). A
  figure over a duration is shown the way Taskwarrior writes one (`2h`, `3d`). A task with no value for the field is left out of
  a sum, average or comparison, and a group in which no task has one shows `–` (not zero). Anything else is refused, saying so.
- **Grouping** by an attribute makes a bar for each value; a task with none is counted under `(none)`. A task with several
  tags is counted once under each, so the bars can add up to more than the tasks (the figure above them does not). A date is
  cut into **days, weeks, months, quarters or years** in your time zone, with weeks starting on the day `weekstart` says, and a
  period with nothing in it is still drawn, so a line runs on and a chart has no gaps. A date is cut into at most 2000 periods.
- **Series.** A chart can have several, as Chart.js has datasets: each is a label and a filter (added to the board's and the
  section's), drawn as its own line or its own bars on the same axis, with a legend. **Stacked** piles the bars on each other
  or adds the lines up. A pie, donut or polar chart has one series (its slices are the values). A series can have a colour of its own, chosen
  from Taskwarrior's basic colours by name (`red`, `bright blue`, … drawn in the page's palette, so a choice follows a light and a
  dark page and your theme's look) or any other `#rrggbb` from the system's picker; with none, the series take the palette's
  blue, red, green, yellow, magenta and cyan in turn. A series can also have a measure of its own (in the file; it is the section's
  otherwise). **The colours of a UDA's values.** A chart grouped by the value of a UDA (`priority`, or your own) draws each value in
  the colour your taskrc gives it, `color.uda.<name>.<value>` (`color.uda.priority.H=red`; `color.uda.<name>.none` for tasks
  with no value): the slices of a pie, donut or polar chart, and the bars of a bar chart of one series. A value with no colour keeps
  the chart's own (the palette in turn; for bars, the series'), and a colour too faint for the page is darkened or lightened until it
  shows. It is the same on the phone, in the app and in a card's widget. A rule for every value (`color.uda.<name>`), and the colours
  of projects and tags, are not used; several series and charts over time keep their series' colours. They share the grouping, so a period in which one series has nothing still has a place for it (zero, or `–`
  for an average). Unlike `task burndown`, which counts what is pending at each moment, a chart counts per period by the date
  you choose (`end`, `entry`, `due`), so "finished per week, by outcome" is a stacked bar chart with series
  `Succeeded` (`outcome:success or outcome:`, which takes a task with no outcome as a success) and `Failed`
  (`outcome:failed`). Every series of a chart must measure the same kind of thing (numbers or durations).
  Each series has its own **Remove series** button in the card's settings; **Add series** puts a new one at the end.
  The order of the series is the order of stacking: the first is at the bottom and the next on it, and each series has **Move up**
  and **Move down** (arrows on the web) to change it. A series with no colour of its own keeps the colour it had when it moves.
- **Burndown-style series.** A series can count **what is open at the end of each period** instead of what happened in it,
  which is how `task burndown` counts: **Pending** (entered, not yet finished, and not started), **Started** (started, not yet
  finished) and **Done** (finished by then, so it adds up). Pick it in the series' **Counts** menu, or give it a `span` in the
  file: `{ "from": "entry", "to": "end", "unless": "start" }` is pending, `{ "from": "start", "to": "end" }` started and
  `{ "from": "end" }` done, and any other dates work (open from `scheduled` until `end`). The chart must be grouped by a date,
  and the periods run from the first task to the current one. Stack them to get Taskwarrior's burndown, and split any of them
  with its filter: Done into `outcome:success or outcome:` (a task with no outcome is a success) and `outcome:failed`. It counts
  the same as `burndown.weekly` for tasks that were started after they were entered (checked week by week), except that
  Taskwarrior counts only a task that is still pending as started, and a task started and then finished is started between the
  two here.
- **The info button.** Every card has an **ⓘ** in its corner. It opens a panel with what the card asks (its measure and grouping,
  or its report), the filter each series ran (with **Open in Tasks** to look at those tasks on the Tasks page), the figure or, for
  a chart, each value on its axis with the number of tasks behind it and the total over all the tasks, and the tasks the filter
  got (the first 200, with the columns it measures and groups by). It is the way to check where a number came from.
- **Order and number.** A chart of values puts the largest first; **Order** changes that (smallest first, or by name), and
  **Keep** shows only the first few. A date is in time order, and there **Keep** shows the latest periods:
  `per day` with **Keep** 7 is the last week, ending today. **Keep** is a number, or the count of one of the board's date ranges
  (below), so the chart follows the range.

**Laying out the board.** The page is 12 columns wide. In **Edit**, drag a section by its move handle (top right) to put it
elsewhere, and its bottom right corner to make it wider or taller; it snaps to the grid. What it lands on is pushed down and
everything floats up to the first free row, so there are never gaps or overlaps. No section is narrower than two columns.
A card too narrow for its tools (about 280 px) shows one **⋯** instead of them, with Information, Move, Settings, Duplicate and Remove
in the menu. **Duplicate** (two overlapping squares) puts a copy of a section, with its settings and the title "`<title> copy`", in the
first free place, and picks it. Without a mouse, each handle takes the arrow keys (move, or resize) and says where the section is now. **Add** puts a new
section in the first free place; **Settings** (the pencil) slides a panel in from the side with its filter, measure and
grouping (a full-width one on a phone); the bin removes it. **Cancel** drops everything since **Edit**.

**On a phone** the sections keep their reading order (top to bottom, left to right) on a grid of 6 columns, or 4 on a small
phone. A number is as wide as its share of the row: four numbers of three columns each sit side by side on 4 columns, three
to a row on 6. A table or a chart is too cramped in a quarter of a phone, so it stacks as Bootstrap's columns do: a whole row
if it is half the grid or wider, half a row if it is narrower. Two columns of the 12 is the narrowest, which is one column
of the phone's. A table or chart has these two sizes on a phone, so its corner drag there jumps between them. **Edit** there shows the same
board with the corner to drag to resize a section by the phone's columns (the width is saved in the 12, so it holds on a
wide screen too), up and down buttons to move it earlier or later in the order, and the same side panel for its settings.

You never have to write the file by hand, but you can: the format is below. A board or section that is not valid is named at
the top of the page and left out (the rest of `settings.json` is read as usual); saving a board writes the valid ones, so fix the
file first if you want to keep what was left out.

```json
{
  "boardsVersion": 1,
  "boards": [
    {
      "id": "week",
      "name": "My week",
      "description": "What I got done",
      "filter": "project.not:Someday",
      "ranges": [{ "name": "daterange", "start": "sow", "end": "eow" }],
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

`x` and `w` are columns (0 to 11, and 2 to 12); `y` and `h` are rows. `measure` is `{ "fn": "count" }` unless it says
otherwise, and `estimate` above is a duration UDA of yours.
