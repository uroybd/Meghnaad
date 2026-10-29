# Settings

**Settings** in the header has a tab each for **General**, the **taskrc**, **tasksh**, **Commands**, **Hooks** and
**Urgency**. Every tab has its own **Apply** (**Save**, on Hooks), and applying one doesn't touch the others. A tab with
unapplied edits has a dot, and the edits stay while you look at another tab or page. If the saved settings change
elsewhere (the Console's `config`, another tab), a tab with no edits of its own follows them.

**General** sets the **home page**, the page the app opens on: any report (including your own) or any of your boards.
The default is `next`. It is kept in the app's own `settings.json` (`"homepage": { "report": "list" }` or `{ "board":
"week" }`), not your taskrc, so it is the same on every device that signs in to the same deployment. A link to a task
still opens that task, and a report or board that is gone opens `next`. (The taskrc's `default.command` is different: it
is the command a line runs when it names none.)

![The Settings page, on its taskrc tab: the saved settings as text to edit, with a colour theme picker and import from a file or a link](../screenshots/settings-taskrc.png)

(The screenshot predates the list of files beside the text, described under [Your taskrc](#your-taskrc).)

## Your taskrc

UDAs, custom reports, contexts, holidays and your other settings live in `~/.taskrc`, not in your synced data, so the app
keeps its own copy: **a folder of files**, kept as you wrote them. Open the **taskrc** tab. The files are on the left
(the main one, `taskrc`, first); choose one to edit it on the right, then **Apply**. Only an allowlist of settings is
read; the full list is in [taskrc support](../taskrc-support.md).

### The editor

The editor numbers the lines and colours each part: a setting's name, its value, a comment, `include` and its file, and
the `[unsupported]` the app writes where it commented a line out. A line that is no setting at all is underlined (it is
commented out when you Apply), and a credential-like name is struck through (it is removed). A line the engine has a
warning for, such as an `include` that points nowhere, is marked in the line numbers. On the web it is CodeMirror
(Tab indents, Ctrl+Z undoes within one file); on the phone it is a text box with the same colours. The engine says what
each piece is, so both agree.

### What is kept and what is not

Text is kept as written: comments, blank lines, order and spacing. Two things change on the way in:

- **A line the app can't use is commented out** (`# [unsupported] some.key=1`) rather than dropped, and the result says
  which lines and why.
- **Sync settings and anything credential-like are removed** (`sync.*`, `taskd.*`, names containing `secret`,
  `password`, `token`, …), commented-out ones included. They are never stored, logged, shown or synced, and only the
  *names* of what was removed are reported.

A UDA that exists on a task but isn't defined in your taskrc is shown but **read-only**.

### `include`

It works as in Taskwarrior: `include holidays/en-US.rc` reads that file where the line stands, so later lines win. Write
the path from the file that has the line (`include ../taskrc` from inside a folder); includes nest up to ten levels. An
include that points nowhere, outside the folder or at an absolute path is reported with its file and line, and the rest
is still read. `include dark-256.theme` (Taskwarrior's own themes) becomes that theme's lines when you apply. A file no
include reaches is kept but marked *unused*.

### Files

#### Add a file

Type its name (`holidays/en-US.rc`) and press **Add**, or use **Add files** to upload several.

#### Import folder

Uploads a whole folder (your dotfiles checkout). `.taskrc` or `taskrc` at its top becomes the main
taskrc. Hidden files and folders (`.git`), binary files and anything over 256 KB are left out and listed, and up to 64
files or 1 MB are taken. It adds to what is here; tick **Replace what is here** to make the folder the whole of it (it
needs a `taskrc`). One import is one step, so **Restore previous** undoes all of it.

#### Download

Keeps the file shown (under its own name) and **Download all (.zip)** every file, keeping folders. They
hold what is *saved*, not unapplied edits, and never a credential. **Import folder** takes an unzipped copy back. On a
phone the system's save dialog asks where to keep it.

#### Include this file in…

On any file but the main one, this adds `include <path>` to the file you choose and shows it;
**Apply** keeps it. **Delete file** removes one (the main taskrc can only be emptied).

#### `config` and the settings pages

They write to the main taskrc, as `task config` does: a line starting with `name=`
changes in place and keeps its comment, a new setting goes on the end, and removing one deletes those lines. A setting
an included file defines isn't edited there: the new line overrides it, or you edit that file to remove it.

#### Restore previous

Undoes the last save, import or delete (all files together).

### Import from a link

Keep your dotfiles repository as the source: paste the address of the file (a GitHub or GitLab file page works, as does a
raw link or a gist's) and choose **Import**.

- It brings the main taskrc only. Included files aren't fetched, so add them (they show as *not found* until you do).
- The Worker fetches it, so the site's CORS rules don't matter, and the file never reaches the browser. It keeps what a
  pasted file would keep.
- It saves straight away (**Restore previous** undoes it) and doesn't remember the link, so import again to refresh.
- Only `https://` links to public sites are followed, with no addresses, ports or user names in the link (the same for any
  redirect), up to 256 KB. The request carries no login, so the link must work without one (a secret gist's does). A web
  page rather than the file is refused.

### Add holidays

This makes a file of a country's public holidays from one of three sources, and shows the list it found before anything is
saved:

#### Holidata

From [holidata.net](https://holidata.net), the data behind Taskwarrior's own holiday files: 42 locales in 34
countries, with the language where a country has several. Saved as `holidays/<locale>.rc` (`holidays/en-US.rc`).

#### Nager.Date

From [date.nager.at](https://date.nager.at): about 200 countries, saved as `holidays/nager-<code>.rc`. For
some countries it lists only holidays that fall on the same date every year, so ones that follow the moon (Eid) can be
missing: look the list over.

#### Calendar link (.ics)

A public feed such as Google's holiday calendars
(`https://calendar.google.com/calendar/ical/en.bd%23holiday%40group.v.calendar.google.com/public/basic.ics`;
`webcal://` works too). The Worker fetches it under the same rules as the taskrc link import (https, a public host, up
to 1 MB). All-day events become holidays, a multi-day one a stretch (`.start` and `.end`), and a plain yearly repeat is
carried over the years. A **Keep** list has every name; what the calendar calls an observance (Valentine's Day) starts
unticked. The file is named after the calendar unless you give a name.

Pick the country (or paste the link), the years (last year to two years ahead to begin with) and **Find holidays**.
Holidays of one region only (Massachusetts' Patriots' Day, a Swiss canton's) are left out unless you tick the region.

**Save** writes the file and, unless they are there already, adds `include holidays/<file>.rc` and
`calendar.holidays=full` to the main taskrc (you can untick them). If the main taskrc has unapplied edits, the lines go
into its box and **Apply** keeps them. A file made this way records where it came from, so **Update holidays** on that file
fetches it again for the years now with the same regions and names (a year with no data is named). A calendar's link is
remembered only if it has no `?`, since such a link can hold a secret. Saving again replaces the file, and **Restore
previous** undoes it.

Holidata and Nager.Date are asked from your browser (or phone) and learn your IP address and the country and years you
asked for, nothing else. A calendar's host learns the Worker's (or the phone's) address and the link.

### Contexts

A context (`context.<name>.read` / `.write`) filters what you see and tags what you add. It can also carry settings,
`context.<name>.rc.<key>`, in force while it is active. As in Taskwarrior they win over everything, including a
command-line `rc.` override.

## Colours

Rows are coloured by Taskwarrior's colour rules, and the same colours reach the calendar, burndown, summary bar and
history graph. With nothing set you get **Meghnaad's own theme**:

- the active task on soft green, overdue in coral, due **today** in bold amber (due later this week is plain white, so
  today's tasks stand out),
- blocked tasks dimmed, recurring ones in orchid, `+next` marked, and no zebra,
- **priority** coloured on its own cell (`color.uda.priority.H`, `.M`, `.L`: coral, amber, green), whatever colour the rest
  of the row has. Taskwarrior's rules colour a whole row; colouring one field is the app's addition, from the same
  settings.

Colours don't blend (`rule.color.merge=no`): a row takes the colour of the first rule in `rule.precedence.color` that
applies, so a keyword or tag rule you add replaces that colour instead of mixing. Put `rule.color.merge=yes` in your
taskrc to blend as Taskwarrior does. Your `color.*` lines work as on the desktop:

```
color.active=bold on sage                  # a rule: `sage` is a softer name for green
color.tag.work=sky                         # a tag, `project.Home`, `keyword.milk`, `uda.estimate.big` likewise
color.overdue=bold rgb500
rule.precedence.color=overdue,active,tag.  # which rule wins first; with `rule.color.merge=yes` the rules blend instead
color.alternate=on gray2                   # shade every other row
```

The **taskrc** tab has a **Colour theme** picker (Meghnaad's own, Taskwarrior's default and the 16 standard themes such as
`dark-256` and `solarized-light-256`) and a **Colours** switch. Choosing a theme replaces the colour lines in the box, and
Apply keeps them. A pasted `include dark-256.theme` is read as that theme. `color=off` (or `rc.color:off`) turns every
colour off, and a task tagged `nocolor` is never coloured.

`colors` in the Console draws the palette, `colors bold coral on sky` a sample, and `colors legend` every colour in force,
each in its own colour. Colours are Taskwarrior's palette (`rgb440`, `gray12`, `color214`, `on bright blue`); the basic
ones are drawn softly for the page, and a colour that wouldn't show on it is nudged toward readable. The friendly names
are `ink`, `coral`/`rose`, `sage`/`moss`, `amber`/`sand`, `sky`/`ocean`, `orchid`/`plum`, `teal`/`aqua`, `snow`/`pearl` and
`slate`. [taskrc support](../taskrc-support.md#colour) lists every colour setting.

## Commands

A command is a word that stands for a whole command line. Define it under **Settings → Commands**: a name, what it stands
for, and optionally an icon. Any [Lucide](https://lucide.dev/icons) icon works, picked by sight, by name, or by what it is
for ("cancel" finds the circled cross). The app carries only the few icons it uses itself; any other is fetched from the
jsdelivr CDN when first shown and remembered by your browser (see *Icons from a CDN* in the README's security notes).
With no network, an icon you haven't used shows as a terminal, and the picker lets you type its name instead.

`fail` for `done outcome:fail` works like this:

```
fail                      → done outcome:fail
task fail                 → the same: the leading `task` is optional
3 fail                    → 3 done outcome:fail
project:Home +work fail   → project:Home +work done outcome:fail
```

What comes before the command word selects the tasks, as it always does (and the active context applies). What comes
after is added to what the command stands for. A command may carry a filter of its own: `late` for `+OVERDUE list` makes
`project:Home late` read `project:Home +OVERDUE list`. Only the command word can be one, so `add fail the test` still
adds a task called "fail the test", and a command used inside another is written out too. A name is written out once on a
line, which lets `done` stand for `done outcome:ok` and still reach Taskwarrior's `done`.

**A command wins over a Taskwarrior command, a report or a taskrc `alias.*` of the same name.** It differs from a taskrc
alias in two ways: it is yours alone (kept in this app's `settings.json`, not the taskrc that mirrors your desktop one),
and it reads the line as Taskwarrior does, so it applies only where the command goes.

**Several commands.** `wrap` for `done outcome:fail && modify +failed` runs the second only if the first worked; `||` and
`;` work as on the console line. The filter before the name goes in front of every one of them (`3 wrap` is `3 done
outcome:fail && 3 modify +failed`, so use a uuid rather than a number when an earlier step changes what the number
means), and what follows the name goes after the last (`3 wrap +x` ends with `3 modify +failed +x`). A button runs each
on the task it is for, by uuid, and stops where `&&` would.

**Buttons.** Every command is also a button: the **⋯** menu on a task (in the table and the drawer) runs it on that task,
and the bar over selected tasks runs it on all of them, a batch at a time. The menu isn't there while you have no
commands. Where a table is wider than its box (a narrow board card, say), each row's buttons (Done, Start or Stop, Edit,
Delete, then the commands) fold into that menu, and open out again when there's room.

**Names** are one word that can't be taken for a filter: no spaces, no `+` `-` `/`, no `:` or `=`, not a number.

![The Commands tab: fail, later and urgent, each with a name, what it stands for and an icon](../screenshots/settings-commands.png)

![The icon picker, searching for "cancel": the circled cross is among the results](../screenshots/icon-picker.png)

![The menu under the ellipsis on a task: fail, later and urgent, each with its icon and what it runs](../screenshots/task-commands-menu.png)

The tab shows the `settings.json` it will save. Other keys in the file are kept, and entries it can't use are listed and
dropped when you apply. The file is at most 64 KB.

## tasksh

![The tasksh tab: the reviewed attribute and the _reviewed report are not defined, the review filter and order, and the six lines Apply would write](../screenshots/settings-tasksh.png)

**Review** is tasksh's `review`: it walks the tasks you haven't looked at lately, one at a time, and you mark each as
reviewed, skip, edit, change, finish or delete it. Start it with **Review** in the top bar (a card with buttons) or by
typing `review` in the Console. `review 10` stops after ten (a limited review is done from the Console), and `rev` is
enough, as in tasksh. As in tasksh the menu is keybindings, not a shell: a key is the answer, with no Enter after it, and
the same choices are buttons.

### What it needs

Two things in the taskrc, which the **tasksh** tab prepares: a `reviewed` attribute (a date UDA recording when you last
looked at a task) and a `_reviewed` report (one `uuid` column) listing the tasks due for review. If they're missing, the
first review asks, in tasksh's words, whether to define them, and does so with the lines below.

The tab says whether each is defined, defined differently (showing what is there now, since applying changes it) or
missing, and lists the exact `config` lines **Apply** would write, the ones tasksh writes the first time it runs. Nothing
else in the taskrc is touched. You can adjust the report's **filter** (by default never reviewed, or not for 6 days, and
still pending or waiting) and its **order**; **Use tasksh's defaults** puts them back.

![A review in the Console: the task, its details, and the prompt asking what to do with it](../screenshots/review-console.png)

![The review card: the task's details and its choices](../screenshots/review-card.png)

### Reviewing

Each task is shown with `[3 of 12]` and everything `info` shows, then the menu:

| Key | |
| --- | --- |
| Enter | Mark it reviewed (`modify reviewed:now`) |
| `s` | Skip it: on to the next, not counted as reviewed |
| `e` | Edit it in the app's editor; closing the editor marks it reviewed and shows it again |
| `m` | Type what to change (`+tag -tag /teh/the/ project:X`), then Enter; Escape cancels. The same task is shown again |
| `c` / `d` | Complete or delete it; both count as reviewed |
| `q` | Stop. What is done stays done, and you can start again where the list left off |

At the end: `End of review. 3 out of 12 tasks reviewed.`

In the Console the prompt is off while the review asks (it reads `review>`), a bar of buttons stands over it, and Escape
quits; the only thing you type is the change after `m`. **Open as card** in that bar carries on in the card from the same
place. In the card the **Reviewed** button has the focus, so Enter marks it, as in tasksh, and the same letters work as keys.

Questions Taskwarrior asks mid-step are asked here too, with Yes and No (or `y` and `n`). For a recurring task: "This is a
recurring task. Do you want to modify all pending recurrences of this same task?" Yes marks the whole series, No only
this one. (`rc.confirmation:no` doesn't silence it, since `recurrence.confirmation` governs it, so tasksh shows it too.)

Every step is an ordinary command (`task <uuid> modify reviewed:now`, `done`, `delete`) sent with `rc.confirmation:no`, so
hooks run, `undo` works and the Tasks table follows. Review steps aren't added to the command history.

## Urgency

![The Urgency tab: the inherit switch, and each coefficient next to its default](../screenshots/settings-urgency.png)

A task's urgency is the sum of Taskwarrior's `urgency.*` terms, with its built-in coefficients unless your taskrc changes
them. The **Urgency** tab shows every coefficient next to its default. Edit one, add your own, or send one back to its
default, then **Apply**. Edits are saved with your imported settings (as `urgency.*` lines in the taskrc tab). Your desktop
`~/.taskrc` isn't touched, so copy any change you want there.

It follows Taskwarrior's rules, so nothing is inherited that Taskwarrior doesn't inherit:

- Coefficients add up: a task gets every matching coefficient, not just the most specific.
- `urgency.user.project.Home.coefficient` applies to `Home` and `Home.Kitchen`, but not `Homework`.
- Tags match user and virtual tags (`urgency.user.tag.OVERDUE.coefficient`). Keywords match the description,
  case-sensitively. `urgency.uda.<name>.coefficient` matches any value of a UDA, and `….<value>.coefficient` one value.
- `urgency.inherit` is off, as in Taskwarrior. When on, a task that blocks others takes the highest urgency of what it
  blocks, through the whole chain, plus 0.01.

`rc.urgency.due.coefficient:0 next` and `rc.urgency.inherit:1` also work for a single command.
