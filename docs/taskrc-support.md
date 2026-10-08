# taskrc support

Which of Taskwarrior's `taskrc` options Meghnaad handles, and which it doesn't.

The list of options comes from the `taskrc(5)` man page of Taskwarrior **3.5.0**; the status of each one comes from
reading this repository's code. The README explains [how the taskrc is imported](../README.md#your-taskrc). In short:
you paste or choose your `~/.taskrc`, only an allowlist of settings is kept, and the result is saved in your bucket
(`web/config.json`), not in your desktop file.

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
| Terminal | 0 | 0 | 1 | 2 | 0 |
| Miscellaneous | 5 | 5 | 8 | 5 | 0 |
| Dates and calendar | 7 | 0 | 1 | 2 | 0 |
| Journal | 3 | 1 | 0 | 0 | 0 |
| Dependencies | 0 | 0 | 0 | 1 | 0 |
| Colour | 0 | 0 | 0 | 1 | 0 |
| Urgency | 17 | 0 | 0 | 0 | 0 |
| Defaults | 2 | 0 | 3 | 0 | 0 |
| Reports | 8 | 0 | 4 | 0 | 0 |
| User defined attributes | 6 | 0 | 0 | 0 | 0 |
| Context | 3 | 0 | 1 | 0 | 0 |
| Sync | 0 | 0 | 0 | 0 | 1 |
| **Total** | **53** | **6** | **20** | **16** | **1** |

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
| `limit` | Not done | The `limit:N`, `limit:page` and `limit:none` filter terms work. The `limit` default from the taskrc is not read |
| `detection`, `defaultwidth`, `defaultheight`, `avoidlastcolumn`, `hyphenate`, `reserved.lines` | N/A | Terminal size and wrapping. The browser lays tables out itself |
| `editor` | N/A | Tasks are edited in the app's editor form |

## Miscellaneous

| Option | Status | Remark |
| --- | --- | --- |
| `search.case.sensitive` | Done | |
| `uda.<name>.indicator` | Done | Shown by the `indicator` column format |
| `recurrence` | Partial | Taskwarrior defaults this to on. Here it is **off** unless the taskrc says `recurrence=on`, because two replicas that both create instances while out of sync make duplicates. See [Recurring tasks](../README.md#recurring-tasks) |
| `recurrence.confirmation` | Done | Unset or `prompt` asks (the question appears in the app), a true-ish value is yes, anything else no |
| `recurrence.indicator` | Done | |
| `recurrence.limit` | Done | |
| `abbreviation.minimum` | Partial | Attribute abbreviations (`desc`, `proj`) work, with the minimum fixed at 2 characters |
| `confirmation` | Partial | Changes to more than one task always ask first. The setting itself is not read |
| `bulk` | Partial | The confirmation threshold is fixed at "more than one task" instead of configurable |
| `allow.empty.filter` | Partial | Writes (`done`, `delete`, `modify`) with no filter are always refused ("no tasks specified"). Taskwarrior's default is to allow them after confirming |
| `regex` | Not done | `/text/` in a filter is a plain text search, not a regular expression |
| `expressions` | Not done | Infix filters only |
| `alias.<name>` | Not done | Aliases are not expanded |
| `list.all.projects`, `summary.all.projects` | Not done | `projects` counts pending tasks only |
| `complete.all.tags`, `list.all.tags` | Not done | `tags` counts pending tasks only |
| `active.indicator`, `tag.indicator`, `dependency.indicator` | Not done | The `indicator` column formats use Taskwarrior's defaults (`+`, `D`) and don't read these |
| `burndown.cumulative` | Not done | There are no burndown reports |
| `date.iso` | Not done | Not read |
| `verbose` | N/A | Accepted and ignored |
| `nag` | N/A | The reminder printed after a command in a terminal |
| `annotation.info`, `indent.annotation`, `indent.report`, `row.padding`, `column.padding`, `print.empty.columns` | N/A | Terminal layout |
| `xterm.title`, `_forcecolor`, `json.array` | N/A | Terminal behaviour. `export` prints a JSON array |
| `debug`, `debug.parser`, `obfuscate` | N/A | Debugging aids for the CLI |

## Dates and calendar

| Option | Status | Remark |
| --- | --- | --- |
| `dateformat` | Done | Taskwarrior's format letters (`Y M D H N S` and the rest) are ported, in tables, info and the detail view |
| `dateformat.report` | Done | |
| `dateformat.info` | Done | |
| `dateformat.annotation` | Done | |
| `report.<name>.dateformat` | Done | |
| `weekstart` | Done | Sunday or Monday (the default). Decides what `sow`/`eow` and the other week-based dates mean |
| `due` | Done | How many days ahead counts as due (`+DUE`). Default 7 |
| `dateformat.edit` | N/A | There is no `task edit` text file; the editor form has date pickers |
| `dateformat.holiday`, `holiday.<name>.*` | N/A | There is no calendar, so holidays are not used |
| `displayweeknumber`, `calendar.*` | Not done | There is no `calendar` command |

## Journal

| Option | Status | Remark |
| --- | --- | --- |
| `journal.time` | Done | `start` and `stop` write the annotations; the detail view turns them into a sessions table and hides the markers |
| `journal.time.start.annotation` | Done | |
| `journal.time.stop.annotation` | Done | |
| `journal.info` | Partial | Stored, but not read. The detail view always shows tracked time when `journal.time` is on, and hides the marker annotations |

## Dependencies

| Option | Status | Remark |
| --- | --- | --- |
| `dependency.reminder`, `dependency.confirmation` | N/A | Prompts for the command line. Dependencies themselves (`depends:`, `+BLOCKED`, `+BLOCKING`) work |

## Colour

| Option | Status | Remark |
| --- | --- | --- |
| `color`, `fontunderline`, `rule.color.merge`, `rule.precedence.color`, `color.*` | N/A | The app has its own light and dark theme, and does not use Taskwarrior colour rules |

## Urgency

All of these can also be edited in the app (**urgency** in the header). See [Urgency](../README.md#urgency) for the
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
| `default.project` | Not done | Not applied by `add` |
| `default.due` | Not done | Not applied by `add` |
| `default.scheduled` | Not done | Not applied by `add` |

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
| Built-in reports | Done | `next`, `list`, `all`, `completed`, `waiting`, `newest`, `oldest`, `overdue`, `active`, `ready`, `recurring`, `unblocked`, `minimal` |
| `report.<name>.annotations` | Not done | Ignored. Annotations are shown in the detail view |
| Built-in `long` | Not done | Define `report.long.*` in your taskrc and it works as a custom report |
| Built-in `ls` | Not done | As above |
| Built-in `blocked` | Not done | As above. `unblocked` exists |

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
| `context.<name>.rc.<key>` | Not done | Per-context overrides (such as a different `default.command`) are ignored |

## Sync

| Option | Status | Remark |
| --- | --- | --- |
| `sync.*`, `taskd.*`, and any name containing `secret`, `password`, `token`, `credential` and similar | Blocked | Never stored, logged or shown; only the *names* of what was dropped are reported. The Worker syncs through its own R2 binding and secret instead |

## Worth doing next

Roughly in order of how much they'd matter to someone coming from the CLI:

1. `default.project`, `default.due` and `default.scheduled` (small, and `add` already applies UDA defaults).
2. `context.<name>.rc.<key>` overrides.
3. The built-in `long`, `ls` and `blocked` reports.
4. `report.<name>.annotations`.
5. A real `regex` mode for `/pattern/` filters.
6. `confirmation`, `bulk` and `allow.empty.filter`, if the stricter defaults get in the way.
