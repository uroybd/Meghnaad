# taskrc support

Which of Taskwarrior's `taskrc` options Meghnaad handles, and which it doesn't.

The list of options comes from the `taskrc(5)` man page of Taskwarrior **3.5.0**; the status of each one comes from
reading this repository's code. [Using Meghnaad](using.md#your-taskrc) explains how the taskrc is imported. In short: you paste or choose your
`~/.taskrc`, only an allowlist of settings is kept, and the result is saved in your bucket (`web/config.json`), not in
your desktop file.

**Scope.** This tracks Taskwarrior **3.5.0 and newer**, and only that. Settings that were removed or deprecated before
3.5.0 are left out of this list and are not supported, even if an older taskrc still contains them.

`show` and `config` (Console only) read and edit the same saved settings, under the same rules; see
[Using Meghnaad](using.md#the-console).

## Status key

| Status | Meaning |
| --- | --- |
| **Done** | Read and behaves like Taskwarrior |
| **Partial** | Read, but with a difference. The remark says what differs |
| **Not done** | Meaningful for a web app, but not implemented. Imported as "other settings not used" (harmless) |
| **N/A** | Terminal, desktop or local-file behaviour with no equivalent here |
| **Blocked** | Refused on purpose, for safety |

## At a glance

Each row below counts once, even where a row covers several related options.

| Area | Done | Partial | Not done | N/A | Blocked |
| --- | --- | --- | --- | --- | --- |
| Files, hooks and environment | 2 | 0 | 2 | 5 | 0 |
| Terminal | 1 | 0 | 0 | 2 | 0 |
| Commands, aliases and matching | 4 | 1 | 0 | 0 | 0 |
| Confirmations and safety | 3 | 0 | 0 | 0 | 0 |
| Recurrence | 4 | 0 | 0 | 0 | 0 |
| Lists, indicators and charts | 6 | 0 | 0 | 0 | 0 |
| Output and debugging | 0 | 1 | 0 | 5 | 0 |
| Dates and calendar | 16 | 0 | 0 | 1 | 0 |
| Journal | 4 | 0 | 0 | 0 | 0 |
| Dependencies | 1 | 0 | 0 | 1 | 0 |
| Colour | 0 | 0 | 0 | 1 | 0 |
| Urgency | 17 | 0 | 0 | 0 | 0 |
| Defaults | 5 | 0 | 0 | 0 | 0 |
| Reports | 8 | 0 | 0 | 0 | 0 |
| User defined attributes | 6 | 0 | 0 | 0 | 0 |
| Context | 4 | 0 | 0 | 0 | 0 |
| Sync | 0 | 0 | 0 | 0 | 1 |
| **Total** | **81** | **2** | **2** | **15** | **1** |

## Files, hooks and environment

| Option | Status | Remark |
| --- | --- | --- |
| `rc.<key>:<value>` on a command line | Done | Typed in the console, for one command. Held to the same allowlist as the taskrc, so `rc.sync.*` and credential-like keys are refused. Harmless unknown ones (`rc.verbose`) are accepted and ignored, as the real `task` does |
| `rc.report.<name>.<attr>:<value>` | Done | Changes one attribute of a report for one command. The header sort in the table uses this |
| `include <file>` | Not done | A file can't be read from a browser. The line is skipped with a warning; paste the included file's contents too |
| `purge.on-sync` | Not done | The app never purges deleted tasks. Leave that to your CLI |
| `data.location`, `TASKDATA` | N/A | Data lives in the R2 bucket |
| `TASKRC`, `XDG_CONFIG_HOME` | N/A | There is no local taskrc; settings are saved in the bucket |
| `hooks`, `hooks.location`, `debug.hooks` | N/A | A Worker can't run local programs, so `on-add`/`on-modify` hooks can't work |
| `gc` | N/A | Working-set ids are computed by the app, not stored |
| `exit.on.missing.db` | N/A | There is no local database |

## Terminal

| Option | Status | Remark |
| --- | --- | --- |
| `limit` | Done | The default number of tasks a report shows (`limit=25`, `none` or `0` for all). It applies to reports whose own filter and command line have no `limit:` word, as `rc.limit` does in Taskwarrior; `rc.limit:5` works for one command. `page` means "as many as fit on the screen", which a browser doesn't have, so it shows everything. The built-in `next` carries `limit:page`, so it is never cut. `export`, `count` and `info` are never cut by the setting. A report shows "showing N of M" when it is cut. Unset shows everything, as Taskwarrior does |
| `detection`, `defaultwidth`, `defaultheight`, `avoidlastcolumn`, `hyphenate`, `reserved.lines` | N/A | Terminal size and wrapping. The browser lays tables out itself |
| `editor` | N/A | Tasks are edited in the app's editor form |

## Commands, aliases and matching

How a typed command line is read.

| Option | Status | Remark |
| --- | --- | --- |
| `search.case.sensitive` | Done | |
| `abbreviation.minimum` | Done | The shortest abbreviation understood, default 2: for commands and report names (`ver` for `version` at 3), attribute and UDA names (`proj:`, `desc:`) and the words `calendar` takes (`due`, month names). Shorter than that, a word is not an abbreviation but just a word, as in Taskwarrior (checked against `task` 3.5.0 at 1 to 4). Modifiers (`.has`, `.before`) are never abbreviated. Differences: an abbreviation that fits two commands is reported as ambiguous here, where Taskwarrior treats it as plain text; the names of days and months in dates stay at three letters, which Taskwarrior does not tie to this setting |
| `regex` | Partial | On by default, as in Taskwarrior. With it on, plain words, `/pattern/`, `.has`, `.hasnt`, `.startswith`, `.endswith`, `.word`, `.noword` and the `/from/to/` substitution in `modify` are regular expressions, and for the description the annotations are searched too. The syntax is ECMAScript, read byte by byte, as Taskwarrior's C++ `std::regex` does: `.` is one byte, and `\w`, `\d`, `\s`, `\b` and case folding are ASCII only. **Not supported: lookahead (`(?=…)`, `(?!…)`) and backreferences (`\1`)**, which ECMAScript has; a pattern that uses them is refused with a message that says so. So is syntax ECMAScript doesn't have (`(?i)`, lookbehind, named groups). Also different: `.` matches `\r` here and not there; a substitution edits the description only, not annotations; and a replacement that would cut a character in half is skipped. `regex=off` matches plain text, with a leading `^` or trailing `$` as an anchor |
| `expressions` | Done | `expressions=postfix` makes `calc` read `1 2 +` instead of `1 + 2`; anything else is infix. `calc` is a port of Taskwarrior's evaluator (same grammar, shunting-yard, operator table and per-type rules, so `7 / 2` is 3, `1y` is 365 days and `2 ^ -1` can't be evaluated) over integers, reals, booleans, strings, dates and durations, with Taskwarrior's messages. References into Taskwarrior's DOM work too: `1.due`, `<uuid>.project` (a uuid may be cut short to eight characters or more), `1.due.year` and the other parts of a date, `1.tags.x`, `1.annotations.1.description`, `rc.<setting>` (the default when the taskrc is silent), `tw.version`, `system.version` and the rest; attribute names must be exact (`1.desc` is just a word, as in Taskwarrior). `~` and `!~` are refused with Taskwarrior's own message, since `calc` has no task to match against. Filters are always infix, as in Taskwarrior, where the setting only concerns `calc` |
| `alias.<name>` | Done | A typed word that is an alias stands for the words it is set to (`alias.rm=delete`), up to ten rounds so one alias can use another and a loop ends. Taskwarrior's own `rm` (delete), `burndown` (burndown.weekly), `history` and `ghistory` are there without a taskrc, and the taskrc can change or empty them. Only typed lines expand: arguments from the buttons are taken as they are, and so is everything after `--`. `history.*` and `ghistory.*` commands don't exist here |

## Confirmations and safety

The questions Taskwarrior asks before it changes things. Taskwarrior asks yes / no / all / quit about each task in turn; here the same questions arrive together as a table with a tick per task.

| Option | Status | Remark |
| --- | --- | --- |
| `confirmation` | Done | On by default. `delete` asks ("Delete task 1 'x'?"), and so does `undo` when there is something to undo. Off: no question for those, though `bulk`, an empty filter and a recurring series still ask. Taskwarrior asks about each task in turn (yes / no / all / quit); here the same questions arrive together as a table with a tick per task (ticking all is "all", ticking none is "quit"), and only tasks the command would actually change are listed. The delete buttons in the app take the second click ("sure?") as the answer, or delete at once when the setting is off. Commands the app doesn't have (`purge`, `duplicate`, `edit`) have no question to ask |
| `bulk` | Done | Default 3: a change to that many tasks or more asks first, for any command that changes tasks, with `confirmation` on or off. `0` never asks because of the count. Text that isn't a number counts as 0, as in Taskwarrior |
| `allow.empty.filter` | Done | On by default. A command that changes tasks, given no filter, asks first ("This command has no filter, and will modify all (including completed and deleted) tasks"), and after a yes the `bulk` question still follows, as in Taskwarrior. Off: refused with Taskwarrior's message. With `confirmation` off and this on it is still refused ("Command prevented from running."), as in Taskwarrior. An active context counts as a filter |

## Recurrence

See [Recurring tasks](using.md#recurring-tasks) for how it behaves.

| Option | Status | Remark |
| --- | --- | --- |
| `recurrence` | Done | On by default, as in Taskwarrior; `recurrence=off` (or `0`, `no`) turns it off. Before each command the app creates the instances that are due, retires finished series and expires tasks past `until`, the way `task` does. Taskwarrior itself advises one primary client with `recurrence=1` and `recurrence=0` on all the others when syncing several, because of a duplication bug. Instances are numbered by their index in the template's mask, which is what keeps a second replica from finding anything missing, but if you see duplicates, turn it off on one side. See [Recurring tasks](using.md#recurring-tasks) |
| `recurrence.confirmation` | Done | Unset or `prompt` asks (the question appears in the app), a true-ish value is yes, anything else no |
| `recurrence.indicator` | Done | |
| `recurrence.limit` | Done | |

## Lists, indicators and charts

| Option | Status | Remark |
| --- | --- | --- |
| `uda.<name>.indicator` | Done | Shown by the `indicator` column format |
| `list.all.projects` | Done | `projects` also counts finished tasks (never deleted ones), and `_projects` lists their projects too (deleted tasks' as well, as in Taskwarrior). `projects` follows Taskwarrior's layout: a project's count includes its sub-projects, sub-projects are indented, and a line gives the number of projects and tasks |
| `summary.all.projects` | Done | With it on, `summary` lists projects whose tasks are all finished too (their bar is full). Off by default |
| `complete.all.tags`, `list.all.tags` | Done | `list.all.tags` makes `tags` count finished and deleted tasks' tags too (the footer counts the tasks before the filter, as Taskwarrior does). `complete.all.tags` makes `_tags` list them, along with the built-in tags; with it set the app offers those tags for completion too |
| `active.indicator`, `tag.indicator`, `dependency.indicator` | Done | What the `start.active`, `tags.indicator` and `depends.indicator` columns show (defaults `*`, `+`, `D`). Their headers follow Taskwarrior: `A`, and as many letters of `Tags` or `Depends` as the indicator is long. The dependency columns count only dependencies still open, so a finished task no longer shows `D` |
| `burndown.cumulative` | Done | On by default, as in Taskwarrior: a finished task stays counted as done on every later bar. Off counts it only on the bar of the day it was finished. Used by `burndown.daily`, `.weekly`, `.monthly` and `.annual` |

## Output and debugging

Terminal output, and tools for debugging the command line client.

| Option | Status | Remark |
| --- | --- | --- |
| `verbose` | N/A | Accepted and ignored |
| `nag` | N/A | The reminder printed after a command in a terminal |
| `annotation.info` | Partial | In Taskwarrior it decides whether `task info` shows annotations. The detail view here always shows them, like opening a file, so the setting is not read |
| `indent.annotation`, `indent.report`, `row.padding`, `column.padding`, `print.empty.columns` | N/A | Terminal layout |
| `xterm.title`, `_forcecolor`, `json.array` | N/A | Terminal behaviour. `export` prints a JSON array |
| `debug`, `debug.parser`, `obfuscate` | N/A | Debugging aids for the CLI |

## Dates and calendar

The `calendar` command is supported, with Taskwarrior's arguments (`calendar`, `calendar y`, `calendar due`, `calendar 2027`, `calendar 3 2027`, `calendar march 2027`). The layout, week numbers, and which days are coloured how were checked against the real `task calendar`. The colours are the app's own theme, not `color.calendar.*`.

| Option | Status | Remark |
| --- | --- | --- |
| `dateformat` | Done | Taskwarrior's format letters (`Y M D H N S` and the rest) are ported, in tables, info and the detail view |
| `dateformat.report` | Done | |
| `dateformat.info` | Done | |
| `dateformat.annotation` | Done | |
| `report.<name>.dateformat` | Done | |
| `weekstart` | Done | Sunday (the default, as in Taskwarrior) or Monday. Decides what `sow`/`eow` and the other week-based dates mean, how the calendar's weeks start, and which week numbers it shows. `rc.weekstart:monday` works for one command |
| `due` | Done | How many days ahead counts as due (`+DUE`). Default 7 |
| `dateformat.edit` | N/A | There is no `task edit` text file; the editor form has date pickers |
| `dateformat.holiday`, `holiday.<name>.name`, `.date`, `.start`, `.end` | Done | A holiday is one `date`, or a stretch from `start` to `end`. Dates are read in `dateformat.holiday` (default `YMD`; the numeric parts `Y y M m D d` and separators are understood). `easter`, `goodfriday`, `eastermonday`, `ascension` and `pentecost` work too, and, as in Taskwarrior, mean the *next* one from today. `include holidays.en-US.rc` is not followed (see Files): paste the holidays into the taskrc dialog |
| `calendar.details` | Done | `sparse` (the default) colours the days that have something due or scheduled; `full` also lists what is due in the months shown, using `calendar.details.report`; `none` turns the colouring off |
| `calendar.details.report` | Done | The report that list uses (default `list`) |
| `calendar.holidays` | Done | `none` (the default), `sparse` (colour the holiday days) or `full` (also list them) |
| `calendar.legend` | Done | On by default |
| `calendar.monthsperline` | Done | How many months a bare `calendar` shows. A browser has no width in characters, so the default is 3 and the months wrap to fit |
| `calendar.offset`, `calendar.offset.value` | Done | Moves the first month shown, by `value` months (default -1) when `offset` is on |
| `displayweeknumber` | Done | Week numbers beside each week, on by default. They follow `weekstart`: ISO weeks for Monday, and weeks counted from the first Sunday for Sunday |
| `date.iso` | Done | On by default. Off, an ISO date typed by itself (`2026-12-25`) is only understood if it matches `dateformat` (checked against `task` 3.5.0 for each combination); a date with a time (`2026-12-25T10:00`) and words (`tomorrow`) always are. Typed dates are read in `dateformat` first, like Taskwarrior (`dateformat=m/d/Y` accepts `12/25/2026`). Differences: ISO week and ordinal dates (`2026-W52`, `2026-359`) are not understood either way, and `20261225` is accepted here though Taskwarrior refuses it |

## Journal

| Option | Status | Remark |
| --- | --- | --- |
| `journal.time` | Done | `start` and `stop` write the annotations; the detail view turns them into a sessions table and hides the markers |
| `journal.time.start.annotation` | Done | |
| `journal.time.stop.annotation` | Done | |
| `journal.info` | Done | The detail view (collapsed until opened) and `info` list what changed and when, built from the replica's operation log and worded like Taskwarrior's `info` journal: changes within a second share a row, `modified` is never listed, a stopped `start` reports its duration (measured to `end` when `done end:` is given). Differences: a `start` removed with no matching start in the history (a snapshot dropped it) says `Start deleted.` rather than a time since 1970. History before a snapshot is not available, as with the CLI |

## Dependencies

| Option | Status | Remark |
| --- | --- | --- |
| `dependency.confirmation` | Done | On by default. Finishing or deleting a task that waits on something and has others waiting on it breaks the chain; the app asks "Would you like the dependency chain fixed?" and, on yes, makes those tasks wait on what it waited on (checked against `task` 3.5.0, task by task in the order they are finished). Off repairs without asking. The questions for several tasks arrive together as a table, after the permission one. A repair that only becomes necessary because of an earlier "no" is not offered, and is left alone |
| `dependency.reminder` | N/A | The reminder text printed to a terminal after `done`, `delete` and `start`. Dependencies themselves (`depends:`, `+BLOCKED`, `+BLOCKING`) work |

## Colour

| Option | Status | Remark |
| --- | --- | --- |
| `color`, `fontunderline`, `rule.color.merge`, `rule.precedence.color`, `color.*` | N/A | The app has its own light and dark theme, and does not use Taskwarrior colour rules |

## Urgency

All of these can also be edited in the app (**urgency** in the header). See [Urgency](using.md#urgency) for the
inheritance rules, which follow Taskwarrior's source.

| Option | Status | Remark |
| --- | --- | --- |
| `urgency.project.coefficient` | Done | Default 1.0 |
| `urgency.active.coefficient` | Done | Default 4.0 |
| `urgency.scheduled.coefficient` | Done | Default 5.0 |
| `urgency.waiting.coefficient` | Done | Default -3.0 |
| `urgency.blocked.coefficient` | Done | Default -5.0 |
| `urgency.blocking.coefficient` | Done | Default 8.0 |
| `urgency.annotations.coefficient` | Done | Default 1.0 |
| `urgency.tags.coefficient` | Done | Default 1.0 |
| `urgency.due.coefficient` | Done | Default 12.0 |
| `urgency.age.coefficient` | Done | Default 2.0 |
| `urgency.age.max` | Done | Default 365 |
| `urgency.user.project.<project>.coefficient` | Done | Applies to the project and its sub-projects, never to a lookalike (`Work` does not reach `Workshop`). Coefficients add up |
| `urgency.user.tag.<tag>.coefficient` | Done | Matches user tags and virtual tags such as `OVERDUE`. `next` is 15.0 by default |
| `urgency.user.keyword.<keyword>.coefficient` | Done | Case-sensitive match on the description |
| `urgency.uda.<name>.coefficient` | Done | Any value of the UDA |
| `urgency.uda.<name>.<value>.coefficient` | Done | One value. The `priority` defaults (H 6.0, M 3.9, L 1.8) are built in |
| `urgency.inherit` | Done | Off by default, as in Taskwarrior. When on, a blocking task takes the highest urgency of what it blocks, through the whole chain, plus 0.01 |

## Defaults

| Option | Status | Remark |
| --- | --- | --- |
| `default.command` | Done | Used when a command line has no command. Falls back to `next` |
| `uda.<name>.default` | Done | Applied by `add` when the UDA isn't given |
| `default.project` | Done | Applied by `add` when the task has no project of its own, after the active context's `write` rule. Not applied by `modify` |
| `default.due` | Done | Applied by `add` when no `due` is given. A duration (`3d`) is counted from now and a date word (`eow`, `2030-01-01`) is read like the same word typed as `due:`. A recurring task still needs its own `due`: the default does not stand in for it, as in Taskwarrior. A value that can't be read is dropped with a warning when the taskrc is imported |
| `default.scheduled` | Done | Same as `default.due`, for the scheduled date |

## Reports

| Option | Status | Remark |
| --- | --- | --- |
| `report.<name>.description` | Done | |
| `report.<name>.columns` | Done | With `.format` variants, UDAs included |
| `report.<name>.labels` | Done | |
| `report.<name>.sort` | Done | Ported from `sort.cpp`, including its quirks. The table header sort uses the same spec |
| `report.<name>.filter` | Done | |
| `report.<name>.context` | Done | |
| `report.<name>.dateformat` | Done | |
| Built-in reports | Done | `next`, `list`, `long`, `ls`, `all`, `completed`, `waiting`, `newest`, `oldest`, `overdue`, `active`, `ready`, `recurring`, `blocked`, `unblocked`, `blocking`, `minimal`, with Taskwarrior's own columns, labels, filters and sorts. Any of them can be overridden one attribute at a time from the taskrc |

## User defined attributes

| Option | Status | Remark |
| --- | --- | --- |
| `uda.<name>.type` | Done | `string`, `numeric`, `date`, `duration`, `uuid` |
| `uda.<name>.label` | Done | |
| `uda.<name>.values` | Done | Also drives sorting. A trailing blank value is allowed |
| `uda.<name>.default` | Done | See Defaults |
| `uda.<name>.indicator` | Done | |
| `uda.priority.*` | Done | `priority` is handled as a string UDA, like Taskwarrior does, so a taskrc that defines it (type and values) changes the priority sort order |

A UDA that exists on a task but isn't defined in the taskrc is shown but read-only.

## Context

| Option | Status | Remark |
| --- | --- | --- |
| `context` | Done | The active context |
| `context.<name>.read` | Done | |
| `context.<name>.write` | Done | Applied to new tasks |
| `context.<name>.rc.<key>` | Done | Settings that are in force while that context is the active one: `default.command`, `limit`, a report's filter or sort (`context.work.rc.report.next.filter`), urgency coefficients, and any other setting this app reads. As in Taskwarrior they win over everything else, a `rc.` override typed on the command line included, and `rc.context:home` for one command brings in `home`'s settings. They are checked like any taskrc line: credential-like keys are refused by name, a setting this app has no use for is only named, and a value that can't be used is dropped with a warning. A context cannot set `context` itself |

## Sync

| Option | Status | Remark |
| --- | --- | --- |
| `sync.*`, `taskd.*`, and any name containing `secret`, `password`, `token`, `credential` and similar | Blocked | Never stored, logged or shown; only the *names* of what was dropped are reported. The Worker syncs through its own R2 binding and secret instead |

The bucket side of sync is not configured from a taskrc, but it behaves like the CLI's. The Worker syncs before and
after every command, writes a snapshot on about one push in ten (as `task sync` does, which never avoids snapshots),
and picks the newest snapshot when it starts cold. Deleting old versions is left to the CLI.

## What is left

Two "Not done" rows remain, and neither can change in a browser:

1. `include <file>`: a browser can't read a file, so this stays a paste-in step.
2. `purge.on-sync`: the app never purges deleted tasks; that is left to the CLI.

Beyond the taskrc, the commands `history.*`, `ghistory.*`, `edit` and `purge` are not in the app.
