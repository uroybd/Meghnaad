# Commands

<!-- Made by scripts/commands-doc.mjs from web/public/keymaps/commands.json: change that file, not this page. -->

A line of the command line that starts with `:` joined to one of these runs it, in the terminal app and the web
app; any other line goes to `task`. They are vim's where vim has them, and may be written as short as vim allows:
the part in brackets may be left out (`:tabn` for `:tabnext`). A key can run one too, with its words, in place of an
action: `"tasks": { "g h": "filter +home" }` in [settings.json](using/settings.md#keys). The task commands act on
the marked tasks when there are any, and else on the chosen one.

| Command | Does |
| --- | --- |
| [`:tabn[ext]`](#tabnext) | the next page, or a page by its number or name |
| [`:tabp[revious]`](#tabprevious) | the page before |
| [`:tabN[ext]`](#tabNext) | the page before, as :tabprevious |
| [`:tabfir[st]`](#tabfirst) | the first page |
| [`:tabr[ewind]`](#tabrewind) | the first page, as :tabfirst |
| [`:tabl[ast]`](#tablast) | the last page |
| [`:q[uit]`](#quit) | quit |
| [`:u[ndo]`](#undo) | task undo |
| [`:w[rite]`](#write) | sync |
| [`:e[dit]`](#edit) | read everything again |
| [`:h[elp]`](#help) | every key and command, and what each does |
| [`:filt[er]`](#filter) | the Tasks page with a filter |
| [`:sor[t]`](#sort) | change the report's sort |
| [`:d[elete]`](#delete) | delete the task, or the marked ones |
| [`:ma[rk]`](#mark) | mark the task, or unmark it, and move down |
| [`:o[pen]`](#open) | open the task's link, or what is under the cursor |
| [`:do[ne]`](#done) | mark the task done |
| [`:sta[rt]`](#start) | start the task, or the marked ones not started |
| [`:sto[p]`](#stop) | stop the task, or the marked ones started |
| [`:to[ggle]`](#toggle) | start the task, or stop it when it is started |
| [`:mod[ify]`](#modify) | modify the task, or the marked ones |
| [`:an[notate]`](#annotate) | annotate the task, or the marked ones |
| [`:den[otate]`](#denotate) | take a note off the task |
| [`:dup[licate]`](#duplicate) | duplicate the task |
| [`:in[fo]`](#info) | the task's info |
| [`:ad[d]`](#add) | add a task |
| [`:rep[ort]`](#report) | show a report |
| [`:con[text]`](#context) | choose the context |
| [`:sy[nc]`](#sync) | sync |
| [`:rev[iew]`](#review) | review the tasks that need it |
| [`:lock`](#lock) | lock the app: with the PIN where there is one, else with the encryption secret |
| [`:n[ext]`](#next) | the next month, the next board |
| [`:prev[ious]`](#previous) | the month before, the board before |
| [`:sn[ext]`](#snext) | the next section |
| [`:spr[evious]`](#sprevious) | the section before |
| [`:paged[own]`](#pagedown) | a page down |
| [`:pageu[p]`](#pageup) | a page up |
| [`:halfd[own]`](#halfdown) | half a page down |
| [`:halfu[p]`](#halfup) | half a page up |
| [`:pro[mpt]`](#prompt) | the cursor in the prompt |
| [`:si[debar]`](#sidebar) | between the page and the reports down the side |
| [`:hid[den]`](#hidden) | show the hidden reports |
| [`:ba[ck]`](#back) | back |
| [`:stopa[ll]`](#stopall) | stop the started task |
| [`:rem[inders]`](#reminders) | the reminder options |
| [`:ex[port]`](#export) | export as a file |
| [`:cl[ear]`](#clear) | clear the console |
| [`:bu[rndown]`](#burndown) | the burndown by day, week or month |
| [`:com[mand]`](#command) | run one of your task commands on the task |
| [`:key`](#key) | add, take out or search keys |
| [`:ho[me]`](#home) | choose the page the app opens on |
| [`:syncw[hen]`](#syncwhen) | choose when to sync |
| [`:th[eme]`](#theme) | the next theme preset |
| [`:sh[ading]`](#shading) | the grey shading of tasks, or none |
| `:5`, `:$` | a row by its number, the last row |
| `:+3`, `:-3` | three rows down, up |

<a id="tabnext"></a>

## :tabn[ext]

Takes a page's number or name.

With nothing after it, the next page, from the last back to the first. With a number, the page at that place in the top bar (1 to 9, Settings last); with a word, the first page whose name starts with it, in any case: `:tabn 3`, `:tabn tasks`, `:tabn bu`. The keys `gt` and `g1` to `g9` run it.

Since 0.8.11.

<a id="tabprevious"></a>

## :tabp[revious]

The page before this one, from the first round to the last. `gT` runs it.

Since 0.8.11.

<a id="tabNext"></a>

## :tabN[ext]

The same as `:tabprevious`, as in vim.

Since 0.8.11.

<a id="tabfirst"></a>

## :tabfir[st]

The first page of the top bar, Tasks.

Since 0.8.11.

<a id="tabrewind"></a>

## :tabr[ewind]

The same as `:tabfirst`, as in vim.

Since 0.8.11.

<a id="tablast"></a>

## :tabl[ast]

The last page of the top bar, Settings.

Since 0.8.11.

<a id="quit"></a>

## :q[uit]

Quit the terminal app. The web app and the phone have nothing to quit.

Since 0.8.11.

<a id="undo"></a>

## :u[ndo]

Taskwarrior's `undo`, which asks first: on the terminal in a box, on the web with the line put on the command line for Enter.

Since 0.8.11.

<a id="write"></a>

## :w[rite]

Sync, as vim's `:w` writes what has changed: on the terminal `task sync` in the background, with Taskwarrior's own settings; on the web and the phone, with the store. The same as `:sync`.

Since 0.8.11.

<a id="edit"></a>

## :e[dit]

Takes settings.

Read the report and the settings again, as vim's `:e` reads the file again. The terminal app reads `settings.json` again too, so a change made in an editor applies. `:edit settings` opens `settings.json` in `$VISUAL` or `$EDITOR`, on the terminal.

Since 0.8.11.

<a id="help"></a>

## :h[elp]

The list of keys and commands, as `?` opens it.

Since 0.8.11.

<a id="filter"></a>

## :filt[er]

Takes a filter.

With words, the Tasks page with them as its filter, as `task` reads a filter: `:filter +home due.before:eow`. With none, the cursor in the filter, to change it. Bound to a key, it is a saved filter a key away: `"tasks": { "g h": "filter +home" }`. On a page with a filter of its own (the web app's other pages), the cursor goes to that one.

Since 0.8.11.

<a id="sort"></a>

## :sor[t]

Takes fields with +, -, --, = or / after them; or replace and a whole sort.

On the terminal. Each word changes the sort in place, in order: `due+` sorts by due ascending (added last when it is not a key yet), `due-` descending, `due--` takes it out, `due=` turns its direction round, and `due/` groups by it, or stops (`due+/` does both). `:sort due- project--` changes two keys and leaves the rest. `:sort replace due+,urgency-` sets the whole sort, as `report.<name>.sort` writes it. With none, the sort editor; in it, `:sort +` adds a key, and `--`, `=` and `/` do theirs to the key under the cursor, and `raise`, `lower`, `reset`, `apply` and `cancel` the editor's own steps.

Since 0.8.11.

<a id="delete"></a>

## :d[elete]

Takes completed.

Delete the chosen task, or the marked tasks when there are any. Taskwarrior asks first. `:delete completed` deletes the completed tasks of the report.

Since 0.8.11.

<a id="mark"></a>

## :ma[rk]

Takes all or range.

Mark the chosen task, or unmark it, and move to the next row. With tasks marked, the task commands act on all of them. `:mark all` marks every row (or unmarks them all when they are all marked); `:mark range` every row from the last one marked to this one.

Since 0.8.11.

<a id="open"></a>

## :o[pen]

Open the first link in the chosen task, in its description and then its annotations: on the web in a new tab, on the terminal with the desktop's opener (`open`, or `xdg-open`). On a list of projects or tags, of boards, or of the boards in Settings, it opens the one under the cursor.

Since 0.8.11.

<a id="done"></a>

## :do[ne]

Mark the chosen task done, or the marked tasks when there are any.

Since 0.8.11.

<a id="start"></a>

## :sta[rt]

Start the chosen task. With tasks marked, start those not started yet and pass over the others.

Since 0.8.11.

<a id="stop"></a>

## :sto[p]

Stop the chosen task. With tasks marked, stop those started and pass over the others.

Since 0.8.11.

<a id="toggle"></a>

## :to[ggle]

Start the chosen task, or stop it when it is started, as `+` does. With tasks marked, use `:start` or `:stop`.

Since 0.8.11.

<a id="modify"></a>

## :mod[ify]

Takes what to change.

With words, `task modify` with them on the marked tasks, or else on the chosen one: `:modify due:eow +next`. With none, the full form.

Since 0.8.11.

<a id="annotate"></a>

## :an[notate]

Takes the note.

With words, add them as a note to the marked tasks, or else to the chosen one: `:annotate called back`. With none, a box for the note.

Since 0.8.11.

<a id="denotate"></a>

## :den[otate]

Takes the note, or the start of it.

With words, take off the note that starts with them, as `task denotate` does. With none, the command line with the task and `denotate` typed, for the note.

Since 0.8.11.

<a id="duplicate"></a>

## :dup[licate]

Taskwarrior's `duplicate` on the chosen task, or the marked ones.

Since 0.8.11.

<a id="info"></a>

## :in[fo]

Takes a tab: details, recurrence, timelog, annotations, unknown or raw.

The chosen task's info: everything about it, its notes and how its urgency adds up. In the info, a tab by its name: `:info timelog`; `:info raw` is `task info` as `task` prints it.

Since 0.8.11.

<a id="add"></a>

## :ad[d]

Takes the task, as task add takes it.

With words, add the task they describe, as `task add` reads them: `:add Buy milk +errand due:tomorrow`. With none, the box to add one, with a preview. It works from any page.

Since 0.8.11.

<a id="report"></a>

## :rep[ort]

Takes a report's name.

With a name, the Tasks page with that report: `:report list`. With none, the list of reports to choose from. In the terminal's list of reports down the side, the report under the cursor.

Since 0.8.11.

<a id="context"></a>

## :con[text]

Takes a context's name, or none.

With a name, Taskwarrior's `context` with it: `:context work`, or `:context none` to have none. With nothing after it, the list of contexts to choose from.

Since 0.8.11.

<a id="sync"></a>

## :sy[nc]

The same as `:write`.

Since 0.8.11.

<a id="review"></a>

## :rev[iew]

tasksh's review: each task the `_reviewed` report lists, one at a time, to mark reviewed, skip, change or finish.

Since 0.8.11.

<a id="lock"></a>

## :lock

Takes pin or key.

In the static web app. With nothing after it, the PIN screen on a trusted device with a PIN, in every tab, and the four keys of the PIN take it down; on any other device the full lock, as the lock button does, and the encryption secret or a passkey opens it again. `:lock pin` asks for the PIN screen (and says so where there is no PIN), `:lock key` for the full lock. Elsewhere (the web app on a Worker, the terminal) it says there is nothing to lock.

Since 0.8.15.

<a id="next"></a>

## :n[ext]

On the calendar, the month after; on a board, the board after. Like the other commands that act in several places, it does what it means on the page it is used on.

Since 0.8.22.

<a id="previous"></a>

## :prev[ious]

On the calendar, the month before; on a board, the board before.

Since 0.8.22.

<a id="snext"></a>

## :sn[ext]

The next section of the settings, of a board, of the help or of a task's info, as vim's `:snext` is the next file in a split.

Since 0.8.22.

<a id="sprevious"></a>

## :spr[evious]

The section before, as `:snext` goes on.

Since 0.8.22.

<a id="pagedown"></a>

## :paged[own]

A page down the list. A count is that many pages.

Since 0.8.22.

<a id="pageup"></a>

## :pageu[p]

A page up the list.

Since 0.8.22.

<a id="halfdown"></a>

## :halfd[own]

Half a page down the list, as `ctrl+d` does.

Since 0.8.22.

<a id="halfup"></a>

## :halfu[p]

Half a page up the list, as `ctrl+u` does.

Since 0.8.22.

<a id="prompt"></a>

## :pro[mpt]

Put the cursor in the prompt, the command line at the bottom, as `/` does.

Since 0.8.22.

<a id="sidebar"></a>

## :si[debar]

On the terminal, move between the page and the list of reports down the side.

Since 0.8.22.

<a id="hidden"></a>

## :hid[den]

In the terminal's list of reports, show the hidden ones too (their names start with `_`), or hide them again.

Since 0.8.22.

<a id="back"></a>

## :ba[ck]

Back to the page from the reports down the side, or from a board to the list of boards.

Since 0.8.22.

<a id="stopall"></a>

## :stopa[ll]

Stop the task that is started, whichever it is (the timer in the top bar), where `:stop` stops the chosen one.

Since 0.8.22.

<a id="reminders"></a>

## :rem[inders]

The reminder options, and turning reminders on or off.

Since 0.8.22.

<a id="export"></a>

## :ex[port]

Export the report as a file (JSON, CSV or TSV, with the fields you choose); on a board, a table of the board.

Since 0.8.22.

<a id="clear"></a>

## :cl[ear]

Clear what the console shows.

Since 0.8.22.

<a id="burndown"></a>

## :bu[rndown]

Takes day, week or month.

The burndown chart by day, by week or by month: `:burndown week`.

Since 0.8.22.

<a id="command"></a>

## :com[mand]

Takes add, edit or delete.

With nothing after it, one of your task commands to run on the chosen task, from a list. In the task commands of Settings, `:command add`, `:command edit` and `:command delete` add one, change the one under the cursor and delete it.

Since 0.8.22.

<a id="key"></a>

## :key

Takes add, remove, none, defaults or search.

In the Keybindings of Settings: `:key add` adds a key to the section, `:key remove` takes your key out (back to the default), `:key none` leaves the action without a key, `:key defaults` shows the default keys or only yours, `:key search` searches them.

Since 0.8.22.

<a id="home"></a>

## :ho[me]

In the general settings, choose the page the app opens on: a report or a board.

Since 0.8.22.

<a id="syncwhen"></a>

## :syncw[hen]

In the sync settings, go through when to sync, and save it.

Since 0.8.22.

<a id="theme"></a>

## :th[eme]

In the appearance settings, go through the presets: auto, light, dark or mono.

Since 0.8.22.

<a id="shading"></a>

## :sh[ading]

In the appearance settings, draw tasks' grey shading, or leave it out.

Since 0.8.22.

## Rows

`:5`, `:$`: A number alone goes to that row of the list, as in vim; `:$` goes to the last. A count before `G` or `gg` does the same (`5G`).

`:+3`, `:-3`: A number with `+` or `-` moves that many rows down or up, as `3j` and `3k` do.
