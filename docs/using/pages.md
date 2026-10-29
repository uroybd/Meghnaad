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
| **Console** | Type commands; reports and answers print here |

## Addresses

Every page has an address. You can share a link with anyone who can sign in to the same deployment, open it in a new
tab, and the browser's back button goes where you were.

| Address | Page |
| --- | --- |
| `/` | Your **home page** (Settings → General): a report or a board, `next` if you haven't chosen |
| `/report/<name>` | Tasks, as that report: `/report/next`, `/report/list`, and any report of your own by name |
| `/projects` `/tags` `/summary` `/calendar` `/burndown` `/console` | those pages |
| `/boards`, `/board/<slug>` | the list of boards, and one board (see [Boards](boards.md#boards)) |
| `/settings`, `/settings/<tab>` | Settings, and one tab: `general` `taskrc` `tasksh` `commands` `hooks` `urgency` |
| `/settings/taskrc/<path>` | one file of the taskrc folder (`/settings/taskrc/holidays/en-US.rc`) |
| `/task/<8 characters>` | a task's detail over the page behind it (the first 8 characters of its uuid) |

An address for a report or board that doesn't exist says so and opens your home page. A filter you typed isn't part of
the address.

## Filters and the prompt

Summary, Calendar, Burndown, Projects and Tags have the same filter box as Tasks: Tab completes, filters become chips,
and the filter helpers sit below. It is focused when the page opens (not on a phone, where that would raise the
keyboard). Every page except the Console has the prompt in a bar at the bottom. The header also has the running
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

## Tag chips

In the Tasks table, clicking a tag chip toggles `+tag` in that report's filter, as typing it would. A pressed chip is in
the filter; click again to remove it. A second tag joins the first with `or` (`( +a or +b )`, tasks with either), and a
third goes into the same parentheses. Tags you typed side by side, which all have to be present, get the next one added
the same way.

Anywhere else (a report printed in the Console, a task's detail, the Projects page, the Tags page itself) a chip opens
that tag's entry on the **Tags** page, expanded and highlighted. A tag no pending task has still gets an entry, so a
chip from a report of finished tasks works too. **Open in Tasks** on an entry filters the Tasks page by that tag.
