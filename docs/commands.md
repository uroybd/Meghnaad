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
| [`:sor[t]`](#sort) | sort the report |
| [`:d[elete]`](#delete) | delete the task, or the marked ones |
| [`:ma[rk]`](#mark) | mark the task, or unmark it, and move down |
| [`:o[pen]`](#open) | open the task's link |
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
| `:5`, `:$` | a row by its number, the last row |
| `:+3`, `:-3` | three rows down, up |

<a id="tabnext"></a>

## :tabn[ext]

Takes a page's number or name.

With nothing after it, the next page, from the last back to the first. With a number, the page at that place in the top bar (1 to 9, Settings last); with a word, the first page whose name starts with it, in any case: `:tabn 3`, `:tabn tasks`, `:tabn bu`. The keys `gt` and `g1` to `g9` run it.

Since the next release.

<a id="tabprevious"></a>

## :tabp[revious]

The page before this one, from the first round to the last. `gT` runs it.

Since the next release.

<a id="tabNext"></a>

## :tabN[ext]

The same as `:tabprevious`, as in vim.

Since the next release.

<a id="tabfirst"></a>

## :tabfir[st]

The first page of the top bar, Tasks.

Since the next release.

<a id="tabrewind"></a>

## :tabr[ewind]

The same as `:tabfirst`, as in vim.

Since the next release.

<a id="tablast"></a>

## :tabl[ast]

The last page of the top bar, Settings.

Since the next release.

<a id="quit"></a>

## :q[uit]

Quit the terminal app. The web app and the phone have nothing to quit.

Since the next release.

<a id="undo"></a>

## :u[ndo]

Taskwarrior's `undo`, which asks first: on the terminal in a box, on the web with the line put on the command line for Enter.

Since the next release.

<a id="write"></a>

## :w[rite]

Sync, as vim's `:w` writes what has changed: on the terminal `task sync` in the background, with Taskwarrior's own settings; on the web and the phone, with the store. The same as `:sync`.

Since the next release.

<a id="edit"></a>

## :e[dit]

Read the report and the settings again, as vim's `:e` reads the file again. The terminal app reads `settings.json` again too, so a change made in an editor applies.

Since the next release.

<a id="help"></a>

## :h[elp]

The list of keys and commands, as `?` opens it.

Since the next release.

<a id="filter"></a>

## :filt[er]

Takes a filter.

With words, the Tasks page with them as its filter, as `task` reads a filter: `:filter +home due.before:eow`. With none, the cursor in the filter, to change it. Bound to a key, it is a saved filter a key away: `"tasks": { "g h": "filter +home" }`.

Since the next release.

<a id="sort"></a>

## :sor[t]

Takes a sort, as a report's sort setting writes it (due+,urgency-).

On the terminal. With words, the report sorted by them for now, written as `report.<name>.sort` writes it: `:sort due+,urgency-`. With none, the sort editor, to add, take out, reverse and reorder the report's sort keys.

Since the next release.

<a id="delete"></a>

## :d[elete]

Delete the chosen task, or the marked tasks when there are any. Taskwarrior asks first.

Since the next release.

<a id="mark"></a>

## :ma[rk]

Mark the chosen task, or unmark it, and move to the next row. With tasks marked, the task commands act on all of them.

Since the next release.

<a id="open"></a>

## :o[pen]

Open the first link in the chosen task, in its description and then its annotations: on the web in a new tab, on the terminal with the desktop's opener (`open`, or `xdg-open`).

Since the next release.

<a id="done"></a>

## :do[ne]

Mark the chosen task done, or the marked tasks when there are any.

Since the next release.

<a id="start"></a>

## :sta[rt]

Start the chosen task. With tasks marked, start those not started yet and pass over the others.

Since the next release.

<a id="stop"></a>

## :sto[p]

Stop the chosen task. With tasks marked, stop those started and pass over the others.

Since the next release.

<a id="toggle"></a>

## :to[ggle]

Start the chosen task, or stop it when it is started, as `+` does. With tasks marked, use `:start` or `:stop`.

Since the next release.

<a id="modify"></a>

## :mod[ify]

Takes what to change.

With words, `task modify` with them on the marked tasks, or else on the chosen one: `:modify due:eow +next`. With none, the full form.

Since the next release.

<a id="annotate"></a>

## :an[notate]

Takes the note.

With words, add them as a note to the marked tasks, or else to the chosen one: `:annotate called back`. With none, a box for the note.

Since the next release.

<a id="denotate"></a>

## :den[otate]

Takes the note, or the start of it.

With words, take off the note that starts with them, as `task denotate` does. With none, the command line with the task and `denotate` typed, for the note.

Since the next release.

<a id="duplicate"></a>

## :dup[licate]

Taskwarrior's `duplicate` on the chosen task, or the marked ones.

Since the next release.

<a id="info"></a>

## :in[fo]

The chosen task's info: everything about it, its notes and how its urgency adds up.

Since the next release.

<a id="add"></a>

## :ad[d]

Takes the task, as task add takes it.

With words, add the task they describe, as `task add` reads them: `:add Buy milk +errand due:tomorrow`. With none, the box to add one, with a preview.

Since the next release.

<a id="report"></a>

## :rep[ort]

Takes a report's name.

With a name, the Tasks page with that report: `:report list`. With none, the list of reports to choose from.

Since the next release.

<a id="context"></a>

## :con[text]

Takes a context's name, or none.

With a name, Taskwarrior's `context` with it: `:context work`, or `:context none` to have none. With nothing after it, the list of contexts to choose from.

Since the next release.

<a id="sync"></a>

## :sy[nc]

The same as `:write`.

Since the next release.

<a id="review"></a>

## :rev[iew]

tasksh's review: each task the `_reviewed` report lists, one at a time, to mark reviewed, skip, change or finish.

Since the next release.

## Rows

`:5`, `:$`: A number alone goes to that row of the list, as in vim; `:$` goes to the last. A count before `G` or `gg` does the same (`5G`).

`:+3`, `:-3`: A number with `+` or `-` moves that many rows down or up, as `3j` and `3k` do.
