# Kanban

A **kanban** shows your tasks as columns, left to right. Each column has a **filter** that picks its tasks and an **On
move** command line, which runs on a task when you move its card into that column. Taskwarrior has no kanban of its
own: this one is made of filters and command lines, so `task` decides which task goes in which column, and a move is a
`modify` or a `done` like any other.

A kanban is drawn from a **report**. It uses that report's filter, its order and its columns, and a card shows the
report's columns: the description first, then the rest. It adds a **filter** of its own, and **Completed since** to
leave out tasks finished long ago.

## Setting it up

Kanban is off until you set it up in **Settings → Kanban**. There are two ways:

- **Automatic** defines a string UDA (`progressState` unless you name another) with the values `inprogress` and
  `review`, using the same `config` lines the tasksh tab uses. It also offers, ticked to begin with, to add a report
  named `_kanban` to your taskrc and draw the kanbans on it (the `_` keeps it out of the report lists, as `task`'s own
  helper names are):

  ```ini
  report._kanban.description=Open tasks and the ones finished lately, for Kanban
  report._kanban.columns=id,priority,project,tags,due.relative,scheduled.countdown,description
  report._kanban.labels=ID,P,Project,Tags,Due,Sch,Description
  report._kanban.filter=( +PENDING or +COMPLETED )
  report._kanban.sort=urgency-
  ```

  Its filter leaves out deleted and waiting tasks and recurring templates, so Kanban's own filter is left empty. If
  your taskrc already has a `_kanban` report, it is used as it is and nothing is added. A setup made before 0.8.54
  named it `kanban`, and keeps drawing on that one. Untick it and Kanban is drawn
  on `all` instead, with the filter `( +PENDING or +COMPLETED )` (which `all` needs, having no filter of its own). It
  shows every line first and writes nothing else to your taskrc. It sets Completed since to `today-14d`, and these
  columns:

  | Column | Filter | On move |
  | --- | --- | --- |
  | To do | `status:pending progressState:` | `pending` |
  | In progress | `status:pending progressState:inprogress` | `inprogress` |
  | Review | `status:pending progressState:review` | `inreview` |
  | Done | `status:completed` | `done` |

  `pending`, `inprogress` and `inreview` are [task commands](settings.md#commands) it adds to Commands, each with a
  label and an icon, so a move is the same word everywhere the app runs one on a task: a task's ⋯ menu, a report, the
  tasks you have selected, and the console (`3 inreview`):

  | Command | Label | Stands for | Icon |
  | --- | --- | --- | --- |
  | `pending` | To do | `modify status:pending progressState:` | circle-dashed |
  | `inprogress` | In progress | `modify status:pending progressState:inprogress` | loader-circle |
  | `inreview` | Review | `modify status:pending progressState:review` | eye |

  Not `review`, which is tasksh's command: a command can't be named like one. Settings made before 0.8.62 had it as
  `review`; the app renames it `inreview` once, with the move lines that ran it.

  They are this app's, kept in `settings.json`, so the `task` CLI on the desktop does not know them. (A taskrc alias
  would be, but `task` writes an alias out wherever its word stands on the line, so `task add review the PR` would
  move the new task to Review.) A command you already have of the same name is left as it is; if it stands for
  something else, that column moves with the whole command line instead. Done moves with `done`: a hook can tidy up
  `progressState` on a finished task if you want it cleared.

  It then makes a first kanban, called Kanban, with nothing of its own. A completed task is in Done whatever its
  `progressState` is. Moving a card out of Done reopens the task, because each move sets `status:pending`. No column
  starts a task: started is for time tracking, not a stage. If tasksh's review is set up (the `reviewed` UDA is
  defined), moving a card to Review also clears `reviewed`, which puts the task back in the review queue.
- **Manual** changes nothing in your taskrc. You write the columns yourself, using tags (`+next`), a UDA of your own,
  `+ACTIVE`, or anything else a filter can say.

## Kanban's settings, and a kanban's own

The **UDA**, the **report**, the **filter**, **Completed since** and the **columns** belong to Kanban as a whole, and
every kanban uses them. A kanban has a **name** and an **address**. It can also have its own report, filter,
Completed since or columns in place of Kanban's: tick **Its own …** (on the phone too), or, in the terminal app, press
Enter on that row. Take it away again and the kanban goes back to Kanban's. The UDA is Kanban's alone.

A column's **On move** is any command line you could type after a filter. A chain works (`modify +doing ; annotate
started`), and so does one of your [task commands](settings.md#commands). It runs on the moved task only, with the
task's uuid in front of each part. A column with an empty On move takes no cards. The columns' filters and moves name
the UDA themselves, so if you rename the UDA, change them to match.

Everything is kept in the app's own `settings.json`, under `kanban`:

```json
"kanban": {
  "enabled": true, "setup": "automatic", "uda": "progressState",
  "report": "kanban", "filter": "", "completedSince": "today-14d",
  "columns": [ { "name": "To do", "filter": "status:pending progressState:",
                 "move": "modify status:pending progressState:" } ],
  "kanbans": [ { "id": "kanban", "name": "Kanban" },
               { "id": "work", "name": "Work", "filter": "project:Work" } ]
}
```

**Turn Kanban off** hides the page. The kanbans stay in `settings.json`, and the UDA (and the `_kanban` report) stay in
your taskrc.

## The page

**Kanban** lists your kanbans, and opens one at `/kanban/<address>`. In the top bar, Kanban (like Boards) opens a
menu of each one and the list; on a phone, the menu lists them under it. Each column is the report run over the
kanban's filter, Completed since and the column's filter:

```text
( <filter> ) ( -COMPLETED or end.after:<completed since> ) ( <column's filter> ) <report>
```

Each part is in brackets, so an `or` in one stays in it. A task goes in the **first** column whose filter matches it,
in the report's order. **Other** is last: the tasks the report and the kanban's filter pick that no column has. It is
folded until you open it, and takes no cards.

Above the columns, the **report**, the **filter** and **Completed since** start as the kanban's, and you can change
them on the page. The kanban stays as saved. On a phone, and in the Android app, they fold under the filter button
beside the kanban's name, which has a dot while they differ from the kanban's own. The gear beside it edits the kanban
in Settings → Kanban. A report you pick is part of the address (`/kanban/work/list`).

The columns share the page's width, each between a comfortable least and most, and take the height left on the
screen: a long column scrolls its own cards. A card is drawn as the report's table draws the task's row: the colour
your taskrc gives the row (`color.overdue`, `color.active`, `color.tag.x`, …, in the order `rule.precedence.color`
says, as `task` itself colours it) is a faint wash behind the card (in the terminal app, its frame), the priority is a
ribbon down its left edge in the colour the taskrc gives that priority (`color.uda.priority.H`, the colour of the
priority's word in a table), tags are chips, a UDA's choice is a pill in its value's colour (the value half of its
badge, with icons on), and a finished task is tinted. A ribbon is not Taskwarrior's: it is only
ever the priority, on every front end, and the cards of a task list on a phone have it too. In the Android app a card is the one the report's list
shows, with its Done button and ⋯ menu, and **Move to…** under its fields. With the phone held upright, a column takes
the screen's width, and a swipe goes to the next.

Move a card by dragging it to another column. On a phone or a touch screen, where dragging is hard, a card has a
**Move to…** menu instead; the Android app has it everywhere. In the terminal app, `H` and `L` move the card to the
column on either side, and `m` picks a column. A move changes that task only: for a recurring task, the instance you
moved, without asking about the rest of its series (`rc.recurrence.confirmation:no` goes in front of the On move).
After a move the columns are read again. If the task didn't land in the column you moved it to (the On move doesn't
make it match that column's filter, or `task` refused it), you are told where it is.

### Icons on the cards

A card shows the report's columns under the task's description, each with its label from the report (`Due 2d`). Tick
**Icons in cards** under Settings → Appearance (see [Cards](settings.md#cards)) and every card shows Taskwarrior's own
fields by an icon instead: `#` for the ID, a flag for the priority, a folder for the project, a tag for the tags, a clock
for `due`, and so on. The icon goes by the field itself, whatever the report's label or format (`due.relative` is still
a clock). In the terminal app the icons are symbols (`⚑ H`, `◷ 2d`), turned on by Icons in cards in Settings →
Appearance; with Nerd Font on as well, they are your Nerd Font's glyphs of the web's icons.

On every front end a card draws its description and fields as the report's table does, with your
[transformations](settings.md#transformations) applied: a ticket key a rule turns into a link is a link on the card.

On the web and the phone, with Icons in cards on, a UDA is a pill, its label then its value; off, it is its label and
value on one line, as Taskwarrior's fields are. A UDA drawn as a progress bar is always the bar after its label. A string
UDA with no `values`, free to hold anything, is a pill or on one line too while it holds a few words (one line of up to
30 characters, counted as shown: a ticket key a transformation made a link counts as the key); longer, on several lines
or Markdown of more than one line (a checklist), it is a paragraph of its own under the other fields,
headed by its name in bold and drawn as Markdown, and a checkbox in it (`- [ ] call back`) can be ticked right on the
card, which saves the task's text with it ticked.

### Swimlanes

A report whose sort marks a group with `/`, as Taskwarrior's own does (`report._kanban.sort=project+/,urgency-`), gives
the kanban swimlanes: a lane for each group, headed as the table heads it (`Project: Home`, `Project: (none)`), in the
report's order. The column names are drawn once at the top, and each lane is a row of the same columns, as tall as its
longest. The board then scrolls as a whole rather than column by column. A lane folds with the arrow by its heading.

A card moves only within its lane: a move runs the column's On move, which doesn't change the task's group. To move a
task to another lane, edit the task (`modify project:Work`). The terminal app draws them the same way: the column
names on one line at the top, then for each lane its heading across the board and a band of the columns; `z` folds
the lane of the card under the cursor (the cursor then rests on its heading), and `[` `]` go to the lane before and
the next.

## In the terminal app

Kanban is a page in the top bar after Boards while it is on. It has no number: `gk` opens it, and `gt` and `gT` go
through it. `:kanban` opens it too.

| Key | What it does |
| --- | --- |
| Enter, `l` | open the kanban (in the list) |
| `b`, Backspace | back to the list |
| `h` `l` | the column before, the column after (in the same lane, where it has cards there) |
| `j` `k` | the card below, the card above (a folded lane is one stop, its heading) |
| `[` `]` | the first card of the lane before, of the next (with [swimlanes](#swimlanes)) |
| `z` | fold or unfold the card's lane (`:kanban fold`) |
| `H` `L` | move the card to the column before or after (`:move left`, `:move right`) |
| `m` | move the card to a column chosen from a list (`:move`) |
| Enter | the card's task info |
| `f` `r` `s` | change the filter, the report or Completed since here, without saving them |
| `o` | show or hide Other |

**Settings → Kanban** is a page of its own, with Kanban's settings and each kanban's, one row each, and waits for
`Ctrl+S` like every page of Settings (an action, such as setting it up or turning it off, happens at once). A column is
added or changed in a box with its name, filter and On move together. While Kanban is off, its rows are the
UDA, the report (Enter switches between adding a `_kanban` report and using `all`) and the two ways to set it up:
Enter on **Set up automatically** shows the lines it will write, and Enter again writes them. Once it is on, `e` or Enter changes the row under the cursor, `a` adds a column, `D` deletes a column or a
kanban (or, on a kanban's own setting, goes back to Kanban's), `K` and `J` move a column, `n` adds a kanban, `o` opens
one, `w` writes the UDA if your taskrc doesn't have it as needed, and `x` turns Kanban off.
