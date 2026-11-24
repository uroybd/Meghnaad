---
name: meghnaad
description: >
  Read and change the person's Taskwarrior tasks safely through the `meghnaad` command line (around the real `task`
  3.5): compact exports, UUIDs, and asking before bulk or destructive changes. Use when the person asks about their
  tasks, todos, projects, due dates or what to do next, or asks to add, change, complete or annotate a task.
---

# Meghnaad: the person's tasks, from a shell

`meghnaad` is a terminal app around the real Taskwarrior `task` 3.5. From a shell it is two things:

- `meghnaad export [options] [filter]`: `task export`, reshaped (JSON with fewer fields, CSV, or TSV).
- `meghnaad <line>`: a `task` command line, with the person's own task commands written out and `'&&'` `'||'` `';'`
  between statements. A line with neither is exactly `task <line>`.

Both work on the person's real data: whatever `TASKRC` and `TASKDATA` point at, else `~/.taskrc` and `~/.task`.

## 1. Check first

```sh
meghnaad --version && task --version    # task must be 3.5.x
```

If `meghnaad` is missing, use `task` for writes and `task export` for reads; everything below about IDs and asking
still holds.

## 2. Read compactly

Pick the fields you need; TSV is one task to a line, with a header:

```sh
meghnaad export -f tsv -F uuid,id,description,project,tags,due,priority,urgency status:pending
meghnaad export -f json -F uuid,description,annotations,depends project:Home   # when values are nested
meghnaad export -r next -f tsv -F uuid,description,due                         # the next report's filter and sort
task rc.verbose=nothing next             # what the person sees, in local time; IDs only, so not for picking a task
task rc.verbose=nothing count project:Home
task rc.verbose=nothing _projects        # also _tags, _unique project
```

`meghnaad export --help` lists every option. `-F table` gives a report's own columns, which usually leave out `uuid`;
name the fields when you will act on the result.

- Times: CSV and TSV write `YYYY-MM-DD HH:MM:SS` in UTC, JSON `20261124T135545Z` (UTC too); `calc`, reports and
  `info` are in local time. Convert to the person's local time before saying a date, or a due date at midnight reads as
  the day before.
- `export` ignores the context; a report (`-r NAME`, or `task next`) applies it. Say which one you showed.
- Lines such as `TASKRC override: …` go to stderr, around a command's own messages; ignore them.
- Filters are Taskwarrior's: `project:Home`, `+tag`, `-tag`, `due.before:eow`, `status:completed`, `/regex/`,
  `( a or b )` with spaced parentheses.

## 3. Dates

Check what a date means before writing it: `task rc.verbose=nothing calc eonm` prints it in local time. Taskwarrior's
named dates are the safe way to say most of them; arithmetic on them is easy to get wrong (`eom + 1mo` is the 30th of
next month, not its end):

| Say | Means |
| --- | --- |
| `today`, `tomorrow`, `eod` | midnight today, midnight tomorrow, the end of today |
| `monday` … `sunday`, `mon` … `sun` | the next such day after today, at midnight |
| `sow`, `eow` / `soww`, `eoww` | the start and end of this week (Monday to Sunday) / of its working week (to Friday) |
| `som`, `eom`, `sonm`, `eonm` | the start and end of this month, of next month |
| `soy`, `eoy` | the start and end of this year |
| `now + 3d`, `2026-12-24`, `2026-12-24T09:00` | three days from now; a date; a date and local time |
| `later` | no particular date (`9999-12-30`) |

## 4. Name tasks by UUID

Working-set IDs (`1`, `2`, …) change whenever a task is completed or deleted, and between your read and your write. Use
the UUID from an export; its first 8 characters are enough (`06dcf9a8 done`).

## 5. Change tasks

```sh
meghnaad rc.verbose=new-uuid add 'Call the plumber' project:Home due:tomorrow +phone   # prints: Created task <uuid>.
meghnaad 06dcf9a8 modify priority:H due:fri
meghnaad 06dcf9a8 annotate 'Asked for a quote'
meghnaad 06dcf9a8 start      # stop, done likewise
```

- Quote a description that has shell characters. Words like `due:fri` or `+tag` in it are read as attributes; to keep
  them as text, put the description after `--`: `meghnaad add -- 'Read about due:soon'`.
- Check the exit status and stderr: hooks may refuse or change a task. Re-read with an export if it matters.
- Run commands with `</dev/null`: `task` then answers "no" to any question instead of waiting, changes nothing and exits
  1. Treat exit 1 with `Task not modified.` or `Task not deleted.` as "it needed a yes".

## 6. Ask before

Say which filter you used for the person's words ("Home errands" as `project:Home +errand`). If it is a guess and could
mean more than one thing, or a description match such as `/report/` finds more than one task, ask which.

Show the person the tasks it will touch (an export of the same filter) and ask, before any of these:

- a change to **more than one task** (`task` itself only asks above `rc.bulk`, 3 by default);
- `delete`, `purge`, `undo`, `import`, `sync`;
- `config`, `context` (define, delete, or switching it), anything that edits the taskrc.

After a yes, name the tasks by their UUIDs (the filter could match more by now) and tell `task` not to ask again:
`rc.bulk=0` turns off its question for more than `rc.bulk` tasks (0 means never ask), `rc.confirmation=off` the one
for `delete` and `purge`; many deletes need both. Put them first on the line:

```sh
meghnaad rc.bulk=0 ba929807 f417f159 modify +weekend </dev/null
meghnaad rc.confirmation=off ede60b13 delete </dev/null
```

Never: `task edit` or anything else that opens an editor or waits for keys (`meghnaad` with no arguments is a full-screen
app); writing files under the data directory; editing `~/.config/meghnaad/settings.json`.

## 7. Chains and task commands

The person may have task commands (their own named commands, such as `inbox` or `kanban-next`). Before every line with a
task command or an operator, see what it will run; the rules in 6 apply to what it prints, statement by statement:

```sh
meghnaad --dry-run 06dcf9a8 inbox '&&' 06dcf9a8 start
```

It prints each statement, task commands written out, and runs nothing. Quote the operators so the shell passes them on.
`rc.` words belong to the statement they are in: in a chain, write them in each statement that needs them.

## 8. Trying things out

To learn how a command behaves, never experiment on the person's data. Use scratch data, with both variables on every
command, and check where it points before the first write:

```sh
d=$(mktemp -d); printf 'data.location=%s/data\n' "$d" > "$d/rc"
TASKRC="$d/rc" TASKDATA="$d/data" task show data.location    # must print $d/data
TASKRC="$d/rc" TASKDATA="$d/data" task rc.verbose=nothing add test
```
