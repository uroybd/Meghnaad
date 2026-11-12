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

`meghnaad-tui --help` says where it keeps its settings, and `meghnaad-tui --version` which release it is. It also starts with another version of `task`, says in the status line which
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
| `7` Boards | The list of your [boards](#boards): `j` `k` choose one and `Enter` opens it; on a board, `j` `k` scroll, `h` `l` (or `[` `]`) go to the board before or after, and `Esc` (or `b`) comes back to the list |
| `8` Console | What the command lines run here printed, scrolled with `j` `k`; `c` clears it |
| `9` Settings | A section each for when to sync, the theme, your task commands and your boards ([Settings](#the-settings-page)) |

`Enter` on a project or tag opens its tasks: the Tasks page with `project:Home` or `+errand` as the filter. Colours
follow `task` ([Colours](#colours)).

## Colours

What Taskwarrior has a colour for is drawn in `task`'s colours, from your taskrc (and any theme it includes):

- **A report's rows** are coloured as `task` colours them: the app asks `task` for the report with its columns swapped
  for `uuid` and colour forced on, and draws each row in the colour that uuid came back in. Every rule counts, as
  `task` applies it: `rule.precedence.color`, `rule.color.merge`, `color.active`, `color.due`, `color.tag.X`,
  `color.project.X`, `color.uda.*`, a `nocolor` tag. The row chosen is drawn reversed, so its colour still shows.
- **The column labels** are in `color.label`, and those the report sorts by in `color.label.sort`.
- **Summary, Calendar, Burndown and `info`** are what `task` printed, in its colours, at the page's width.

Taskwarrior's default colours shade every other row, weekends and week numbers in fixed dark greys, which suit a dark
terminal and turn into black blocks on a light one. By default that shading is left out (and dark grey text is drawn
dim), so the pages read on either; `"shading": "task"` draws it as `task` does, `color.alternate` on every other row of a
report included.

What Taskwarrior has no colour for is the app's own: a light and a dark preset, in the colours the web app and the phone
use, so the three look alike.

| Role | Drawn on | Light | Dark |
| --- | --- | --- | --- |
| `accent` | the brand, the page open, keys in the hints, titles | `#1f6feb` | `#58a6ff` |
| `focus` | the border and title of the pane the keys go to (drawn thick, so it shows without colour too) | `bold #1f6feb` | `bold #58a6ff` |
| `border` | the other panes | `#75756f` | `#9a9a90` |
| `selected` | the row, item or field chosen: `reverse` (its own colours swapped), `marker` (the `▸` and bold only), `accent`, or a colour | `reverse` | `reverse` |
| `dim` | notes, help, what matters less | `#75756f` | `#9a9a90` |
| `ok` | the timer | `#1a7f37` | `#56d364` |
| `bad` | what failed, overdue counts | `#cf222e` | `#ff7b72` |

Which preset is drawn is `tui.theme` in `settings.json`, which the Theme section of the Settings page goes through:

| `"theme"` | Draws |
| --- | --- |
| `"auto"` (or none) | The preset for the terminal's background: the app asks the terminal for it (`OSC 11`), and where it does not answer, the system's appearance (macOS's dark mode, GNOME's colour scheme); dark when neither says |
| `"light"`, `"dark"` | That preset, whatever the terminal is |
| `"mono"` | No colour: bold, reverse and dim only (as the `NO_COLOR` variable does) |
| `{ "base": "auto", "accent": "magenta", … }` | A preset (`base`, `auto` when left out) with roles of your own over it |

On `auto` the app follows the terminal as it changes: it asks again when the terminal gets the focus back and when the
system's appearance changes (looked at every few seconds), since a terminal that follows the system changes with it.
`settings.json` is read again when it changes, so a theme edited by hand shows at once.

A role is written as a taskrc colour is (`bold white on rgb013`, `color33`, `bright red`), or as `#rrggbb`; on a terminal
without 24-bit colour (`COLORTERM` is not `truecolor` or `24bit`) a `#` colour is drawn as the nearest of its 256. An
`accent` of your own is the `focus` too, unless you set that as well. A role that is not a colour is said in the status
line and on the Settings page, and the preset's is used. `color=off` in the taskrc cannot be seen from here: `task`
turns colour off itself whenever it writes to a pipe, as it does for this app.

## The keys

On every page:

| Key | Does |
| --- | --- |
| `1` to `9` | Open a page |
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

When it runs is the `tui.sync` setting, which `y` in the Sync section of the Settings page goes through and saves:

| `"sync"` | Syncs |
| --- | --- |
| `"manual"` (or none) | Only when asked: `S`, or `sync` at the command line |
| `"startAndClose"` | Also when the app opens and when it closes |
| `"everyChange"` | Also after every command that changes tasks (`add`, `modify`, `done`, `start`, `delete`, `undo`, …, from a key, the form, the command line or the terminal), and once when a review ends |

Closing waits for a sync that is running, then syncs once more when set to, and says on the terminal if that failed.

## Boards

A board is a page of your own: numbers, charts and tables over your tasks, on a grid of twelve columns, the same boards
the web app and the phone draw ([Boards](using/boards.md)). The Boards page lists them, each with its description, and
opens the one you choose.

The terminal app shows boards and does not make or change them. They are the `boards` of `settings.json`, so there are
two ways to make one:

- **On the web app or the phone**, where a board is made and laid out on the page (**Boards** → **New board**, then
  **Edit**). Then copy that `settings.json` here: from the web app's Settings, or `web/settings.json` in the bucket.
- **By hand**, in `settings.json` here: a board is an `id`, a `name` and its `sections`, each placed on the grid (`x`,
  `y`, `w`, `h`) with a `type` and what it shows. [Boards](using/boards.md) has every kind of section and a whole
  board as JSON. `R` reads the file again.

What each section shows is worked out as on the web: one `task <filter> export` for each filter the board uses (the
board's, the section's and a series', with the board's date ranges written in), measured by the same code. Numbers are
a figure; bars run down the page for values and across for dates; lines and areas, pies, donuts, polar charts and
radars are drawn in braille dots; a heatmap is a grid of shaded cells; a table is the report as `task` prints it, at
the section's width. A series' colour is the one it names, else blue, red, green, yellow, magenta and cyan in turn. On a
page narrower than 100 columns the board has six columns, as on a phone, and the sections keep their order. A board
is read when its page opens, and again with `R`.

## The Settings page

The page has a section each for Sync, Theme, Task commands and Boards, one under the other. A key acts on the
section that has the focus, whose border is in the accent and names its keys: `j` `k` move through a section's rows
and on into the next one, and `[` `]` jump between sections.

| Section | Keys |
| --- | --- |
| Sync | `y`, `Space` or `Enter` goes through when to sync, and saves it ([Sync](#sync)) |
| Theme | Shows the preset drawn and each role in its colours, and Taskwarrior's own; `Space` or `Enter` goes through `auto`, `light`, `dark` and `mono` (the `base` of a theme with roles of its own), `s` turns `task`'s shading on or off, and each is saved ([Colours](#colours)) |
| Task commands | `a` adds one, `e` or `Enter` changes the one chosen, `D` deletes it |
| Boards | `Enter` opens the board chosen |

## Settings

The app's own `settings.json` is kept on this machine only, in the first of:

1. `$MEGHNAAD_CONFIG_DIR`, when it is set;
2. `$XDG_CONFIG_HOME/meghnaad`;
3. `~/.config/meghnaad`.

The file is optional; without it there are no task commands or boards, syncing is by hand and the theme is the
default. The terminal app uses what it shares with the web app and the phone, `taskCommands` (words that stand for a
command line, which you can also add, change and delete on the Settings page) and `boards` ([Boards](#boards)), and its
own settings, all under `tui`: `sync` ([Sync](#sync)), `theme` and `shading` ([Colours](#colours)).

```json
{
  "taskCommands": [{ "name": "fail", "command": "done outcome:fail" }, { "name": "late", "command": "+OVERDUE list" }],
  "tui": {
    "sync": "startAndClose",
    "theme": { "base": "auto", "accent": "#1f6feb", "selected": "reverse" },
    "shading": "none"
  }
}
```

It is the same JSON as the web app's (**Settings → Commands**, or `web/settings.json` in the bucket), so copying that
file here gives you the same task commands and boards; the web app and the phone keep `tui` as it is and have no use
for it. Keys the terminal app has no use for (`appearance`) are ignored, and every key is kept as it is when it saves.
A `sync` at the top of the file, where 0.6.53 kept it, is still read, and moved into `tui` the next time the app saves. A file that is not a JSON object is reported in the status line and read as
none, and the app will not save over it until it is put right.

The app reads and writes `settings.json` in that folder and touches nothing else there. It writes the file in place, so
a symlink stays a symlink and a dotfiles manager (stow and the like) can own the folder. It never reaches the bucket: syncing is `task sync`, and its settings (`sync.*`
in the taskrc, `AWS_ENDPOINT_URL` and the like in the environment) are Taskwarrior's.

## How it fits (for contributors)

It is a front end like the web app and the phone (layer 2 in `AGENTS.md`), with the real `task` as its backend instead
of `tc-core`'s engine. `tc-core` comes in only for what `task` has no command for, the same helpers the page and the
phone call: completion (`complete`), the add preview (`preview`), the cells of a report (`cells`), dates in
`dateformat`, colours in the taskrc's words, and what a board means and measures (`board`), all fed with what `task`
said, so a report's columns, dates and indicators read as in the web app. The settings those need come from `task
_show`, which is what `task` itself resolved, parsed by the engine's taskrc reader, which keeps nothing sync or
credential-like. The app never reads or edits your taskrc file itself; review's setup goes through `task config`.
Chains and task commands are the web app's logic, ported to Rust with its tests as the phone has them in Kotlin, and so
are the Projects and Tags lists, review (tasksh's, from the web app's port), the reminders, the timer and the glue of
boards (Android's `Boards.kt`). Nothing of it is compiled into the browser engine or the
Worker.
