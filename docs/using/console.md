# The console

Commands follow Taskwarrior: `[filter] command [modifications]`, and the leading `task` is optional.

```
next                                   # a report (built-in, or defined in your taskrc)
project:Home +errand due.before:eow list
add Pay rent project:Home due:eom +bills
3 modify due:2026-12-25T08:30 priority:H
3 start        3 stop        3 done        3 delete        undo
3 annotate called the landlord
estimate:big count                     # UDAs work like any attribute
rc.report.next.sort:due-,project+ next # per-command overrides, as in Taskwarrior
calc 2 days + 3 hours
help
```

| | |
| --- | --- |
| **Write** | `add` `log` `duplicate` `import` `purge` `modify` `done` `delete` `start` `stop` `annotate` `denotate` `append` `prepend` `undo` |
| **Read** | `info` (or `information`) `count` `projects` `tags` `summary` `calendar` `burndown.daily` `.weekly` `.monthly` `.annual` `history.daily` `.weekly` `.monthly` `.annual` `ghistory.daily` `.weekly` `.monthly` `.annual` `timesheet` `stats` `udas` `columns` `reports` `context` `show` `config` `export` `ids` `uuids` `calc` `commands` `_get` `_projects` `_tags`, and the other lists Taskwarrior makes for shell completion: `_commands` `_aliases` `_columns` `_config` `_context` `_ids` `_show` `_udas` `_unique <attribute>` `_urgency` `_uuids` `_version` `_zshids` `_zshuuids` `_zshcommands` `_zshattributes`. `ids` squeezes runs into ranges (`1 3-5`) and `uuids` is one line |
| **Filters** | `attr:value` with modifiers (`.is .not .has .startswith .before .after .by .none .any …`), or compared as `due < eom`, `urgency>5`, `project != Home`, `description ~ Pay` (`= == != !== < <= > >= ~ !~`, with or without spaces, and a value that begins with `^`, `*` or `%` is an arithmetic operator to Taskwarrior's lexer, so `description ~ ^Pay` fails as it does there: anchor with `description.startswith:Pay`; `priority` goes H, M, L and a task with none is below them all, or as the taskrc's `uda.priority.values` has them: with `H,M,,L` a task with none is between M and L, and with a list that has no blank one with none is above them all); `+tag` / `-tag`; virtual tags (`+OVERDUE +DUETODAY +READY +ACTIVE +BLOCKED …`); plain words and `/pattern/`; ids (`3`, `1-4,7`) and uuid prefixes (several side by side, `1 3` or two uuids, pick all of them, and the rest of the filter narrows that group); `and` `or` `xor` and parentheses, `!(…)` to reverse a group. As in Taskwarrior, `tags` is one string (sorted, joined by commas: `tags:a` is the start of it and `tags.is:a,b` all of it; ask for a tag with `+a`), `not` is a word to find, and a `!` with no parenthesis takes only the value next to it: before it (`! project:Work` is false for every task) or after it (`project:Work !` is `project:false`, `+a !` the tag `false`, `project !` the tasks with no project; after a group or a task number it reverses it). A word is broken at every parenthesis (`not(+a)` is `not` and `(+a)`). A comparison may have arithmetic and attributes on either side, worked out by `calc`'s evaluator for each task (`due < now + 3d`, `urgency * 2 > 10`, `now - entry < 1h`, `( urgency + 1 ) * 2 > 5`; glued, `urgency*2>10`): an attribute name stands for what the task holds, a task with no date is on neither side of a comparison with one, and what cannot be worked out (`due - now` on a task with no due) stops the command with Taskwarrior's message. `=~` is no operator there, as in Taskwarrior |
| **Dates** | `today tomorrow eow som eoy monday 3d 2w`, `2026-12-25`, `2026-12-25T08:30`, `now+2h`, and anything your `dateformat` describes (`12/25/2026` with `m/d/Y`). Also as Taskwarrior reads them: the start and end of the previous, this and next day, week, work week, month, quarter and year (`sopw eonm soq eoww`), `later` and `someday`, month names (`jan`) and any three letters of a day, `22nd`, `10am` or `14:30`, ISO weeks (`2026-W44`) and days of the year (`2026-300`), `easter` `goodfriday` `eastermonday` `ascension` `pentecost` `midsommar` |

**History and the timesheet** answer in the Console, and typed in the bar under another page they switch to it.
`history.monthly` (also `.daily`, `.weekly`, `.annual`) counts the tasks added, completed and deleted in each period, with
the net change and the averages; `ghistory.*` draws the same as a bar of `+` (added), `X` (completed) and `-` (deleted),
80 columns wide. A recurring template is not counted as added, and a filter narrows them (`project:Home history.monthly`).
`timesheet` lists what was completed and started, by week and day; with no filter it covers the last four weeks, and
`report.timesheet.filter` changes that. `information` is `info` under its full name.

**`export` gives you a file.** It prints a summary and a **Download** button; the file is Taskwarrior's own JSON, the
same as `task export` (ids and urgency included), so `task import` reads it. It takes a filter (`project:Home export`) and
a report's name (`export next` uses that report's filter and sort), and `json.array=off` makes it one task per line.

**`modify` and `add` take `tags:` and `status:` too.** `tags:a,b` makes those the task's tags and no others (`tags:` none), and
`status:completed` or `status:deleted` finishes the task, with an `end` of now, and `status:pending` takes it back. An `end:`
on a task that is still pending is refused (`modify`) or left out (`add`), as in Taskwarrior; `rtype:`, `mask:`, `imask:`,
`uuid:`, `id:` and `urgency:` are refused with "The 'rtype' attribute does not allow a value of 'periodic'". A dependency that is
taken away (`depends:-3`) has to be one the task has. A tag is kept under any name, as in Taskwarrior (`+URGENT`, `+a:b`, `+Mixed`;
names are case-sensitive, and a name that is also a virtual tag, such as `+PENDING`, is stored but a filter reads the virtual one).
Not done: an empty description is refused.

**Changes beside a command.** What follows `done`, `delete`, `start`, `stop`, `annotate`, `append` and `prepend` can carry
changes, as in Taskwarrior: `3 done end:-2h` (finished two hours ago), `3 start due:eow +urgent`,
`3 annotate called her due:friday`. Attributes, tags and substitutions are applied to the task; the plain words left over
are the annotation (or, for `annotate`, `append` and `prepend`, the text). The buttons send their text literally, so a
note that starts with `due:` stays a note.

**`purge`** removes deleted tasks for good: `purge status:deleted`, or `purge project:Old` (a task has to be deleted
first; anything else is skipped with a hint). It always asks about each task ("Permanently remove task 3 'x'?"),
like `delete`, and with no filter it asks as every other command does, or refuses under `allow.empty.filter=off`.
Tasks that waited on a purged task stop waiting on it. A recurring template can only be purged once its instances are
deleted, and it takes them with it, after a question that follows `recurrence.confirmation` (`no` refuses). `undo`
brings a purged task back with all it had; the Console remembers only the last few commands, and not across a restart.

**`log` and `duplicate`.** `log Paid the invoice project:Home` records a task that is already finished: its `end` is
the moment you enter it, and it refuses `recur:` and `wait:`, which a finished task has no use for. `3 duplicate` makes
a copy of task 3 as a new pending task: it keeps everything but its identity, `start`, `end` and `entry`. What follows
`duplicate` is applied to the copy (`3 duplicate due:eow +again`), and plain words become a note on it. Copying a
recurring instance gives an ordinary task; copying a template gives another template. Both write like any other command,
so they ask first at `bulk` tasks, and a `duplicate` with no filter asks as the others do.

**`commands`** lists every command and report in a table (its category, whether it writes, and what it takes: a
filter, modifications, other words), so it shows what this app understands, which is a little less than `task`. A
report you define in the taskrc is there too. **`_get`** prints the value of each DOM reference it is given, separated
by spaces: `_get 1.due 1.project rc.bulk tw.version`. A reference with no value prints as nothing; a word that is not a
reference is an error. It reads the same references as `calc` (see below), and credentials are never among them.

**`import`** brings tasks in from a file, such as one `export` wrote (or `task export` on your desktop). Typing it
shows a **Choose a file…** button; the file is read in the browser and checked by the server, which then says how many
tasks are new, how many would change and how many are already there, and lists them. Nothing is written until you press
**Import**, and then all of it is or none, as one `undo` step. A task is matched by its `uuid`: new ones are added, one
that differs takes the file's attributes whole (what the file leaves out is removed, as in Taskwarrior), and one that is
the same is left alone, so importing the same file twice changes nothing. A file with a bad task (an invalid date or uuid,
an unknown status, malformed annotations) is refused with the task's number and the reason. A file may be one task, an
array, or a task to a line; the most is about 3 MB (a few thousand tasks) or 5,000 tasks at a time, so split a bigger one.
Missing `entry`, `end` and `modified` are filled in, `default.project` and the UDA defaults apply to what lacks them, and
hooks (`on_add`, `on_modify`) run for each task. `import-v2` (Taskwarrior 2's data files) is not in the app.

**`stats`** counts the tasks that match a filter (all of them, with none): by status, annotations, tags, projects,
blocked and blocking, how old the oldest is and how often tasks are added and finished. The two rows about undo and the
sync backlog describe this page: `undo` is what it remembers, and the backlog is normally 0, since every write is pushed at once (it is more while you are offline).

**Contexts.** `context define work +work or project:Work` saves a context, `context work` makes it the active one,
`context none` turns it off, `context delete work` removes it, and `context` (or `context list`) lists them and
`context show` says which one is in force (the filter bar above a table names it too). They edit the saved settings,
the same ones the taskrc tab and `config` do, so the pages follow at once. Defining asks first, as in Taskwarrior, when its filter
matches no pending task, and deleting asks when `confirmation` is on. Tab completes `context` too: what it can do (`list`, `show`, `none`, `define`, `delete`) and the contexts you have defined, and
`rc.context:` the same way. A filter that can't also be what new tasks get
(one with `or`, a `-tag` or a modifier such as `.before`) makes a read-only context, and says so.
Put the whole filter in quotes (`context define this_week "+WEEK or (due.after:sow and due.before:eow)"`) and it is read as
one filter, as Taskwarrior reads it, unless it starts with a `name:value`, which takes all of it.

The active context is the console's: it sits above the prompt (`context work`, with a ✕ that runs `context none`), and every
command you type runs in it. While a context is active the pill in the top bar, the Run button and the prompt are amber,
and they turn back when it is cleared. With room the prompt names the context (`task[work]>`); on a phone it stays
`task>` and the context is in a tooltip (a long press). The pages follow it unless told otherwise. The Tasks page, Summary, Calendar, Burndown,
Projects and Tags have a **Context** menu of their own: *same as console* (the default), *none*, or any context. A page's
choice is only for that page (it is remembered on this device, not synced), so `context work` in the console does not
change a page that chose *none*. The choice is sent with the page's commands as Taskwarrior's own `rc.context=<name>`.
A board has no menu: it follows the console's context unless its filter names one. Put `rc.context=this_week` in the board's
filter (in its settings) to make that its default (a card that ignores the board's filter ignores that too), or in a card's
filter to give that card its own; a card's
**Information** names the context it ran in. The **Filter** above a board (collapsed until you open it) holds the board's own
filter; what you type there replaces it while you look at the board, and nothing is saved (**Reset** brings the board's own
back). New tasks (the **+** and the console's `add`) still take the *active* context's write rule.

**Where a report prints.** Typed in the Console, a report (`list`, `next`, …) prints there. Typed in the bar under any
other page, it opens in the Tasks view, where it can be sorted and filtered.

**Keys:** `/` focuses the prompt · `Tab` completes (and cycles a menu; `Shift+Tab` goes back) · `↑`/`↓` the last 20
commands · `Esc` closes a menu · `Ctrl+L` clears the console.

**The prompt shows what it is reading.** The command is bold, `name:value` attributes are purple, `+tag` green, ids orange,
`and` / `or` and brackets red, `rc.` settings grey and italic, and quoted text its own colour. After `add` and `log` the rest is
a description, so `and` there is only a word. A command that is longer than the line wraps, so all of it can be read.
Suggestions open as you type, under the filter boxes and above the prompt; Tab or a tap takes one.
They know where the word is: a command is said once, so with `add`, `modify` or a report already in the line (before the word or after it) only attributes are offered,
not `calendar`, `summary` or another report; and a bare number is a task id only in front of the command, not in a
description that follows it.

**Abbreviations and aliases.** A command, report or attribute name may be shortened to `abbreviation.minimum`
characters (default 2): `proj:Home`, `ann 3 text`. `alias.<name>=<words>` in the taskrc makes a typed word stand for
others; Taskwarrior's own `rm` (delete) and `burndown` (burndown.weekly) work without one. Only what you type is
expanded, never what the buttons send.

**Several commands on one line.** Like a shell, the console runs `a && b` (`b` only if `a` worked), `a || b` (`b`
only if `a` failed) and `a ; b` (always), left to right with no precedence, so `a && b || c` is `(a && b) || c`. Each
command draws its answer in the console under one heading, the line as you typed it, and the prompt is back after the
last one. A command that asks a question
(a delete, say) waits for the answer before the line goes on; "no" counts as a failure. Clearing the console ends the line.
An error is a failure, and so is a command that could not be reached. This is the page's, as it is a shell's: the engine
is sent one command at a time, so these are not Taskwarrior syntax. An operator that is not quoted always separates
commands, in a filter too, so write `or` and `and` there (as Taskwarrior documents them), or quote it (`'||'`, `\|\|`).
`add 'Fish & chips; or pizza'` keeps its `;`. `review` has to be on a line of its own. Task commands (below) may use
them too.

**Text is matched as regular expressions**, as in Taskwarrior (`regex=1`): plain words, `/pattern/`, `.has`,
`.startswith`, `.endswith`, `.word`, and the `/from/to/` substitution in `modify`. A pattern is searched for anywhere in
the text, and for the description the annotations are searched too. Quote one that has spaces (`'/buy.*milk/'`).
Taskwarrior reads these with C++ `std::regex` (ECMAScript, byte by byte) and so does this app: `\w`, `\d` and
case-insensitive matching are ASCII only. Lookahead and backreferences are refused with a message, as is syntax
ECMAScript doesn't have (`(?i)`). `regex=off` goes back to plain text.

**`calc`** is Taskwarrior's calculator: numbers, durations, dates, strings, `and`/`or`, comparisons, and references
such as `1.due - now`, `1.tags.x`, `rc.bulk` or `system.version`. `expressions=postfix` makes it read `2 days 3 hours +`.
Numbers and dates are read as Taskwarrior's lexer reads them (`1e3`, `5.`, `2023-W01-3`, `2023-100`, `2023-10`; a whole
number from 315532800 to the year 9999 is an epoch time), and anything else that starts like a number (`3x`, `1,5`) is a
word, printed as typed (so are integers joined by `-` or `,` with nothing between them, `5-3` and `1,2`, and `/ 10 /`). `rc.<setting>` and `_get rc.<setting>` answer with Taskwarrior's default when the taskrc is silent.
A date in a filter may be an expression too: `due.before:(now+2d)`, `due.after:eow-1d+1h`.

**`projects` and `tags`** list what is pending, with a project's count including its sub-projects.
`list.all.projects` and `list.all.tags` add finished tasks; `complete.all.tags` does the same for tag completion.

**`show` and `config`** read and change the settings the app keeps, and work **only in the Console** (typed in the
bar under another page, they switch you to it). `show` lists every setting, defaults included, with the ones you
changed highlighted and their default beneath; `show weekstart` narrows it to names containing that text. `config
weekstart monday` sets one (several words become one value, `config name ""` blanks it), and `config weekstart` with no
value removes it, putting the default back. With `confirmation` on it asks first, as Taskwarrior does. The change is saved
in your bucket just like an imported taskrc, and is the same list the **taskrc** tab shows. Both follow the
[same rules as an import](settings.md#your-taskrc): sync and credential-like names are refused, never echoed or shown, and a name the
app doesn't read, or a value it can't accept (`weekstart someday`), is refused with the reason instead of being kept.

**Your own commands.** A word you defined under Settings, then Commands (`fail` for `done outcome:fail`) works at this
prompt too, with or without `task` and after a filter: see [Commands](settings.md#commands).

## Questions Taskwarrior asks

The same questions as on the desktop, from the same settings, but answered on the page.

- **`delete` and `undo` ask** (`confirmation`, on by default).
- **A change to `bulk` tasks or more asks** (default 3), whatever the command.
- **A command with no filter asks** before it changes every task, finished and deleted ones included
  (`allow.empty.filter`).
- **Finishing or deleting a task in the middle of a dependency chain** offers to repair the chain
  (`dependency.confirmation`): what waited on it then waits on what it waited on.
- **Editing, appending to, prepending to, annotating or deleting a task of a recurring series** asks whether to do the
  same to the rest of the series (`recurrence.confirmation`).

Taskwarrior asks yes / no / all / quit about each task in turn. Here the questions arrive together as a **table with a
tick for each task the command would change**: tick all, none, or the ones you want, then go ahead. Only tasks the
command would actually change are listed. A single task is a plain yes/no. The delete buttons ask by turning into
"sure?" on the first click.
