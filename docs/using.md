# Using Meghnaad

The app has a normal interface and a console, and both talk to the same command engine, so anything you can click
you can type, and the other way round. Behaviour follows Taskwarrior **3.5.0**; where it differs from the desktop,
this guide says so. How the engine works is in [Architecture](architecture.md); every supported `taskrc` setting is in
[taskrc support](taskrc-support.md).

**Contents:** [The pages](#the-pages) · [The console](#the-console) · [Questions](#questions-taskwarrior-asks) ·
[Tasks](#tasks) · [Your taskrc](#your-taskrc) · [Urgency](#urgency) · [Recurring tasks](#recurring-tasks) ·
[Hooks](#hooks) · [Time tracking and reminders](#time-tracking-and-reminders) · [On a phone](#on-a-phone) · [Different from the
desktop](#different-from-the-desktop)

## The pages

| Page | What it shows |
| --- | --- |
| **Tasks** | Any report as a table, with filters. Click a row for the task's detail |
| **Projects** | Projects and sub-projects, with their tasks underneath |
| **Summary** | How far along each project is |
| **Calendar** | Months, with what is due or scheduled on which day |
| **Burndown** | Pending, started and done over time |
| **Console** | Type commands; reports and answers print here |

Summary, Calendar and Burndown have the same filter box as Tasks (Tab completes, filters become chips, the filter
helpers are below it) and it is focused when you open the page (not on a phone, where that would raise the keyboard).
The header also has the running **timer**, the **bell** (reminders), **urgency** (coefficients) and **taskrc**
(settings). Every page except Console has the prompt in a bar at the bottom; the Console has it inside.

The **last command** is shown above the prompt, including what a click ran: pick a report or sort a column and you can
see the exact command, then press **↑** to edit and re-run it.

## The console

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
| **Write** | `add` `modify` `done` `delete` `start` `stop` `annotate` `denotate` `append` `prepend` `undo` |
| **Read** | `info` `count` `projects` `tags` `summary` `calendar` `burndown.daily` `.weekly` `.monthly` `.annual` `udas` `columns` `reports` `contexts` `show` `config` `export` `ids` `uuids` `calc` `_projects` `_tags` |
| **Filters** | `attr:value` with modifiers (`.is .not .has .startswith .before .after .by .none .any …`); `+tag` / `-tag`; virtual tags (`+OVERDUE +DUETODAY +READY +ACTIVE +BLOCKED …`); plain words and `/pattern/`; ids (`3`, `1-4,7`) and uuid prefixes; `and` `or` `not` and parentheses |
| **Dates** | `today tomorrow eow som eoy monday 3d 2w`, `2026-12-25`, `2026-12-25T08:30`, `now+2h`, and anything your `dateformat` describes (`12/25/2026` with `m/d/Y`) |

**Changes beside a command.** What follows `done`, `delete`, `start`, `stop`, `annotate`, `append` and `prepend` can carry
changes, as in Taskwarrior: `3 done end:-2h` (finished two hours ago), `3 start due:eow +urgent`,
`3 annotate called her due:friday`. Attributes, tags and substitutions are applied to the task; the plain words left over
are the annotation (or, for `annotate`, `append` and `prepend`, the text). The buttons send their text literally, so a
note that starts with `due:` stays a note.

**Where a report prints.** Typed in the Console, a report (`list`, `next`, …) prints there. Typed in the bar under any
other page, it opens in the Tasks view, where it can be sorted and filtered.

**Keys:** `/` focuses the prompt · `Tab` completes (and cycles a menu; `Shift+Tab` goes back) · `↑`/`↓` the last 20
commands · `Esc` closes a menu · `Ctrl+L` clears the console.

**Abbreviations and aliases.** A command, report or attribute name may be shortened to `abbreviation.minimum`
characters (default 2): `proj:Home`, `ann 3 text`. `alias.<name>=<words>` in the taskrc makes a typed word stand for
others; Taskwarrior's own `rm` (delete) and `burndown` (burndown.weekly) work without one. Only what you type is
expanded, never what the buttons send.

**Text is matched as regular expressions**, as in Taskwarrior (`regex=1`): plain words, `/pattern/`, `.has`,
`.startswith`, `.endswith`, `.word`, and the `/from/to/` substitution in `modify`. A pattern is searched for anywhere in
the text, and for the description the annotations are searched too. Quote one that has spaces (`'/buy.*milk/'`).
Taskwarrior reads these with C++ `std::regex` (ECMAScript, byte by byte) and so does this app: `\w`, `\d` and
case-insensitive matching are ASCII only. Lookahead and backreferences are refused with a message, as is syntax
ECMAScript doesn't have (`(?i)`). `regex=off` goes back to plain text.

**`calc`** is Taskwarrior's calculator: numbers, durations, dates, strings, `and`/`or`, comparisons, and references
such as `1.due - now`, `1.tags.x`, `rc.bulk` or `system.version`. `expressions=postfix` makes it read `2 days 3 hours +`.

**`projects` and `tags`** list what is pending, with a project's count including its sub-projects.
`list.all.projects` and `list.all.tags` add finished tasks; `complete.all.tags` does the same for tag completion.

**`show` and `config`** read and change the settings the app keeps, and work **only in the Console** (typed in the
bar under another page, they switch you to it). `show` lists every setting, defaults included, with the ones you
changed highlighted and their default beneath; `show weekstart` narrows it to names containing that text. `config
weekstart monday` sets one (several words become one value, `config name ""` blanks it), and `config weekstart` with no
value removes it, putting the default back. With `confirmation` on it asks first, as Taskwarrior does. The change is saved
in your bucket just like an imported taskrc, and is the same list the **taskrc** page shows. Both follow the
[same rules as an import](#your-taskrc): sync and credential-like names are refused, never echoed or shown, and a name the
app doesn't read, or a value it can't accept (`weekstart someday`), is refused with the reason instead of being kept.

## Questions Taskwarrior asks

The same questions as on the desktop, from the same settings, but answered on the page.

- **`delete` and `undo` ask** (`confirmation`, on by default).
- **A change to `bulk` tasks or more asks** (default 3), whatever the command.
- **A command with no filter asks** before it changes every task, finished and deleted ones included
  (`allow.empty.filter`).
- **Finishing or deleting a task in the middle of a dependency chain** offers to repair the chain
  (`dependency.confirmation`): what waited on it then waits on what it waited on.
- **Editing one task of a recurring series** asks whether to change the rest of the series too
  (`recurrence.confirmation`).

Taskwarrior asks yes / no / all / quit about each task in turn. Here the questions arrive together as a **table with a
tick for each task the command would change**: tick all, none, or the ones you want, then go ahead. Only tasks the
command would actually change are listed. A single task is a plain yes/no. The delete buttons ask by turning into
"sure?" on the first click.

## Tasks

- **Reports.** Pick any report, including your own, and add filters by typing (Tab completes) or from the helpers.
  Filters show as removable chips. A report's own filter and default sort are shown above the table.
- **Sorting.** Click a column header (again to reverse, a third time to reset, Shift-click to add a tie-breaker). It is
  applied as `rc.report.<name>.sort:…`, so it matches what you would type.
- **Reading the table.** IDs are bold, and hovering one shows the task's uuid with a Copy button. Project names are
  split at the dots, each part in its own colour. Tags are pills. Urgency is coloured by level (under 5, to 10, to 15,
  from 15). The `start.active`, `tags.indicator` and `depends.indicator` columns follow `active.indicator`,
  `tag.indicator` and `dependency.indicator`.
- **The detail view** (click a row): all fields, annotations, dependencies you can follow, a coloured status pill, the
  work sessions table (with `journal.time`) and a collapsed **History** of what changed and when (`journal.info`, on by
  default, worded like `task info`). History comes from the task's operation log, so changes made before the last
  snapshot are not in it. Buttons: done, start/stop, edit, delete.
- **Add tasks** from the sidebar. *More fields…* opens the full form: project, priority, due/wait/scheduled/until, tags,
  dependencies, UDAs, "start now" and a first note. Every date field takes a date and an optional time.
- **Annotations and string UDAs can span several lines.** In an annotation box Enter saves and Shift+Enter starts a
  new line; in a UDA field Enter starts a new line and Ctrl/Cmd+Enter saves. How a table shows annotations is up to
  the report's `description` column, as in Taskwarrior 3 (`description`, `description.oneline`, `description.count`,
  `description.desc`).
- **Dates are shown the way your taskrc says**: `dateformat`, `dateformat.report` (tables), `dateformat.info` (detail
  view), `dateformat.annotation` (notes) and a report's own `dateformat`, with Taskwarrior's tokens (`Y y M m D d H h N
  n S s A a B b V v J j w`). Without one, dates show as `2026-12-25`, plus the time when there is one.

**Summary** lists each project's tasks remaining, their average age, how much is complete and a bar, sub-projects
indented. A project's name opens its tasks in the Tasks view. `summary.all.projects=1` includes finished projects.

**Calendar** draws the months with week numbers, today, weekends, the days that have something due (green; due today
in purple; overdue in red) or scheduled, and holidays. It takes Taskwarrior's arguments (`calendar y`, `calendar due`,
`calendar 3 2027`, `calendar march 2027`) in the page's box, and has buttons for earlier, later, today, a year and
"from the first due date". A filter-shaped word (`project:Work`) narrows which tasks colour the days. Click a day to see
what is due in the `list` report. Holidays are defined in the taskrc as `holiday.<id>.name` and `.date` (or `.start`
and `.end`).

**Burndown** charts pending, started and done over time with the net fix rate and an estimated completion date, by day,
week, month or year. It counts as Taskwarrior does, quirks included (checked bar by bar). `burndown.cumulative=0`
stops carrying finished tasks forward.

## Your taskrc

UDAs, custom reports, contexts and your other settings live in `~/.taskrc`, not in your synced data, so the app keeps
its own copy. Open **taskrc**, paste or choose your file, and save. Only an allowlist of settings is kept (the full
list, with what each does, is [taskrc support](taskrc-support.md)).

- **Sync settings and anything credential-like are blocked** (`sync.*`, `taskd.*`, names containing `secret`,
  `password`, `token`, …). Only the *names* of what was dropped are shown; the raw text is never stored.
- What is saved is shown back, so each import edits it instead of replacing it. **Restore previous** undoes the last save.
- `include` lines can't be followed; paste the included files too.
- **A context** (`context.<name>.read` / `.write`) filters what you see and tags what you add. It can also carry its
  own settings, `context.<name>.rc.<key>`, in force while it is active; as in Taskwarrior they win over everything,
  a command-line `rc.` override included.
- A UDA that exists on a task but isn't defined in your taskrc is shown but **read-only**.

## Urgency

A task's urgency is the sum of Taskwarrior's `urgency.*` terms, with its built-in coefficients unless your taskrc
changes them. Open **urgency** to see every coefficient next to its default, edit it, add your own, or send one back to
its default. Edits are saved with your imported settings (as `urgency.*` lines in the taskrc dialog). Your desktop
`~/.taskrc` is not touched, so copy any change you want there. It follows Taskwarrior's rules, so nothing is inherited
that Taskwarrior doesn't inherit:

- Coefficients add up: a task gets every matching coefficient, not just the most specific.
- `urgency.user.project.Home.coefficient` applies to `Home` and `Home.Kitchen`, but not to `Homework`.
- Tags match user and virtual tags (`urgency.user.tag.OVERDUE.coefficient`); keywords match the description,
  case-sensitively; `urgency.uda.<name>.coefficient` matches any value of a UDA, `….<value>.coefficient` one value.
- `urgency.inherit` is off, as in Taskwarrior. On, a task that blocks others takes the highest urgency of what it
  blocks, through the whole chain, plus 0.01.

`rc.urgency.due.coefficient:0 next` and `rc.urgency.inherit:1` also work for a single command.

## Recurring tasks

```
add Water plants recur:weekly due:friday
add Pay rent recur:monthly due:eom until:2027-12-31
3 modify recur:2w                      # change the period of a recurring task
```

Periods are Taskwarrior's: `daily` `weekdays` `weekly` `biweekly` `monthly` `quarterly` `yearly`, counts such as `3d`
`2w` `6mo`, and ISO durations like `P1M`. A repeating task shows a repeat icon; its detail view lists its instances, and
an instance links back to it. The recurring task itself is a template: you can edit it but not complete or start it.

- **Recurrence is on by default**, as in Taskwarrior (`recurrence=1`). Before each command the app creates the
  instances that are due (`recurrence.limit=N` keeps N upcoming ones, default 1), retires finished series and honours
  `until`. Taskwarrior itself advises that when several clients sync, one is primary and the others set
  `recurrence=0`, as a workaround for a duplication bug. Instances are numbered by their index in the template's mask,
  so the CLI and the web app run side by side without duplicating each other in normal use (checked against real
  `task` 3.5.0), but if you ever see duplicates, set `recurrence=off` here or in your CLI's taskrc so only one side
  generates them. With it off the app still shows and edits recurring tasks and reads instances made elsewhere.
- **Editing one task of a series** follows `recurrence.confirmation`: `prompt` (the default) asks whether to change
  all pending recurrences or only this task; `yes` always changes the whole series; `no` only the task you edited.
  Descriptive changes (description, project, priority, tags, UDAs) are shared; dates and the period stay per task.
- Deleting a recurring task asks first, because it deletes its open instances too.
- `recurrence.indicator` (default `R`) is what the `recur.indicator` column shows.

## Hooks

Taskwarrior runs your scripts when a command starts, when a task is added or changed, and when it ends. A Worker can't
run scripts, so here a hook is a **Rust function you write in `crates/tc-core/src/my_hooks.rs`** and deploy with the
Worker. That file ships with four commented-out placeholders, so nothing happens until you fill one in. (The code that
runs them, `hooks.rs`, is separate on purpose: see [Keeping your hooks across updates](#keeping-your-hooks-across-updates).)

| Hook | Runs | It can |
| --- | --- | --- |
| `on_launch(h, command)` | before a command does anything | refuse it (`Err("…")`) |
| `on_add(h, task)` | for each new task, before it is saved | return the task changed, or refuse |
| `on_modify(h, old, new)` | for each task `modify`, `done`, `delete`, `start`, `stop`, `annotate`… changes, before it is saved | return the new task changed, or refuse |
| `on_exit(h, changed)` | after the command, with the tasks it changed | only talk |

A task is a `Facts`, the plain view reports use: `description`, `project`, `priority`, `tags`, the dates (`due`,
`wait`, …), `depends`, `annotations`, UDAs in `extra`, and the read-only `status`, `blocked`, `blocking`. A hook gets
the task and hands one back; whatever differs is applied, in the same undo step as the command. A hook may change what
`modify` can (description, project, priority, tags, dates, `depends`, UDAs); `uuid`, `status`, annotations and the
recurrence fields are read-only, and changing them is an error. A refused command saves nothing.

`h.say("…")` and `h.warn("…")` print. The lines appear under the command's result in the Console (a toast elsewhere);
`println!` goes nowhere in WebAssembly.

```rust
fn on_add(&self, h: &mut Hooked, mut task: Facts) -> Result<Facts, Reject> {
    if task.project.as_deref() == Some("Work") {
        task.tags.insert("office".into());
        h.say("Tagged +office.");
    }
    Ok(task)
}

fn on_modify(&self, _h: &mut Hooked, old: &Facts, new: Facts) -> Result<Facts, Reject> {
    if old.status == "pending" && new.status == "completed" && old.blocked {
        return Err("Finish what it depends on first.".into());
    }
    Ok(new)
}
```

Locally, `npm run dev` rebuilds the Worker when you save `my_hooks.rs`; for the deployed one, run `npm run deploy`.

Notes: hooks run inside the Worker with no network or files, and `hooks=off` (or `rc.hooks:off`) turns them all off.
`undo` and the recurring instances created in the background don't run hooks, as in Taskwarrior. A command that asks a
question first runs `on_launch` once per round. Each hook you add makes the Worker a little bigger; a few hundred lines
cost a few KB compressed, while a new dependency can cost far more.

### Keeping your hooks across updates

Your hooks are the only part of the code you change, and they live in one file, `my_hooks.rs`, apart from the engine
that runs them (`hooks.rs`). Keep them as **one commit on a branch of their own**, and an update from upstream becomes a
rebase that almost never has anything to resolve. This assumes you cloned or forked the project with git; `upstream`
below is the project's repository.

```bash
# Once: keep `main` an untouched copy of upstream, and your hooks on a branch.
git remote add upstream <the project's URL>        # skip if `origin` already is upstream
git switch -c my-hooks
#   …edit crates/tc-core/src/my_hooks.rs…
git commit -am "My hooks"
npm run deploy                                      # deploy from this branch
```

```bash
# Each update:
git fetch upstream
git switch main && git merge --ff-only upstream/main   # main stays identical to upstream
git switch my-hooks && git rebase main                 # your hooks, replayed on the new code
npm run deploy
```

Because `main` is exactly upstream, `git diff main my-hooks` is always precisely your hooks, and a rebase only stops if
upstream changed the *same lines* of `my_hooks.rs` (say, it reworded a placeholder or changed a hook's signature). Then
git marks the file; fix it by hand, `git add crates/tc-core/src/my_hooks.rs`, `git rebase --continue`. Run
`git config rerere.enabled true` once and git remembers how you resolved it. If upstream changes what a hook receives, the
build fails in `my_hooks.rs` and the compiler says what to adjust.

**As a patch file**, for keeping your hooks outside the clone, or moving them to a fresh checkout or another machine:

```bash
# Save: your commit as a patch (keeps its message), or just the file's changes.
git format-patch main..my-hooks -o ~/my-hooks-patches
git diff main my-hooks -- crates/tc-core/src/my_hooks.rs > ~/my-hooks.patch

# Restore, on a clean, up-to-date checkout of upstream:
git am -3 ~/my-hooks-patches/*.patch               # as a commit; -3 lets git merge if the file has moved on
git apply --3way ~/my-hooks.patch                  # or as an uncommitted change (it is staged for you)
```

Either way a conflict is shown as ordinary `<<<<<<<` markers in `my_hooks.rs`. Keep the patch in your own repository or
dotfiles next to your `.deploy.vars`; it is a few dozen lines, easy to read in review, and it is the whole of your
customisation.

## Time tracking and reminders

- Add `journal.time=on` to your taskrc. `start` and `stop` then record "Started task" / "Stopped task" (the texts are
  configurable), and the detail view shows a sessions table with a running total. A running task shows a timer in the
  header and the tab title.
- The **bell** turns on reminders: a heads-up before a timed due date, a morning reminder for dates without a time,
  tasks coming off `wait`, and one digest a day for what's already overdue. Reminders need the **tab to be open** (the
  app can't wake up in the background), and system notifications need HTTPS or `localhost` plus your permission.

## On a phone

The layout adapts below about 760 px: the report list moves into a menu (☰), tasks become cards with big buttons, the
header sort becomes a **Sort** picker, forms and the detail view go full screen, and a **+** button adds a task. The
prompt gets **Complete** (⇥) and **Previous command** (⌃) buttons since there's no Tab or arrow key, and completions
can be tapped. You can install it to your home screen from the browser menu.

## Different from the desktop

- **Numeric ids belong to this app** (pending tasks numbered by creation time), so they won't match your laptop's. Use
  uuid prefixes where it matters.
- **`undo`** works on the last few commands made while this Worker instance lives; it is forgotten when it is recycled.
- **Hooks are Rust, not scripts** (`on-add`, `on-modify`, …): a Worker can't run local programs, so they are functions in
  `my_hooks.rs` that you edit and redeploy. See [Hooks](#hooks).
- **`edit`, `purge`, `history.*`** are not in the app.
- **The first request after a quiet spell** is slower: the Worker rebuilds its state from the bucket's newest snapshot.
