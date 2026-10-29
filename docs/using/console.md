# The console

Commands follow Taskwarrior: `[filter] command [modifications]`. The leading `task` is optional.

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

**Keys:** `/` focuses the prompt · `Tab` completes (and cycles a menu; `Shift+Tab` goes back) · `↑`/`↓` the last 20
commands · `Esc` closes a menu · `Ctrl+L` clears the console.

**Where a report prints.** Typed in the Console, a report (`list`, `next`, …) prints there. Typed in the bar under
another page, it opens in the Tasks view, where it can be sorted and filtered. History, the timesheet, `show` and
`config` always answer in the Console, so typing them elsewhere switches to it.

## Commands

| | |
| --- | --- |
| **Write** | `add` `log` `duplicate` `import` `purge` `modify` `done` `delete` `start` `stop` `annotate` `denotate` `append` `prepend` `undo` |
| **Read** | `info` (or `information`) `count` `projects` `tags` `summary` `calendar` `burndown.daily` `.weekly` `.monthly` `.annual` `history.*` `ghistory.*` (same four periods) `timesheet` `stats` `udas` `columns` `reports` `context` `show` `config` `export` `ids` `uuids` `calc` `commands` `_get` `_projects` `_tags`, and the lists Taskwarrior makes for shell completion: `_commands` `_aliases` `_columns` `_config` `_context` `_ids` `_show` `_udas` `_unique <attribute>` `_urgency` `_uuids` `_version` `_zshids` `_zshuuids` `_zshcommands` `_zshattributes`. `ids` squeezes runs into ranges (`1 3-5`); `uuids` is one line |

**`commands`** lists every command and report with its category, whether it writes, and what it takes. It shows what this
app understands, which is a little less than `task`; your own reports are there too. **`_get`** prints the value of each
DOM reference it is given: `_get 1.due 1.project rc.bulk tw.version`. A reference with no value prints nothing, a word
that isn't a reference is an error, and credentials are never among them.

**Abbreviations and aliases.** A command, report or attribute name can be shortened to `abbreviation.minimum`
characters (default 2): `proj:Home`, `ann 3 text`. `alias.<name>=<words>` in the taskrc makes a typed word stand for
others. Taskwarrior's own `rm` (delete) and `burndown` (burndown.weekly) work without one. Only what you type is
expanded, never what the buttons send.

**Your own commands.** A word defined under Settings → Commands (`fail` for `done outcome:fail`) works at this prompt
too, with or without `task`, and after a filter. See [Commands](settings.md#commands).

## Filters

`attr:value` with modifiers (`.is .not .has .startswith .before .after .by .none .any …`), or compared as `due < eom`,
`urgency>5`, `project != Home`, `description ~ Pay` (`= == != !== < <= > >= ~ !~`, with or without spaces).

- **Tags and words:** `+tag` / `-tag`, virtual tags (`+OVERDUE +DUETODAY +READY +ACTIVE +BLOCKED …`), plain words and
  `/pattern/`.
- **Ids:** `3`, `1-4,7` and uuid prefixes. Several side by side (`1 3`, or two uuids) pick all of them, and the rest of
  the filter narrows that group.
- **Logic:** `and`, `or`, `xor`, parentheses, and `!(…)` to reverse a group.
- **Arithmetic:** a comparison may have arithmetic on either side (see below).
- **Priority** goes H, M, L, and a task with none is below them all. If the taskrc's `uda.priority.values` says otherwise
  it follows that: with `H,M,,L` a task with none sits between M and L, and with no blank one it is above them all.

**Arithmetic in a comparison** is worked out by `calc`'s evaluator for each task: `due < now + 3d`, `urgency * 2 > 10`,
`now - entry < 1h`, `( urgency + 1 ) * 2 > 5` (glued too: `urgency*2>10`). An attribute stands for what the task holds,
and a task with no date is on neither side of a comparison with one. What can't be worked out (`due - now` with no due)
stops the command with Taskwarrior's message.

As in Taskwarrior, and so the same here:

- `tags` is one string (sorted, joined by commas): `tags:a` is the start of it and `tags.is:a,b` all of it. Ask for a tag
  with `+a`.
- A value that begins with `^`, `*` or `%` is an arithmetic operator to Taskwarrior's lexer, so `description ~ ^Pay`
  fails. Use `description.startswith:Pay`.
- `not` is a word to find. A `!` with no parenthesis takes only the value next to it: `! project:Work` is false for every
  task, `project:Work !` is `project:false`, `+a !` the tag `false`, and `project !` the tasks with no project. After a
  group or a task number it reverses it.
- A word is broken at every parenthesis (`not(+a)` is `not` and `(+a)`). `=~` is not an operator.

**Dates:** `today tomorrow eow som eoy monday 3d 2w`, `2026-12-25`, `2026-12-25T08:30`, `now+2h`, and anything your
`dateformat` describes (`12/25/2026` with `m/d/Y`). Also as Taskwarrior reads them: the start and end of the previous,
this and next day, week, work week, month, quarter and year (`sopw eonm soq eoww`), `later` and `someday`, month names
(`jan`), any three letters of a day, `22nd`, `10am`, `14:30`, ISO weeks (`2026-W44`), days of the year (`2026-300`), and
`easter` `goodfriday` `eastermonday` `ascension` `pentecost` `midsommar`. A date in a filter may be an expression too:
`due.before:(now+2d)`, `due.after:eow-1d+1h`.

**Text is matched as regular expressions**, as in Taskwarrior (`regex=1`): plain words, `/pattern/`, `.has`,
`.startswith`, `.endswith`, `.word`, and the `/from/to/` substitution in `modify`. A pattern is searched for anywhere in
the text, and for the description the annotations too. Quote one with spaces (`'/buy.*milk/'`). Taskwarrior reads these
with C++ `std::regex` (ECMAScript, byte by byte) and so does this app, so `\w`, `\d` and case-insensitive matching are
ASCII only. Lookahead, backreferences and syntax ECMAScript lacks (`(?i)`) are refused with a message. `regex=off` goes
back to plain text.

## Changing tasks

**`modify` and `add` take `tags:` and `status:` too.** `tags:a,b` makes those the task's tags and no others (`tags:`
none). `status:completed` or `status:deleted` finishes the task with an `end` of now, and `status:pending` takes it back.
`end:` on a pending task is refused (`modify`) or left out (`add`), as in Taskwarrior. `rtype:`, `mask:`, `imask:`,
`uuid:`, `id:` and `urgency:` are refused. A dependency you take away (`depends:-3`) must be one the task has. A tag is
kept under any name (`+URGENT`, `+a:b`, `+Mixed`; case-sensitive), and one that is also a virtual tag such as `+PENDING`
is stored, but a filter reads the virtual one. An empty description is refused.

**Changes beside a command.** What follows `done`, `delete`, `start`, `stop`, `annotate`, `append` and `prepend` can
carry changes: `3 done end:-2h` (finished two hours ago), `3 start due:eow +urgent`, `3 annotate called her due:friday`.
Attributes, tags and substitutions are applied to the task, and the plain words left over are the annotation (or, for
`annotate`, `append` and `prepend`, the text). The buttons send their text literally, so a note that starts with `due:`
stays a note.

**`log` and `duplicate`.** `log Paid the invoice project:Home` records a task that is already finished: its `end` is the
moment you enter it, and `recur:` and `wait:` are refused. `3 duplicate` copies task 3 as a new pending task, keeping
everything but its identity, `start`, `end` and `entry`. What follows is applied to the copy (`3 duplicate due:eow
+again`), with plain words becoming a note. Copying a recurring instance gives an ordinary task; copying a template
gives another template. Both ask first at `bulk` tasks, like any write.

**`purge`** removes deleted tasks for good: `purge status:deleted`, or `purge project:Old`. A task has to be deleted first;
anything else is skipped with a hint. It asks about each task ("Permanently remove task 3 'x'?"), and with no filter it
asks as other commands do, or refuses under `allow.empty.filter=off`. Tasks that waited on a purged task stop waiting. A
recurring template can be purged only once its instances are deleted, and takes them with it, after a question that
follows `recurrence.confirmation` (`no` refuses). `undo` brings a purged task back with all it had; the Console
remembers only the last few commands, and not across a restart.

**`import`** reads tasks from a file, such as one `export` wrote (or `task export` on your desktop).

1. Typing it shows **Choose a file…**. The browser reads the file and the server checks it, then says how many tasks are
   new, how many would change and how many are already there, and lists them.
2. Nothing is written until you press **Import**. Then all of it is, or none, as one `undo` step.
3. Tasks are matched by `uuid`. New ones are added, a different one takes the file's attributes whole (what the file
   leaves out is removed, as in Taskwarrior), and an identical one is left alone, so importing a file twice changes
   nothing.

A file with a bad task (an invalid date or uuid, an unknown status, malformed annotations) is refused with the task's
number and the reason. A file may be one task, an array, or a task per line. The limit is about 3 MB (a few thousand
tasks) or 5,000 tasks at a time, so split a bigger one. Missing `entry`, `end` and `modified` are filled in,
`default.project` and UDA defaults apply to what lacks them, and hooks run for each task. `import-v2` isn't in the app.

## Reading and reports

**`export` gives you a file.** It prints a summary and a **Download** button. The file is Taskwarrior's own JSON, as
`task export` writes it (ids and urgency included), so `task import` reads it. It takes a filter (`project:Home export`)
and a report's name (`export next` uses that report's filter and sort), and `json.array=off` makes it one task per line.

**History and the timesheet.** `history.monthly` (also `.daily`, `.weekly`, `.annual`) counts the tasks added, completed
and deleted in each period, with the net change and averages. `ghistory.*` draws the same as a bar of `+` (added), `X`
(completed) and `-` (deleted), 80 columns wide. A recurring template isn't counted as added, and a filter narrows them
(`project:Home history.monthly`). `timesheet` lists what was completed and started, by week and day; with no filter it
covers the last four weeks, and `report.timesheet.filter` changes that.

**`stats`** counts the tasks that match a filter (all, with none): by status, annotations, tags, projects, blocked and
blocking, the age of the oldest, and how often tasks are added and finished. The two rows about undo and the sync backlog
describe this page: `undo` is what it remembers, and the backlog is normally 0, since every write is pushed at once (it
grows while you're offline).

**`projects` and `tags`** list what is pending, with a project's count including its sub-projects. `list.all.projects`
and `list.all.tags` add finished tasks, and `complete.all.tags` does the same for tag completion.

**`calc`** is Taskwarrior's calculator: numbers, durations, dates, strings, `and`/`or`, comparisons, and references such
as `1.due - now`, `1.tags.x`, `rc.bulk` or `system.version`. `expressions=postfix` makes it read `2 days 3 hours +`.
Numbers and dates are read as Taskwarrior's lexer reads them (`1e3`, `5.`, `2023-W01-3`, `2023-100`, `2023-10`; a whole
number from 315532800 to the year 9999 is an epoch time). Anything else that starts like a number (`3x`, `1,5`) is a
word, printed as typed, and so are integers joined by `-` or `,` (`5-3`, `1,2`) and `/ 10 /`. `rc.<setting>` and `_get
rc.<setting>` answer with Taskwarrior's default when the taskrc is silent.

**`show` and `config`** read and change the settings the app keeps, and work **only in the Console**.

- `show` lists every setting, defaults included, with your changes highlighted and the default beneath. `show weekstart`
  narrows it to names containing that text.
- `config weekstart monday` sets one (several words become one value; `config name ""` blanks it). `config weekstart`
  with no value removes it, bringing the default back. With `confirmation` on it asks first.
- The change is saved in your bucket like an imported taskrc, and is the list the **taskrc** tab shows. Both follow the
  [same rules as an import](settings.md#your-taskrc): sync and credential-like names are refused and never echoed, and
  a name the app doesn't read, or a value it can't accept (`weekstart someday`), is refused with the reason.

## Contexts

`context define work +work or project:Work` saves a context. `context work` makes it active, `context none` turns it
off, `context delete work` removes it, `context` (or `context list`) lists them, and `context show` says which is in
force. They edit the same saved settings as the taskrc tab and `config`, so the pages follow at once. Defining asks first
when its filter matches no pending task, and deleting asks when `confirmation` is on. Tab completes the subcommands and
your context names, and `rc.context:` the same way.

A filter that can't also serve as what new tasks get (one with `or`, a `-tag` or a modifier such as `.before`) makes a
read-only context, and says so. Put the whole filter in quotes (`context define this_week "+WEEK or (due.after:sow and
due.before:eow)"`) to read it as one filter, as Taskwarrior does, unless it starts with a `name:value`, which takes all of it.

**The active context is the console's.** It sits above the prompt (`context work`, with a ✕ that runs `context none`),
and every command runs in it. While one is active, the pill in the top bar, the Run button and the prompt are amber.
The prompt names it (`task[work]>`) when there's room; on a phone it stays `task>` and a long press shows it.

**Pages.** Tasks, Summary, Calendar, Burndown, Projects and Tags have a **Context** menu: *same as console* (default),
*none*, or any context. The choice is for that page only (kept on this device, not synced), so `context work` in the
console doesn't change a page that chose *none*. It is sent with the page's commands as `rc.context=<name>`.

**Boards.** A board has no menu: it follows the console's context unless its filter names one. Put
`rc.context=this_week` in the board's filter to make that its default (a card that ignores the board's filter ignores that
too), or in a card's filter for that card alone. A card's **Information** names the context it ran in. The **Filter**
above a board (collapsed until you open it) holds the board's own filter; what you type there replaces it while you look,
nothing is saved, and **Reset** brings the board's own back.

New tasks (the **+** and `add`) still take the *active* context's write rule.

## The prompt

### Colours

The command is bold, `name:value` attributes are purple, `+tag` green, ids orange, `and`/`or` and
brackets red, `rc.` settings grey italic, and quoted text its own colour. After `add` and `log` the rest is a
description, so `and` there is only a word. A long command wraps, so all of it can be read.

### Suggestions

Suggestions open as you type, under the filter boxes and above the prompt; Tab or a tap takes one. They know where
the word is: a command is said once, so with `add`, `modify` or a report already in the line only attributes are offered
(not `calendar`, `summary` or another report), and a bare number is a task id only in front of the command, not in a
description that follows it.

## Several commands on one line

Like a shell, the console runs `a && b` (`b` only if `a` worked), `a || b` (`b` only if `a` failed) and `a ; b` (always),
left to right with no precedence, so `a && b || c` is `(a && b) || c`.

- Each answer is drawn in the console under one heading, the line as you typed it, and the prompt returns after the last.
- A command that asks a question (a delete, say) waits for the answer before the line goes on, and "no" counts as a
  failure. Clearing the console ends the line. An error is a failure, and so is a command that couldn't be reached.
- This is the page's doing, as it is a shell's: the engine is sent one command at a time, so these are not Taskwarrior
  syntax. An unquoted operator always separates commands, in a filter too, so write `or` and `and` there, or quote it
  (`'||'`, `\|\|`). `add 'Fish & chips; or pizza'` keeps its `;`.
- `review` has to be on a line of its own. Your task commands may use these too.

## Questions Taskwarrior asks

The same questions as on the desktop, from the same settings, answered on the page.

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
