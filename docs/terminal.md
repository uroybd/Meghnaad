# The terminal app

`meghnaad-tui` is Meghnaad on the desktop, beside its web app and Android app: a full-screen terminal app around the
real `task` 3.5.0, laid out as the web app is. It has no engine, replica or sync of its own. Every report it shows is
`task <filter> export <report>` and every change it makes is a `task` command line, so it works on whatever data and
taskrc your `task` uses (`~/.task` and `~/.taskrc`, or `TASKDATA`, `TASKRC` and `rc.data.location`), and your hooks run
as they always do. What it adds is the web app's pages, a command line with Tab completion, `&&` `||` `;` chains and your
own task commands, an add line that shows the task it will make, tasksh's review, a timer and reminders.

## Install

It needs `task` 3.5.0 on the `PATH` (or `MEGHNAAD_TASK` set to the binary). Get `meghnaad-tui` any of three ways, then
run `meghnaad-tui`:

- **Homebrew**, for macOS and Linux: `brew install uroybd/tap/meghnaad-tui`, which brings `task` with it, and
  `brew upgrade` for a newer one.
- **Prebuilt**, no Rust needed: download `meghnaad-tui-<version>-<platform>.tar.gz` from the
  [latest release](https://github.com/uroybd/Meghnaad/releases/latest) (Linux or macOS, x86_64 or arm64) and put
  `meghnaad-tui` where you keep programs, for example `~/.local/bin`. On macOS, if it is blocked for coming from the
  internet, run `xattr -d com.apple.quarantine ~/.local/bin/meghnaad-tui` (or fetch it with `curl`).
- **From source**, with Rust 1.91 or later: `cargo install --git https://github.com/uroybd/Meghnaad meghnaad-tui --locked`
  puts it in `~/.cargo/bin` (add `--tag v<version>` for a release instead of the latest commit), and
  `cargo uninstall meghnaad-tui` removes it.

`meghnaad-tui --help` says where it keeps its settings. It also starts with another version of `task`, says in the status line which
one it found, and then behaves as that version does.

## The screen

The top bar has the pages, by number (with their names where they fit), then the started task with how long it has run,
the context, review and the reminders. The reports are down the left side (on a terminal 72 columns wide or more) and
the page fills the middle, each in a rounded pane titled in its border; the pane the keys go to has its border in the
accent, and its bottom border says which row of how many is chosen. The bottom two lines are the status line and the
command line, which shows the page's keys until you type into it, with `? all keys` and `q quit` always on the right.

| Page | Shows |
| --- | --- |
| `1` Tasks | A report: the one your `default.command` names, else `next`, with its columns, labels, filter and sort |
| `2` Projects | The project tree from the pending tasks, with counts (sub-projects included) and how many are overdue |
| `3` Tags | Each tag, with the same counts |
| `4` Summary | `task summary`, as it prints it |
| `5` Calendar | `task calendar`, from this month; `h` `l` go a month back or on |
| `6` Burndown | `task burndown.weekly`; `d` `w` `m` show the daily, weekly or monthly one |
| `7` Console | What the command lines run here printed, scrolled with `j` `k`; `c` clears it |
| `8` Settings | When to sync (`y`), your task commands, which you add, change and delete here, and with `t` the taskrc as `task` reads it |

`Enter` on a project or tag opens its tasks: the Tasks page with `project:Home` or `+errand` as the filter. Summary,
Calendar, Burndown and `info` are drawn in `task`'s own colours, at the page's width, with one change: the grey
shading Taskwarrior's default colours give alternate rows, weekends and week numbers is left out, and dark grey text is
drawn dim, so the pages read on a light terminal as on a dark one. Today, due, overdue and the bars keep their colours.
The app's own colours are the terminal's, with the web app's blue as the accent. The report's cells are not coloured
yet.

## The keys

On every page:

| Key | Does |
| --- | --- |
| `1` to `8` | Open a page |
| `Tab` | Move to the reports down the side and back (`Esc` also comes back); `Enter` opens one, `.` shows the hidden ones, whose names start with `_` |
| `:` | The command line |
| `v` | Review the tasks that need it |
| `C` | Choose the context (`task context <name>`, or `none`) |
| `T` | Stop the started task the timer shows |
| `N` | Reminders on or off |
| `S` | [Sync](#sync) now, in the background |
| `u` | `task undo`, on the terminal so `task` can ask first |
| `R` | Read everything again |
| `?` | Every key |
| `q` | Quit |

On the Tasks page:

| Key | Does |
| --- | --- |
| `j` `k`, arrows, `g` `G`, PgUp PgDn | Move through the report |
| `Enter` | `task <uuid> info`, scrolled with `j` `k`; any other key closes it |
| `a` | Add a task, in a box over the page: type what follows `task add`, and the box shows the task it will make; `↓` or `Ctrl+F` opens it into [the full form](#the-form) |
| `m` | Modify the selected task, in [the full form](#the-form) |
| `A` | Annotate the selected task, in a box that shows it as it is |
| `d` | Mark the selected task done |
| `s` `x` | Start, stop the selected task |
| `D` | Delete the selected task, on the terminal so `task` can ask first |
| `c` | Run one of your [task commands](#settings) on the selected task: a box lists them, typing narrows the list, `Enter` runs the one chosen as `<uuid> <name>` would run at the command line |
| `/` | The filter put in front of the report, as in `task <filter> next`; it stays until you change it or quit |
| `r` | Another report, by name |

The quick keys (`a`, `A`, `d`, `s`, `x`, `T`, `C`) and the form run `task` without handing it the terminal and put its first line
(`Created task 3.`) in the status line. They add no overrides. If `task` or a hook asks a question there, it gets no
answer, which is no, and nothing changes; use the command line with `!` when you want to answer. After every command
the pages and what completion offers are read again.

The add box is Meghnaad's reading of the line, by the same rules as `task add`, ported from Taskwarrior. What is made
is what `task add` makes. A line it finds a problem in stays open with the reason. Adding and
changing task commands on the Settings page use the same box.

### The form

The web app's task form, in a box over the page: a line for each part of a task, its name on the left. Description,
project, priority, due, wait, scheduled, until, repeat, tags (space separated), each UDA your taskrc defines, and what it
depends on (task ids); for a new task, whether to start it now and a note, and for one that exists, an annotation to add
under the ones it has. `↑` `↓` go from line to line, `Tab` completes the line's word as the command line would
(`project:Ho` → `project:Home`), `Space` ticks Start, `Enter` saves and `Esc` leaves.

A date is typed as `task` takes it (`fri`, `eom`, `2026-11-20`, `2026-11-20T14:30`) and shown after it as `task` will read
it; a repeat shows its period in words. One task of a series shows its period but leaves it to the recurring task.

Saving sends only what changed: `<uuid> modify` with those attributes (`project:` to remove one, `+tag` and `-tag`,
`depends:-3`), or `add` with what is filled in, then `+LATEST start` and `+LATEST annotate` when asked for. The bottom
border shows the command line it will run. A form `task` refuses stays open with the reason, as does one with no
description, or a repeat without a due date.

In the command line, the boxes and the form, `Tab` completes the word: the shared part first, then each option in turn, with `Shift+Tab` going
back. `Ctrl+A` and `Ctrl+E` go to the start and the end, `Ctrl+W` deletes a word, `Ctrl+U` the line, `Enter` runs it and
`Esc` leaves it. Completion offers commands and reports, attributes, projects, tags, UDA values, task ids and date words,
from what `task` lists (`_show`, `_projects`, `_tags` and the pending tasks).

## The command line

`:` takes a whole command line without the `task` in front, as the web app's console does.

- **Where it runs.** A line runs here, and what `task` prints goes to the Console page (opened for you when it is more
  than a line; a single line goes to the status line). A line whose command asks before it acts or opens an editor
  (`delete`, `edit`, `undo`, `purge`, `import`, `config`, and their abbreviations) hands the terminal to `task`
  instead, so its output and questions are exactly the CLI's, and the app comes back when you press Enter. Put `!` in
  front of any line to run it on the terminal, for example a bulk `modify` that asks for each task. A question asked of
  a line run here is answered no, and the console says so.
- **Chains.** `1 done && 2 start ; next` runs each statement as a shell would: `&&` after a success, `||` after a
  failure, `;` always. Quote an operator to keep it (`add 'a && b'`). A chain with one statement that needs the terminal
  runs there as a whole.
- **Task commands.** A word from `taskCommands` in `settings.json` stands for its command line, with the filter before
  it and the words after it: `3 fail` runs `3 done outcome:fail`. See [Commands](using/settings.md#commands).
- **Review.** `review`, or `review 10` to stop after ten, begins a review, as `v` does.
- **Sync.** `sync` alone syncs in the background, as `S` does.

## Review

Review is tasksh's: it walks the tasks the `_reviewed` report lists (by default, pending or waiting tasks not reviewed
in the last six days) and shows each with `info`. `Enter` marks it reviewed (`modify reviewed:now`), `s` skips it, `e`
opens `task edit` and then marks it reviewed, `m` takes what follows `modify`, `c` completes it, `d` deletes it and `q`
ends the review. Each step runs with `rc.confirmation:no rc.verbose:nothing`, as tasksh runs them.

Review needs a `reviewed` UDA of type date and the `_reviewed` report in your taskrc. When either is missing, it asks
before writing each one with `task config`, in tasksh's words; this is the one time the app changes your taskrc, and only
when you answer `y`.

## The timer and reminders

The top bar shows the started task (the earliest, with `+N` when there are more) and how long it has run, counting each
second; `T` stops it. Reminders are the web app's: a heads-up 15 minutes before a due time, an all-day task at 9 on its
day, a task coming back when its wait ends, and one notice a day for whatever is overdue. They show in the status line
and the Console page, and the count is in the top bar. They work while the app is open, each is said once per run, and
the pending tasks are read again every minute for them. `N` turns them off.

## Sync

Syncing is `task sync`, with Taskwarrior's own sync settings (`sync.*` in the taskrc, `AWS_ENDPOINT_URL` and the like in
the environment). It runs in the background: the screen stays yours, the top bar shows `⟳ syncing`, then how long ago it
worked or `⟳ failed`, and how it went is said in the status line and kept on the Console page, with what `task` wrote
when it failed. The pages are read again when it ends. While it runs, a command you give waits for it, so the two never
use the data at once.

When it runs is the `sync` setting, which `y` on the Settings page goes through and saves:

| `"sync"` | Syncs |
| --- | --- |
| `"manual"` (or none) | Only when asked: `S`, or `sync` at the command line |
| `"startAndClose"` | Also when the app opens and when it closes |
| `"everyChange"` | Also after every command that changes tasks (`add`, `modify`, `done`, `start`, `delete`, `undo`, …, from a key, the form, the command line or the terminal), and once when a review ends |

Closing waits for a sync that is running, then syncs once more when set to, and says on the terminal if that failed.

## Settings

The app's own `settings.json` is kept on this machine only, in the first of:

1. `$MEGHNAAD_CONFIG_DIR`, when it is set;
2. `$XDG_CONFIG_HOME/meghnaad`;
3. `~/.config/meghnaad`.

The file is optional; without it there are no task commands and syncing is by hand. The terminal app uses its
`taskCommands`, words that stand for a command line, which you can also add, change and delete on the Settings page,
and its own `sync` ([Sync](#sync)):

```json
{
  "sync": "startAndClose",
  "taskCommands": [{ "name": "fail", "command": "done outcome:fail" }, { "name": "late", "command": "+OVERDUE list" }]
}
```

It is the same JSON as the web app's (**Settings → Commands**, or `web/settings.json` in the bucket), so copying that
file here gives you the same task commands; the web app and the phone have no use for `sync`. Keys the terminal app has no use for yet (`boards`, `appearance`) are
ignored and kept as they are when it saves. A file that is not a JSON object is reported in the status line and read as
none, and the app will not save over it until it is put right.

The app reads and writes `settings.json` in that folder and touches nothing else there. It writes the file in place, so
a symlink stays a symlink and a dotfiles manager (stow and the like) can own the folder. It never reaches the bucket: syncing is `task sync`, and its settings (`sync.*`
in the taskrc, `AWS_ENDPOINT_URL` and the like in the environment) are Taskwarrior's.

## How it fits (for contributors)

It is a front end like the web app and the phone (layer 2 in `AGENTS.md`), with the real `task` as its backend instead
of `tc-core`'s engine. `tc-core` comes in only for what `task` has no command for, the same helpers the page and the
phone call: completion (`complete`), the add preview (`preview`), the cells of a report (`cells`) and dates in
`dateformat`, all fed with what
`task` said, so a report's columns, dates and indicators read as in the web app. The settings those need come from
`task _show`, which is what `task` itself resolved, parsed by the engine's taskrc reader, which keeps nothing sync or
credential-like, and the Settings page shows those lines without them. The app never reads or edits your taskrc file
itself; review's setup goes through `task config`. Chains and task commands are the web app's logic,
ported to Rust with its tests as the phone has them in Kotlin, and so are the Projects and Tags lists, review (tasksh's,
from the web app's port), the reminders and the timer. Nothing of it is compiled into the browser engine or the
Worker.
