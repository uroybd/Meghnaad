# taskrc support

Which of Taskwarrior's `taskrc` options Meghnaad handles, and which it doesn't. The list of options comes from the
`taskrc(5)` man page of Taskwarrior **3.5.0**, and the status of each comes from reading this repository's code.

**How the taskrc is used.** You paste or choose your `~/.taskrc` (or a folder of files, with `include` followed). The text
is kept as written in your bucket (`web/rc/…`), only an allowlist of settings is read from it (and what they add up to is
kept in `web/config.json`), and your desktop file is never touched. Lines that can't be used are commented out; sync and
credential lines are removed. See [Your taskrc](using/settings.md#your-taskrc). The app's own settings, such as task
commands, aren't taskrc options: they live in a `settings.json` of their own ([Commands](using/settings.md#commands)).

**Scope.** This tracks Taskwarrior **3.5.0 and newer** only. Settings removed or deprecated before 3.5.0 aren't listed and
aren't supported, even if an older taskrc contains them.

`show` and `config` (Console only) read and edit the same saved settings under the same rules; see
[the console](using/console.md#the-console).

## Status key

| Status | Meaning |
| --- | --- |
| **Done** | Read and behaves like Taskwarrior |
| **Partial** | Read, with a difference. The remark says what |
| **Not done** | Meaningful for a web app but not implemented. Imported as "other settings not used" (harmless) |
| **N/A** | Terminal, desktop or local-file behaviour with no equivalent here |
| **Blocked** | Refused on purpose, for safety |

## At a glance

Each row below counts once, even where a row covers several related options.

| Area | Done | Partial | Not done | N/A | Blocked |
| --- | --- | --- | --- | --- | --- |
| Files, hooks and environment | 3 | 1 | 1 | 5 | 0 |
| Terminal | 1 | 0 | 0 | 2 | 0 |
| Commands, aliases and matching | 4 | 1 | 0 | 0 | 0 |
| Confirmations and safety | 3 | 0 | 0 | 0 | 0 |
| Recurrence | 4 | 0 | 0 | 0 | 0 |
| Lists, indicators and charts | 6 | 0 | 0 | 0 | 0 |
| Output and debugging | 0 | 1 | 0 | 5 | 0 |
| Dates and calendar | 16 | 0 | 0 | 1 | 0 |
| Journal | 4 | 0 | 0 | 0 | 0 |
| Dependencies | 1 | 0 | 0 | 1 | 0 |
| Colour | 7 | 1 | 0 | 1 | 0 |
| Urgency | 17 | 0 | 0 | 0 | 0 |
| Defaults | 5 | 0 | 0 | 0 | 0 |
| Reports | 10 | 0 | 0 | 0 | 0 |
| User defined attributes | 6 | 0 | 0 | 0 | 0 |
| Context | 4 | 0 | 0 | 0 | 0 |
| Sync | 0 | 0 | 0 | 0 | 1 |
| **Total** | **91** | **4** | **1** | **15** | **1** |

## Files, hooks and environment

| Option | Status | Remark |
| --- | --- | --- |
| `rc.<key>:<value>` on a command line | Done | Typed in the console, for one command. Held to the same allowlist as the taskrc, so `rc.sync.*` and credential-like keys are refused. Harmless unknown ones (`rc.verbose`) are accepted and ignored, as `task` does |
| `rc.report.<name>.<attr>:<value>` | Done | Changes one attribute of a report for one command. The table's header sort uses it |
| `include <file>` | Done | Followed in the app's folder of files (Settings → taskrc): the path is relative to the file with the line, later lines win, and files nest to ten levels. A missing file or one outside the folder is reported with its file and line (Taskwarrior stops with an error; the app reports and goes on). Absolute paths, `~` and `$VAR` aren't followed, since there is no file system to read. Taskwarrior's `*.theme` files are expanded when you apply |
| `purge.on-sync` | Not done | The app purges only when you run `purge`, never by itself on a sync. Leave that to your CLI |
| `data.location`, `TASKDATA` | N/A | Data lives in the R2 bucket |
| `TASKRC`, `XDG_CONFIG_HOME` | N/A | There is no local taskrc; its place is a folder of files in the bucket (`web/rc/`), the main one being `taskrc` |
| `hooks` | Partial | The master switch (`hooks=off`, or `rc.hooks:off` for one command). A browser can't run scripts, so hooks are functions in a Lua script kept in the bucket and edited in Settings: `on_launch`, `on_add`, `on_modify`, `on_exit`. The same script runs with the real `task` through `meghnaad-hook`, and `hooks=off` turns it off there too. See [Hooks](using/hooks.md#hooks) |
| `hooks.location`, `debug.hooks` | N/A | There is no hooks directory and no scripts to trace |
| `gc` | N/A | Working-set ids are computed by the app, not stored |
| `exit.on.missing.db` | N/A | There is no local database |

## Terminal

| Option | Status | Remark |
| --- | --- | --- |
| `limit` | Done | The default number of tasks a report shows (`limit=25`; `none` or `0` for all). It applies to reports whose own filter and command line have no `limit:` word, as `rc.limit` does; `rc.limit:5` works for one command. `page` means "as many as fit on screen", which a browser doesn't have, so it shows everything, and the built-in `next` carries `limit:page` so it is never cut. `export`, `count` and `info` are never cut. A cut report says "showing N of M". Unset shows everything, as Taskwarrior does |
| `detection`, `defaultwidth`, `defaultheight`, `avoidlastcolumn`, `hyphenate`, `reserved.lines` | N/A | Terminal size and wrapping. The browser lays tables out itself |
| `editor` | N/A | Tasks are edited in the app's editor form |

## Commands, aliases and matching

How a typed command line is read.

| Option | Status | Remark |
| --- | --- | --- |
| `search.case.sensitive` | Done | |
| `abbreviation.minimum` | Done | The shortest abbreviation understood, default 2, for commands, report names, attribute and UDA names (`proj:`, `desc:`) and the words `calendar` takes. Shorter than that, a word is just a word, as in Taskwarrior (checked against `task` 3.5.0 at 1 to 4). Modifiers (`.has`, `.before`) are never abbreviated. Differences: an abbreviation that fits two commands is reported as ambiguous here, where Taskwarrior treats it as plain text; day and month names in dates stay at three letters, which Taskwarrior doesn't tie to this setting |
| `regex` | Partial | On by default, as in Taskwarrior. Plain words, `/pattern/`, `.has`, `.hasnt`, `.startswith`, `.endswith`, `.word`, `.noword` and the `/from/to/` substitution in `modify` are regular expressions, and for the description the annotations are searched too. The syntax is ECMAScript read byte by byte, as C++ `std::regex` does: `.` is one byte, and `\w`, `\d`, `\s`, `\b` and case folding are ASCII only. **Not supported: lookahead (`(?=…)`, `(?!…)`) and backreferences (`\1`)**, which ECMAScript has, and syntax it lacks (`(?i)`, lookbehind, named groups); a pattern using them is refused with a message. Also different: `.` matches `\r` here and not there, a substitution edits the description only (not annotations), and a replacement that would cut a character in half is skipped. `regex=off` matches plain text, with a leading `^` or trailing `$` as an anchor |
| `expressions` | Done | `expressions=postfix` makes `calc` read `1 2 +` instead of `1 + 2`; anything else is infix. `calc` is a port of Taskwarrior's evaluator (same grammar, operator table and per-type rules, so `7 / 2` is 3, `1y` is 365 days and `2 ^ -1` can't be evaluated) over integers, reals, booleans, strings, dates and durations, with Taskwarrior's messages. DOM references work: `1.due`, `<uuid>.project` (a uuid may be cut to eight characters or more), `1.due.year` and the other date parts, `1.tags.x`, `1.annotations.1.description`, `rc.<setting>` (the default when the taskrc is silent), `tw.version`, `system.version`. Attribute names must be exact (`1.desc` is just a word). `~` and `!~` are refused with Taskwarrior's message, since `calc` has no task to match. Filters are always infix |
| `alias.<name>` | Done | A typed word that is an alias stands for the words it is set to (`alias.rm=delete`), up to ten rounds so aliases can use each other and a loop ends. Taskwarrior's own `rm` (delete), `burndown` (burndown.weekly), `history` and `ghistory` exist without a taskrc, and the taskrc can change or empty them. Only typed lines expand: button arguments and everything after `--` are taken as they are. `history.*` and `ghistory.*` commands don't exist here. For a word standing for a whole command line (with its own filter, icon and button), see [Commands](using/settings.md#commands): they live in `settings.json`, not the taskrc |

## Confirmations and safety

The questions Taskwarrior asks before it changes things. Taskwarrior asks yes / no / all / quit about each task in turn;
here the same questions arrive together as a table with a tick per task.

| Option | Status | Remark |
| --- | --- | --- |
| `confirmation` | Done | On by default. `delete` and `purge` ask ("Delete task 1 'x'?", "Permanently remove task 1 'x'?"), and so does `undo` when there is something to undo. Off: no question for those, though `bulk`, an empty filter and a recurring series still ask. The questions arrive as a table with a tick per task (all ticked is "all", none is "quit"), listing only tasks the command would actually change. The delete buttons take the second click ("sure?") as the answer, or delete at once when the setting is off. `duplicate` asks only at `bulk` tasks. `edit` is not in the app |
| `bulk` | Done | Default 3: a change to that many tasks or more asks first, for any command that changes tasks, whether `confirmation` is on or off. `0` never asks because of the count. Text that isn't a number counts as 0, as in Taskwarrior |
| `allow.empty.filter` | Done | On by default. A command that changes tasks, given no filter, asks first ("This command has no filter, and will modify all (including completed and deleted) tasks"), and after a yes the `bulk` question still follows. Off: refused with Taskwarrior's message. With `confirmation` off and this on it is still refused ("Command prevented from running."), as in Taskwarrior. An active context counts as a filter |

## Recurrence

See [Recurring tasks](using/recurring.md#recurring-tasks) for how it behaves.

| Option | Status | Remark |
| --- | --- | --- |
| `recurrence` | Done | On by default; `recurrence=off` (or `0`, `no`) turns it off. Before the commands Taskwarrior flags Recur (reports, `count`, `export`, …) the app creates the instances that are due and expires tasks past `until`. Taskwarrior advises one primary client with `recurrence=1` and `recurrence=0` on the others, because two that each make an instance before syncing make two. Between this app's own clients that can't happen: an instance's uuid derives from its template and index, so every client makes the same task and replicas merge it. A real `task` on the same data still makes its own copy, so turn it off there |
| `recurrence.confirmation` | Done | Unset or `prompt` asks, a true-ish value is yes, anything else no. It governs `modify`, `append`, `prepend`, `annotate` and `delete` on a task of a series, and `purge` of a template |
| `recurrence.indicator` | Done | |
| `recurrence.limit` | Done | |

## Lists, indicators and charts

| Option | Status | Remark |
| --- | --- | --- |
| `uda.<name>.indicator` | Done | Shown by the `indicator` column format |
| `list.all.projects` | Done | `projects` also counts finished tasks (never deleted ones), and `_projects` lists their projects too (deleted ones' as well, as in Taskwarrior). `projects` follows Taskwarrior's layout: a project's count includes its sub-projects, sub-projects are indented, and a line gives the number of projects and tasks |
| `summary.all.projects` | Done | On, `summary` lists projects whose tasks are all finished too (their bar is full). Off by default |
| `complete.all.tags`, `list.all.tags` | Done | `list.all.tags` makes `tags` count finished and deleted tasks' tags too (the footer counts tasks before the filter, as Taskwarrior does). `complete.all.tags` makes `_tags` list them, along with the built-in tags, and the app offers them for completion |
| `active.indicator`, `tag.indicator`, `dependency.indicator` | Done | What the `start.active`, `tags.indicator` and `depends.indicator` columns show (defaults `*`, `+`, `D`). Headers follow Taskwarrior: `A`, and as many letters of `Tags` or `Depends` as the indicator is long. The dependency columns count only open dependencies, so a finished task no longer shows `D` |
| `burndown.cumulative` | Done | On by default, as in Taskwarrior: a finished task stays counted as done on every later bar. Off counts it only on the bar of the day it was finished. Used by `burndown.daily`, `.weekly`, `.monthly` and `.annual` |

## Output and debugging

Terminal output, and tools for debugging the command line client.

| Option | Status | Remark |
| --- | --- | --- |
| `verbose` | N/A | Accepted and ignored |
| `nag` | N/A | The reminder printed after a command in a terminal |
| `annotation.info` | Partial | In Taskwarrior it decides whether `task info` shows annotations. The detail view here always shows them, so the setting isn't read |
| `indent.annotation`, `indent.report`, `row.padding`, `column.padding` | N/A | Terminal layout |
| `print.empty.columns` | Done | As in Taskwarrior, a report leaves out a column that is empty for every row (default `no`); `yes` keeps them (`rc.print.empty.columns:yes` for one command). The engine says which columns are empty, and the web and Android tables do the leaving out. A report with no rows keeps its columns |
| `xterm.title`, `_forcecolor` | N/A | Terminal behaviour |
| `debug`, `debug.parser`, `obfuscate` | N/A | Debugging aids for the CLI |

## Dates and calendar

The `calendar` command takes Taskwarrior's arguments (`calendar`, `calendar y`, `calendar due`, `calendar 2027`, `calendar
3 2027`, `calendar march 2027`). Its layout, week numbers and which days are coloured how were checked against the real
`task calendar`. The colours are the app's own theme, not `color.calendar.*`.

| Option | Status | Remark |
| --- | --- | --- |
| `dateformat` | Done | Taskwarrior's format letters (`Y M D H N S` and the rest), in tables, info and the detail view |
| `dateformat.report` | Done | |
| `dateformat.info` | Done | |
| `dateformat.annotation` | Done | |
| `report.<name>.dateformat` | Done | |
| `weekstart` | Done | Sunday (the default, as in Taskwarrior) or Monday. It decides how the calendar's weeks start, which week numbers it shows, and how an ISO week such as `2026-W44` is counted (from Monday, ISO weeks; from Sunday, Taskwarrior's older count). `sow`, `eow` and the other week dates count from Monday whatever it says, because Taskwarrior's code does (checked against `task` 3.5.0 on a Sunday with `weekstart=Sunday`); a board's date range is the exception and follows the page's week. `rc.weekstart:monday` works for one command |
| `due` | Done | How many days ahead counts as due (`+DUE`). Default 7 |
| `dateformat.edit` | N/A | There is no `task edit` text file; the editor form has date pickers |
| `dateformat.holiday`, `holiday.<name>.name`, `.date`, `.start`, `.end` | Done | A holiday is one `date`, or a stretch from `start` to `end`. Dates are read in `dateformat.holiday` (default `YMD`; the numeric parts `Y y M m D d` and separators work). `easter`, `goodfriday`, `eastermonday`, `ascension` and `pentecost` work too and, as in Taskwarrior, mean the *next* one from today. Taskwarrior's own ids (`holiday.en-US1.name`, with a dash) are read. `include holidays.en-US.rc` works: add that file to the folder and include it from the main taskrc, or use **Add holidays**, which fetches the current years from Holidata, Nager.Date or a public `.ics` calendar and writes the file. (The `holidays.*.rc` files that ship with Taskwarrior hold only 2021 and 2022) |
| `calendar.details` | Done | `sparse` (default) colours the days that have something due or scheduled; `full` also lists what is due in the months shown, using `calendar.details.report`; `none` turns the colouring off |
| `calendar.details.report` | Done | The report that list uses (default `list`) |
| `calendar.holidays` | Done | `none` (default), `sparse` (colour the holiday days) or `full` (also list them; here a day's tooltip, or a tap, names its holidays too). Whatever the setting, a due or scheduled date set on a holiday is flagged in the form and the console |
| `calendar.legend` | Done | On by default |
| `calendar.monthsperline` | Done | How many months a bare `calendar` shows. A browser has no width in characters, so the default is 3 and the months wrap to fit |
| `calendar.offset`, `calendar.offset.value` | Done | Moves the first month shown by `value` months (default -1) when `offset` is on |
| `displayweeknumber` | Done | Week numbers beside each week, on by default. They follow `weekstart`: ISO weeks for Monday, and weeks counted from the first Sunday for Sunday |
| `date.iso` | Done | On by default. Off, an ISO date typed alone (`2026-12-25`) is understood only if it matches `dateformat` (checked against `task` 3.5.0 for each combination); a date with a time (`2026-12-25T10:00`) or words (`tomorrow`) always is. Typed dates are read in `dateformat` first, like Taskwarrior (`dateformat=m/d/Y` accepts `12/25/2026`). ISO week and day-of-year dates (`2026-W52`, `2026-359`) and year-month (`2026-12`) are read as Taskwarrior reads them and follow `date.iso` too. Difference: `20261225` is accepted here though Taskwarrior refuses it |

## Journal

| Option | Status | Remark |
| --- | --- | --- |
| `journal.time` | Done | `start` and `stop` write the annotations; the detail view turns them into a sessions table and hides the markers |
| `journal.time.start.annotation` | Done | |
| `journal.time.stop.annotation` | Done | |
| `journal.info` | Done | The detail view and `info` (collapsed, read only when opened) list what changed and when, built from the replica's operation log and worded like Taskwarrior's `info` journal: changes within a second share a row, `modified` is never listed, and a stopped `start` reports its duration (measured to `end` when `done end:` is given). Difference: a `start` removed with no matching start in the history (a snapshot dropped it) says `Start deleted.` rather than a time since 1970. History before a snapshot isn't available, as with the CLI |

## Dependencies

| Option | Status | Remark |
| --- | --- | --- |
| `dependency.confirmation` | Done | On by default. Finishing or deleting a task that waits on something and has others waiting on it breaks the chain; the app asks "Would you like the dependency chain fixed?" and, on yes, makes those tasks wait on what it waited on (checked against `task` 3.5.0, task by task in the order they are finished). Off repairs without asking. For several tasks the questions arrive together as a table, after the permission one. A repair that only becomes necessary because of an earlier "no" isn't offered |
| `dependency.reminder` | N/A | The reminder printed to a terminal after `done`, `delete` and `start`. Dependencies themselves (`depends:`, `+BLOCKED`, `+BLOCKING`) work |

## Colour

Taskwarrior's colour rules work, drawn on a web page instead of a terminal. A colour is a palette index underneath, as in
a terminal (`rgb440`, `gray12`, `color214` are the exact xterm colours). The basic sixteen are drawn in a softer palette
that follows the light or dark page, a background in them is tinted into the page, and a foreground that wouldn't show is
moved toward readable. Besides Taskwarrior's names (`red`, `cyan`, …) the basic colours have friendlier ones: `ink`,
`coral`/`rose`, `sage`/`moss`, `amber`/`sand`, `sky`/`ocean`, `orchid`/`plum`, `teal`/`aqua`, `snow`/`pearl`, and `slate`
for bright black. `bold coral on sky` is `bold red on blue`.

| Option | Status | Remark |
| --- | --- | --- |
| `color` | Done | On unless the taskrc says `color=off` (or `rc.color:off` for one command); then nothing is coloured: rows, charts, the history graph |
| `color.active`, `.blocked`, `.blocking`, `.completed`, `.deleted`, `.due`, `.due.today`, `.overdue`, `.recurring`, `.scheduled`, `.tagged`, `.until`, `.project.none`, `.tag.none` | Done | Colour the task's whole row. Ported from Taskwarrior's `rules.cpp`, each checked against the colour the real `task` gives a task |
| `color.tag.<name>`, `color.project.<name>`, `color.keyword.<word>`, `color.uda.<name>`, `color.uda.<name>.<value>` | Done | A tag (virtual ones too), a project or one inside it, a word in the description or a note (case as `search.case.sensitive`), a UDA with a value or this value (`none` for no value) |
| `rule.precedence.color`, `rule.color.merge` | Done | The order the rules apply in, as Taskwarrior reads it (a word completes to every rule it begins: `tag.`, `keyword.`), and whether colours blend or the highest rule wins. The `nocolor` tag exempts a task |
| `color.alternate` | Done | Shades every other row of a report, with the rules laid over it |
| `color.calendar.*`, `color.burndown.*`, `color.summary.*`, `color.history.*` | Done | The calendar's days, the burndown bars, the summary bar and the history graph (counts inside coloured bars, as Taskwarrior draws it in colour) |
| `color.header`, `.label`, `.label.sort`, `.footnote`, `.warning`, `.error`, `.debug`, `color.sync.*`, `color.undo.*` | Partial | Read, checked, and listed by `show` and `colors legend`, but not drawn: the app has no table header, footnote or sync output to colour |
| `fontunderline` | N/A | The table header underline of a terminal |
| Themes | Done | `web/public/themes/` has the 16 standard Taskwarrior themes and the app's own. The taskrc tab's **Colour theme** picker puts a theme's lines into the box, and a pasted `include dark-256.theme` is expanded to them. The files are static assets, not part of the Worker |

Where the app differs from the desktop: with no `color.*` lines the rows follow **Meghnaad's own theme**
(`meghnaad.theme`: soft colours, no zebra), not Taskwarrior's defaults, which are the `default` theme in the picker. And a
`#` starts a comment anywhere in a taskrc line (`bulk=5 # five`), as in Taskwarrior, which several themes rely on.

## Urgency

All of these can also be edited in the app (**Settings → Urgency**). See [Urgency](using/settings.md#urgency) for the
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
| `urgency.user.project.<project>.coefficient` | Done | Applies to the project and its sub-projects, never a lookalike (`Work` doesn't reach `Workshop`). Coefficients add up |
| `urgency.user.tag.<tag>.coefficient` | Done | Matches user tags and virtual tags such as `OVERDUE`. `next` is 15.0 by default |
| `urgency.user.keyword.<keyword>.coefficient` | Done | Case-sensitive match on the description |
| `urgency.uda.<name>.coefficient` | Done | Any value of the UDA |
| `urgency.uda.<name>.<value>.coefficient` | Done | One value. The `priority` defaults (H 6.0, M 3.9, L 1.8) are built in |
| `urgency.inherit` | Done | Off by default, as in Taskwarrior. On, a blocking task takes the highest urgency of what it blocks, through the whole chain, plus 0.01 |

## Defaults

| Option | Status | Remark |
| --- | --- | --- |
| `default.command` | Done | Used when a command line has no command. Falls back to `next` |
| `uda.<name>.default` | Done | Applied by `add` when the UDA isn't given |
| `default.project` | Done | Applied by `add` when the task has no project of its own, after the active context's `write` rule. Not applied by `modify` |
| `default.due` | Done | Applied by `add` when no `due` is given. A duration (`3d`) is counted from now, and a date word (`eow`, `2030-01-01`) is read like the same word typed as `due:`. A recurring task still needs its own `due`, as in Taskwarrior. A value that can't be read is dropped with a warning on import |
| `default.scheduled` | Done | Same as `default.due`, for the scheduled date |

## Reports

| Option | Status | Remark |
| --- | --- | --- |
| `report.<name>.description` | Done | |
| `report.<name>.columns` | Done | With `.format` variants, UDAs included |
| `report.<name>.labels` | Done | |
| `report.<name>.sort` | Done | Ported from `sort.cpp`, quirks included. The table header sort uses the same spec |
| `report.<name>.filter` | Done | |
| `report.<name>.context` | Done | |
| `report.<name>.dateformat` | Done | |
| `json.array` | Done | Whether `export` wraps its tasks in a JSON array (the default: `[`, one task per line, `]`) or prints one task per line. `rc.json.array:off` works for one command |
| `report.timesheet.filter` | Done | What `timesheet` shows with no filter. Taskwarrior's default is the pending tasks started, and the completed tasks ended, in the last four weeks |
| `report.timesheet.context` | Done | Whether the active context applies to `timesheet`. As in Taskwarrior it doesn't, unless this is on |
| Built-in reports | Done | `next`, `list`, `long`, `ls`, `all`, `completed`, `waiting`, `newest`, `oldest`, `overdue`, `active`, `ready`, `recurring`, `blocked`, `unblocked`, `blocking`, `minimal`, with Taskwarrior 3.5.0's columns, labels, filters, sorts and descriptions. A test compares each with `task show report.` on a fresh install, and the interop script checks that each picks and orders the same tasks as the real CLI. Any can be overridden one attribute at a time from the taskrc |

## User defined attributes

| Option | Status | Remark |
| --- | --- | --- |
| `uda.<name>.type` | Done | `string`, `numeric`, `date`, `duration`, `uuid` |
| `uda.<name>.label` | Done | |
| `uda.<name>.values` | Done | Also drives sorting. A trailing blank value is allowed |
| `uda.<name>.default` | Done | See Defaults |
| `uda.<name>.indicator` | Done | |
| `uda.priority.*` | Done | `priority` is a string UDA Taskwarrior defines itself (label `Priority`, values `H,M,L,`), so a taskrc names only what it changes: `uda.priority.values=Critical,Important,` alone is enough. The values are what a task may be given (anything else is refused with "The 'priority' attribute does not allow a value of …"), the sort order and the order for `priority.above:`, `.below:` and `<`, and what the editors, bulk Modify and completion offer. The blank stands where the list puts it (`H,M,,L`: between M and L); a list with no blank puts a task with no priority above every level. Urgency is separate: only `urgency.uda.priority.<value>.coefficient` counts, so a renamed level adds nothing until it has one. `udas` lists it, and `show` prints the default as `H,M,L,` |

A UDA that exists on a task but isn't defined in the taskrc is shown but read-only.

## Context

| Option | Status | Remark |
| --- | --- | --- |
| `context` | Done | The active context. `context <name>` and `context none` change it |
| `context.<name>.read` | Done | `context define` writes it |
| `context.<name>.write` | Done | Applied to new tasks (`add`, `log`). `context define` writes it too, unless the filter can't be one (`or`, `-tag`, a modifier) |
| `context.<name>.rc.<key>` | Done | Settings in force while that context is active: `default.command`, `limit`, a report's filter or sort (`context.work.rc.report.next.filter`), urgency coefficients, and any other setting this app reads. As in Taskwarrior they win over everything, a command-line `rc.` override included, and `rc.context:home` for one command brings in `home`'s settings. They are checked like any taskrc line: credential-like keys are refused by name, a setting the app has no use for is only named, and a value that can't be used is dropped with a warning. A context can't set `context` itself |

## Sync

| Option | Status | Remark |
| --- | --- | --- |
| `sync.*`, `taskd.*`, and any name containing `secret`, `password`, `token`, `credential` and similar | Blocked | Never stored, logged or shown; only the *names* of what was dropped are reported. The Worker syncs through its own R2 binding and secret, and the Android app takes its bucket or sync server (`sync.server.url`, `sync.server.client_id`) on the phone, into its Keystore |

The bucket side of sync isn't configured from a taskrc, but it behaves like the CLI's. The Worker syncs before and after
every command, writes a snapshot on about one push in ten (as `task sync` does), and picks the newest snapshot when it
starts cold. It also tidies the bucket as the CLI does: after about one push in twenty it deletes versions that lost a
race, older snapshots, and versions over 180 days old that a snapshot covers.

## What is left

One "Not done" row remains, and it can't change in a browser: `purge.on-sync`. The app purges only when you run `purge`,
never by itself on a sync; that is left to the CLI. Beyond the taskrc, the commands `edit` and `import-v2` aren't in the
app.
