# The pages

| Page | What it shows |
| --- | --- |
| **Tasks** | Any report as a table, with filters. Click a row for the task's detail |
| **Projects** | Projects and sub-projects, with their tasks underneath, and a filter box |
| **Tags** | Every tag with its tasks and how many are overdue, expandable, and a filter box |
| **Summary** | How far along each project is |
| **Calendar** | Months, with what is due or scheduled on which day |
| **Burndown** | Pending, started and done over time |
| **Boards** | Pages of your own: numbers, tables and charts, each with its own filter, laid out on a grid |
| **Console** | Type commands; reports and answers print here |

**Every page has an address**, so a link to it can be shared with anyone who can sign in to the same deployment, opened in
a new tab, and the browser's back button goes where you were:

| Address | Page |
| --- | --- |
| `/` | Your **home page** (Settings → General): a report or a board, `next` when you have not chosen |
| `/report/<name>` | Tasks, as that report: `/report/next`, `/report/list`, `/report/_reviewed`, and every report of your own is its name |
| `/projects` `/tags` `/summary` `/calendar` `/burndown` `/console` | those pages |
| `/boards`, `/board/<slug>` | the list of boards, and one board (its slug is its address; see [Boards](boards.md#boards)) |
| `/settings`, `/settings/<tab>` | Settings, and one of its tabs: `general` `taskrc` `tasksh` `commands` `hooks` `urgency` |
| `/settings/taskrc/<path>` | one file of the taskrc folder, in the taskrc tab (`/settings/taskrc/holidays/en-US.rc`) |
| `/task/<8 characters>` | a task's detail, over the page behind it (the first 8 characters of its uuid) |

An address for a report or a board that is not there says so and opens your home page. A filter you typed is not in the address.

Summary, Calendar, Burndown, Projects and Tags have the same filter box as Tasks (Tab completes, filters become chips, the filter
helpers are below it) and it is focused when you open the page (not on a phone, where that would raise the keyboard).
The header also has the running **timer**, the **bell** (reminders) and **Settings** (the taskrc, tasksh, commands, hooks and urgency, below). Every page except Console has the prompt in a bar at the bottom; the Console has it inside.

**Projects and Tags start as your open tasks**, waiting ones included (three chips in their filter box: `status.not:completed`,
`status.not:deleted` and `status.not:recurring`, which leaves out finished tasks and recurring templates). That is a filter like any
other: take the `completed` or `deleted` chip away and those tasks come in too, tinted green (completed) or red (deleted), so a project or tag stays listed after
its last task is done. Any other filter works as well (`project:Home`, `due.before:eow`, `+work`).

**A long report draws in steps.** A report of hundreds of tasks takes a while to lay out, so the table draws the first 200
at once and more (300 at a time) as you scroll towards the end, or when you press **Show more** or **Show all** under it.
It is still the whole report: the select-all box, the count and the sort are about every task, drawn or not.

**Tag chips are buttons.** In the table on the Tasks page, clicking a tag chip toggles `+tag` in that report's filter (a
pressed chip means it is in the filter; click it again to take it out), as typing `+tag` would. Clicking a second tag joins it to the first with `or` (`( +a or +b )`, the tasks with either), and a third goes into the same parentheses; click one again to take it out. Tags you typed side by side, which all have to be there, get the next one added the same way. Anywhere else (a
report printed in the Console, the task detail, the Projects page, a task on the Tags page itself) it opens that tag's entry
on the **Tags** page, expanded and highlighted. A tag no pending task has still gets its entry, so a chip from a report of
finished tasks works too. **Open in Tasks** on an entry filters the Tasks page by that tag.

The **last command** is shown above the prompt, including what a click ran: pick a report or sort a column and you can
see the exact command, then press **↑** to edit and re-run it.
