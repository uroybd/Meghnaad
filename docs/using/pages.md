# The pages

| Page | What it shows |
| --- | --- |
| **Tasks** | Any report as a table, with filters. Click a row for the task's detail |
| **Projects** | Projects and sub-projects with their tasks, and a filter box |
| **Tags** | Every tag with its tasks and how many are overdue, and a filter box |
| **Summary** | How far along each project is |
| **Calendar** | Months, with what is due or scheduled on each day |
| **Burndown** | Pending, started and done over time |
| **Boards** | Pages of your own: numbers, tables and charts, each with its own filter |
| **Kanban** | Your tasks as columns of cards, moved from one to another; there once it is set up ([Kanban](kanban.md)) |
| **Console** | Type commands; reports and answers print here |

**Boards** and **Kanban** hold several of something, so in the top bar each opens a menu: every board (or kanban), to
open it directly, and last the list of them. On a phone the menu lists them under the page.

## Addresses

Every page has an address. You can share a link with anyone who can sign in to the same deployment, open it in a new
tab, and the browser's back button goes where you were.

| Address | Page |
| --- | --- |
| `/` | Your **home page** (Settings → General): a report or a board, `next` if you haven't chosen |
| `/report/<name>` | Tasks, as that report: `/report/next`, `/report/list`, and any report of your own by name |
| `/projects` `/tags` `/summary` `/calendar` `/burndown` `/console` | those pages |
| `/boards`, `/board/<slug>` | the list of boards, and one board (see [Boards](boards.md#boards)) |
| `/kanbans`, `/kanban/<slug>`, `/kanban/<slug>/<report>` | the list of kanbans, one kanban, and one shown with another report (see [Kanban](kanban.md#the-page)) |
| `/settings`, `/settings/<tab>` | Settings, and one tab: `general` `device` `appearance` `keys` `taskrc` `tasksh` `kanban` `commands` `hooks` `urgency` |
| `/settings/device` | [this device's own settings](settings.md#this-device) (an older `/settings/device/<tab>` opens it too) |
| `/settings/taskrc/<path>` | one file of the taskrc folder (`/settings/taskrc/holidays/en-US.rc`) |
| `/task/<8 characters>` | a task's detail over the page behind it (the first 8 characters of its uuid) |

An address for a report, board or kanban that doesn't exist says so and opens your home page; a kanban's address while
Kanban is off opens Settings → Kanban. A filter you typed isn't part of
the address.

## Filters and the prompt

Summary, Calendar, Burndown, Projects and Tags have the same filter box as Tasks: Tab completes, filters become chips,
and the filter helpers sit below. It does not take the keyboard when the page opens, so the page's keys work at once:
`f` puts the cursor in it, and Escape takes it out again. Every page except the Console has the prompt in a bar at the bottom. The header also has the running
**timer**, the **bell** (reminders) and **Settings**.

The **last command** appears above the prompt, including what a click ran. Pick a report or sort a column and you can
see the exact command; press **↑** to edit and re-run it.

## Projects and Tags

Both start with your open tasks, waiting ones included. That is three chips in the filter box: `status.not:completed`,
`status.not:deleted` and `status.not:recurring`. They are filters like any other. Take the `completed` or `deleted` chip
away and those tasks come in too, tinted green or red, so a project or tag stays listed after its last task is done.
Any other filter works (`project:Home`, `due.before:eow`, `+work`).

## Long reports

A report of hundreds of tasks takes a while to lay out, so the table draws the first 200 at once and 300 more each time
you scroll near the end, or press **Show more** or **Show all**. The report is still whole: select-all, the count and
the sort cover every task, drawn or not.

## Exporting a table

Under every table (a report page, a report in the console, a table on a board) is a quiet **Export** beside the count. It
exports the tasks that table shows: its filter and report as `<filter> export <report>`, so the report's own filter, sort
and limit apply, with the context the table ran in put in front (`export` itself leaves the context out, as
Taskwarrior's does). The dialog shows the command line it ran, then asks how the file should look:

- **Format**: JSON (the default), CSV or TSV. JSON starts as a list or one task to a line as your taskrc's
  `json.array` says, and you can change it for this file.
- **Fields**: all of them, only the table's columns (`due.relative` is `due`), or the ones you tick.

Each task in the file also has its tracked time, `time_tracking`: the sessions of its time log, each with when it began
and ended, how long it was in seconds and the notes written during it, and their total (`0` when nothing is tracked).
It is one of the fields to tick, and in CSV and TSV its cell is the total in seconds ([details](../terminal.md#tracked-time-in-an-export)).

CSV and TSV have a header of the fields; tags and dependencies are separated by spaces, and annotations by their text,
with `; ` between them. Dates are written `2026-11-19 18:00:00`, in UTC, which a spreadsheet reads as a date in any
locale. CSV keeps a value's line breaks inside quotes (a spreadsheet shows them in one cell); TSV writes a tab, line
break or backslash as `\t`, `\n`, `\\`, so each line is one task for tools that read lines. The file is named for
what it holds and when, as `export` names it (`tasks-next-project-Home-20261116-015300.json`), with `.csv` or `.tsv`.
The browser remembers your last choice of format and fields; the phone and [the terminal app](../terminal.md#export)
have the same dialog, and the terminal app has `meghnaad export` for scripts.

## Tag chips

In the Tasks table, clicking a tag chip toggles `+tag` in that report's filter, as typing it would. A pressed chip is in
the filter; click again to remove it. A second tag joins the first with `or` (`( +a or +b )`, tasks with either), and a
third goes into the same parentheses. Tags you typed side by side, which all have to be present, get the next one added
the same way.

Anywhere else (a report printed in the Console, a task's detail, the Projects page, the Tags page itself) a chip opens
that tag's entry on the **Tags** page, expanded and highlighted. A tag no pending task has still gets an entry, so a
chip from a report of finished tasks works too. **Open in Tasks** on an entry filters the Tasks page by that tag.
