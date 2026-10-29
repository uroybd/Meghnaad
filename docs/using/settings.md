# Settings

**Settings** in the header opens one page with a tab each for **General**, the **taskrc**, **tasksh**, **Commands**, **Hooks** and **Urgency**. Every tab has
its own **Apply** (**Save**, on Hooks), and applying one does not touch the others. A tab with edits that are not applied yet has a dot, and
the edits stay while you look at another tab or another page. If the saved settings change from somewhere else (the
Console's `config`, another tab), a tab with no edits of its own follows them.

**General** chooses the **home page**: the page the app opens on. It can be any report (including your own) or any of your
boards; left alone it is `next`. It is kept in the app's own `settings.json` (`"homepage": { "report": "list" }` or
`{ "board": "week" }`), not in your taskrc, so it is the same on every device that signs in to the same deployment. A link
to a task still opens that task, and a report or board that is gone opens `next`. (The taskrc's `default.command` is
different: it is the command a line runs when it names none.)

![The Settings page, on its taskrc tab: the saved settings as text to edit, with a colour theme picker and import from a file or a link](../screenshots/settings-taskrc.png)

(The screenshot predates the list of files beside the text, described under [Your taskrc](#your-taskrc).)

## Your taskrc

UDAs, custom reports, contexts, holidays and your other settings live in `~/.taskrc`, not in your synced data, so the app
keeps its own copy: **a folder of files**, kept as you wrote them. Open **Settings**, then its **taskrc** tab. On the left
are the files (the main one, `taskrc`, first); choose one to edit it on the right, then **Apply**. Only an allowlist of
settings is read (the full list, with what each does, is [taskrc support](../taskrc-support.md)).

- **The editor** numbers the lines and colours what each one is: a setting's name, its value, a comment, `include` and the
  file it names, the `[unsupported]` the app writes where it commented a line out. A line that is no setting at all is
  underlined (it is commented out when you Apply), and a credential-like name is struck through (it is removed). A line the
  engine has a warning for (an `include` that points nowhere) is marked in the line numbers. On the web it is CodeMirror
  (loaded with this tab; Tab indents, Ctrl+Z undoes within one file), on the phone a text box with the same colours and
  numbers; the engine says what each piece is, so both agree.
- **Text is kept as written**: comments, blank lines, order and spacing stay. Two things change on the way in. **A line the
  app can't use is commented out** (`# [unsupported] some.key=1`) instead of being dropped, and the result says which lines
  and why. **Sync settings and anything credential-like are removed** (`sync.*`, `taskd.*`, names containing `secret`,
  `password`, `token`, …), a commented-out one included: they are never stored, logged, shown or synced, and only the
  *names* of what was removed are reported.
- **`include` works as in Taskwarrior**: `include holidays/en-US.rc` reads that file where the line stands, so later lines
  win, with the path written from the file that has the line (`include ../taskrc` from inside a folder), nested up to ten
  levels. An include that points nowhere, outside the folder or at an absolute path is reported with its file and line,
  and the rest is still read. `include dark-256.theme` (Taskwarrior's own themes) becomes that theme's lines when you
  apply. A file no include reaches is kept but marked *unused*.
- **Add a file** by typing its name (`holidays/en-US.rc`) and **Add**, or **Add files** to upload several. **Import
  folder** uploads a whole folder (your dotfiles checkout): `.taskrc` or `taskrc` at its top becomes the main taskrc, hidden
  files and folders (`.git`), binary files and anything over 256 KB are left out and listed, and up to 64 files or 1 MB are
  taken. It adds to what is here; tick **Replace what is here** to make the folder the whole of it (it needs a `taskrc`).
  One import is one step, so **Restore previous** undoes all of it.
- **Download** keeps the file shown (under its own name, `en-US.rc`) and **Download all (.zip)** every file in one zip that
  keeps the folders (`taskrc`, `holidays/en-US.rc`). They hold what is *saved*, not edits in a box you haven't applied, so
  they never carry a credential (those were removed on the way in). Unzip it and **Import folder** takes it back. On the
  phone, the system's save dialog asks where to keep it.
- **Add holidays** makes a file of a country's public holidays, from one of three places, and shows the list it found before
  anything is saved:
  - **Holidata** ([holidata.net](https://holidata.net), the data behind Taskwarrior's own holiday files): 42 locales in 34
    countries, the language too where a country has several. Saved as `holidays/<locale>.rc` (`holidays/en-US.rc`).
  - **Nager.Date** ([date.nager.at](https://date.nager.at)): 200 or so countries, saved as `holidays/nager-<code>.rc`
    (`holidays/nager-BD.rc`). For some countries it lists only the holidays that fall on the same date every year, so the
    ones that follow the moon (Eid) can be missing: look the list over.
  - **Calendar link (.ics)**: a public calendar feed, such as Google's holiday calendars (their addresses look like
    `https://calendar.google.com/calendar/ical/en.bd%23holiday%40group.v.calendar.google.com/public/basic.ics`; `webcal://`
    works too). The Worker fetches it for the page, under the rules the taskrc link import has (https, a public host, up to
    1 MB). All-day events become holidays, one that lasts several days a stretch (`.start` and `.end`), and a plain yearly
    repeat is carried over the years. A calendar's **Keep** list has every name in it; what the calendar calls an
    observance (Valentine's Day) is unticked to begin with. The file is named after the calendar unless you give a name.
  Pick the country (or paste the link), the years (last year to two years ahead to begin with) and **Find holidays**.
  Holidays that belong to one region only (Massachusetts' Patriots' Day, a Swiss canton's) are left out unless you tick the
  region. **Save** writes the file and, unless they are there already, adds `include holidays/<file>.rc` and
  `calendar.holidays=full` to the main taskrc, which you can untick. If the main taskrc has edits you have not applied,
  the lines go into its box instead and **Apply** keeps them. A file made this way says where it came from, so **Update
  holidays**, on that file, fetches it again for the years now with the same regions and names (a year the source has no
  data for is named; a calendar's link is remembered only when it has no `?` in it, since such a link can hold a secret).
  Saving again replaces that file; **Restore previous** undoes it. Holidata and Nager.Date are asked from your browser (or
  phone), and learn your IP address and the country and years you asked for, nothing else; the calendar's host learns the
  Worker's (or the phone's) address and the link.
- **Include this file in…** (on any file but the main one) adds `include <path>` to the file you choose, written from that
  file, and shows it; **Apply** keeps it. **Delete file** removes one (the main taskrc can't be deleted, only emptied).
- **`config` and the settings pages write to the main taskrc**, as `task config` does: a line that starts with `name=`
  changes in place and keeps its comment, a new setting goes on the end, removing one deletes those lines. A setting an
  included file defines is not edited there: the new line overrides it, or, to remove it, you edit that file.
- **Restore previous** undoes the last save, import or delete (all the files together).
- **Import from a link** keeps your dotfiles repository as the source: paste the address of the file (a GitHub or GitLab
  file page works, as does a raw link or a gist's) and choose **Import**. It brings the main taskrc only; files it
  includes are not fetched, so add them (they show as *not found* until you do). The Worker fetches it, so the site's CORS
  rules don't matter, and keeps what a pasted file would keep; the file itself never reaches the browser. It saves
  straight away (**Restore previous** undoes it) and does not remember the link, so import again to refresh. Only
  `https://` links to public web sites are followed (no addresses, ports or user names in the link; the same for any
  redirect), up to 256 KB. The request carries no login, so the link must work without one (a secret gist's does). A
  web page rather than the file is refused.
- **A context** (`context.<name>.read` / `.write`) filters what you see and tags what you add. It can also carry its
  own settings, `context.<name>.rc.<key>`, in force while it is active; as in Taskwarrior they win over everything,
  a command-line `rc.` override included.
- A UDA that exists on a task but isn't defined in your taskrc is shown but **read-only**.

## Colours

Rows are coloured by Taskwarrior's colour rules, and the same colours reach the calendar, the burndown and the summary
bar, and the history graph. With nothing set you get **Meghnaad's own theme**: the active task on a soft green,
overdue in coral, due **today** in bold amber (due later this week is plain white, so today's tasks are the ones that
stand out), blocked ones dimmed, recurring ones in orchid, `+next` marked, and no zebra. **Priority** is coloured on its
own cell (`color.uda.priority.H`, `.M`, `.L`: coral, amber, green), whatever colour the rest of the row has, and is not a row
rule. (Taskwarrior's rules colour a whole row; colouring one field is the app's own addition, from the same settings.) Colours do not
blend (`rule.color.merge=no`): a row takes the colour of the first rule in `rule.precedence.color` that applies to it,
so its look follows its state, and a keyword or tag rule you add replaces that colour instead of mixing with it. Put
`rule.color.merge=yes` in your taskrc to blend as Taskwarrior does. Everything is Taskwarrior's, so your `color.*` lines
work as they do on the desktop:

```
color.active=bold on sage                  # a rule: `sage` is a softer name for green
color.tag.work=sky                         # a tag, `project.Home`, `keyword.milk`, `uda.estimate.big` likewise
color.overdue=bold rgb500
rule.precedence.color=overdue,active,tag.  # which rule wins first; with `rule.color.merge=yes` the rules blend instead
color.alternate=on gray2                   # shade every other row
```

The **taskrc** tab has a **Colour theme** picker (Meghnaad's own, Taskwarrior's default, and the 16 standard
themes such as `dark-256` and `solarized-light-256`) and a **Colours** switch; choosing a theme replaces the colour lines
in the box, and Apply keeps them. A pasted `include dark-256.theme` line is read as that theme. `color=off` (or `rc.color:off`)
turns every colour off, and a task tagged `nocolor` is never coloured.

`colors` (Console) draws the palette, `colors bold coral on sky` a sample, and `colors legend` every colour in force, each
in its own colour. Colours are Taskwarrior's palette (`rgb440`, `gray12`, `color214`, `on bright blue`); the basic ones are
drawn softly for the page, and a colour that would not show on it is nudged toward readable. The friendly names are
`ink`, `coral`/`rose`, `sage`/`moss`, `amber`/`sand`, `sky`/`ocean`, `orchid`/`plum`, `teal`/`aqua`, `snow`/`pearl`,
`slate`. [taskrc support](../taskrc-support.md#colour) lists every colour setting.

## Commands

A command is a word that stands for a whole command line. Define it in **Settings**, then **Commands**: a name, what it
stands for, and optionally an icon (any [Lucide](https://lucide.dev/icons) icon, picked by looking at it, by its name or by what it is
for: "cancel" finds the circled cross).
The app carries only the few icons it uses itself; any other is fetched from the jsdelivr CDN when it is first shown and
remembered by your browser (see *Icons from a CDN* in the README's security notes). With no network, an icon you have not
used before shows as a terminal, and the picker lets you type an icon's name instead of choosing it.
`fail` for `done outcome:fail` works as

```
fail                      → done outcome:fail
task fail                 → the same: the leading `task` is optional
3 fail                    → 3 done outcome:fail
project:Home +work fail   → project:Home +work done outcome:fail
```

What comes before the command word selects the tasks, as it always does (and so does the active context); what comes
after is added to what the command stands for. A command may carry a filter of its own: `late` for `+OVERDUE list`
makes `project:Home late` read `project:Home +OVERDUE list`. Only the command word can be one, so `add fail the test`
still adds a task called "fail the test", and a command used inside another is written out too. A name is written out
once on a line, which lets `done` stand for `done outcome:ok` and still reach Taskwarrior's `done`.

**A command wins over a Taskwarrior command, a report or a taskrc `alias.*` with the same name.** It is unlike a taskrc
alias in two ways: it is yours alone (it is kept in this app's `settings.json`, not in the taskrc that mirrors your
desktop one), and it reads the line the way Taskwarrior does, so it only applies where the command goes.

A command may be several commands: `wrap` for `done outcome:fail && modify +failed` runs the second only if the first
worked, and `||` and `;` work as on the console line. The filter before the name goes in front of every one of them
(`3 wrap` is `3 done outcome:fail && 3 modify +failed`; so use a uuid, not a number, when an earlier step changes what the
number means), and what comes after the name goes after the last (`3 wrap +x` ends with `3 modify +failed +x`). A
button runs each on the task it is for, by its uuid, and stops where `&&` would.

Every command is also a button: the **⋯** menu on a task (in the table, and in the drawer) runs it on that task, and the
one in the bar over selected tasks runs it on all of them (a batch at a time, like the other bulk actions). The menu is
not there while you have no commands. Where a table is wider than its box (a table in a narrow board card, say), the
buttons on each row fold into that menu too (Done, Start or Stop, Edit and Delete, then the commands), so the column is one
small button and the table has the room; it opens out again when there is room for it. Names are one word that can't be taken for a filter (no spaces, no `+` `-` `/`, no
`:` or `=`, not a number).

![The Commands tab: fail, later and urgent, each with a name, what it stands for and an icon](../screenshots/settings-commands.png)

![The icon picker, searching for "cancel": the circled cross is among the results](../screenshots/icon-picker.png)

![The menu under the ellipsis on a task: fail, later and urgent, each with its icon and what it runs](../screenshots/task-commands-menu.png)

The tab shows the `settings.json` it will save. Other keys in the file are kept, and entries it can't use are listed and
dropped when you apply. The file is at most 64 KB.

## tasksh

![The tasksh tab: the reviewed attribute and the _reviewed report are not defined, the review filter and order, and the six lines Apply would write](../screenshots/settings-tasksh.png)

**Review** is tasksh's `review`: it walks the tasks you have not looked at lately, one at a time, and you mark each as
reviewed, skip it, edit it, change it, finish it or delete it. Start it with **Review** in the top bar (a card with
buttons) or by typing `review` in the Console. `review 10` stops after ten (that, a limited review, is done from the
Console), and `rev` is enough, as in tasksh. In tasksh the menu is keybindings, not a shell, and here too: a key is the
answer, with no Enter after it, and the same choices are buttons.

It works from two things in the taskrc, which the **tasksh** tab prepares: a `reviewed` attribute (a date UDA that
records when you last looked at a task) and a `_reviewed` report (one `uuid` column) that lists the tasks due for review.
If they are not there, the first review asks, in tasksh's words, whether to define them, then does so with the lines below. It says
whether each is defined, defined differently (with what is there now, since applying changes it), or missing, and lists
the exact `config` lines **Apply** would write, which are the ones tasksh writes the first time it runs. Nothing else
in the taskrc is touched. You can adjust the report's **filter** (by default never reviewed, or not for 6 days, and
still pending or waiting) and **order**; **Use tasksh's defaults** puts them back.

![A review in the Console: the task, its details, and the prompt asking what to do with it](../screenshots/review-console.png)

![The review card: the task's details and its choices](../screenshots/review-card.png)

Each task is shown with `[3 of 12]` and everything `info` shows, then the menu:

| Key | |
| --- | --- |
| Enter | Mark it reviewed (`modify reviewed:now`) |
| `s` | Skip it: on to the next, and it is not counted as reviewed |
| `e` | Edit it in the app's editor; closing the editor marks it reviewed and shows it again |
| `m` | Type what to change (`+tag -tag /teh/the/ project:X`), then Enter; Escape cancels. The same task is shown again |
| `c` / `d` | Complete or delete it; both count as reviewed |
| `q` | Stop. What is done stays done, and you can start again where the list left off |

At the end: `End of review. 3 out of 12 tasks reviewed.` In the Console the prompt is off while the review asks (it reads
`review>`), a bar of buttons stands over it, and Escape quits; the one thing you type is the change after `m`. **Open as
card** in that bar carries on in the card from the same place. In the card the **Reviewed** button has the focus, so Enter
marks it, as in tasksh, and the same letters work as keys.

Questions Taskwarrior asks in the middle of a step are asked here too, with Yes and No (or `y` and `n`): for a recurring
task, "This is a recurring task. Do you want to modify all pending recurrences of this same task?" Yes marks the whole
series, No only this one. (`rc.confirmation:no` does not silence that question, which `recurrence.confirmation` governs, so
tasksh shows it as well.)

Every step is an ordinary command (`task <uuid> modify reviewed:now`, `done`, `delete`) sent with
`rc.confirmation:no`, so hooks run, `undo` works, and the Tasks table follows. Review steps are not added to the command
history.

## Urgency

![The Urgency tab: the inherit switch, and each coefficient next to its default](../screenshots/settings-urgency.png)

A task's urgency is the sum of Taskwarrior's `urgency.*` terms, with its built-in coefficients unless your taskrc
changes them. Open **Settings**, then its **Urgency** tab, to see every coefficient next to its default, edit it, add your
own, or send one back to its default, then **Apply**. Edits are saved with your imported settings (as `urgency.*` lines in the taskrc tab). Your desktop
`~/.taskrc` is not touched, so copy any change you want there. It follows Taskwarrior's rules, so nothing is inherited
that Taskwarrior doesn't inherit:

- Coefficients add up: a task gets every matching coefficient, not just the most specific.
- `urgency.user.project.Home.coefficient` applies to `Home` and `Home.Kitchen`, but not to `Homework`.
- Tags match user and virtual tags (`urgency.user.tag.OVERDUE.coefficient`); keywords match the description,
  case-sensitively; `urgency.uda.<name>.coefficient` matches any value of a UDA, `….<value>.coefficient` one value.
- `urgency.inherit` is off, as in Taskwarrior. On, a task that blocks others takes the highest urgency of what it
  blocks, through the whole chain, plus 0.01.

`rc.urgency.due.coefficient:0 next` and `rc.urgency.inherit:1` also work for a single command.
