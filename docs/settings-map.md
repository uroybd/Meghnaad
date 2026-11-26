# The settings map

Every setting Meghnaad has, where it is shown, and how it is edited, for all three front ends at once. It is the
guardrail that keeps them alike: the web app, the Android app and the terminal app show the same pages, in the same
order, with the same parts on each, and the same kind of control for each setting. A front end that leaves something
out says so here, and why.

A new setting, a moved one or a new page is added here in the same change, before the front ends follow it.

## Controls

Each setting is one of these. The front ends draw them each in their own way, and always the same way.

| Control | Web | Android | Terminal |
| --- | --- | --- | --- |
| **switch**: on or off | a checkbox with its label | a checkbox row | `[x] Label` / `[ ] Label`; `Space` or `Enter` |
| **choice**: one of a list | a `select`, its choices under group headings | a dropdown menu | the value and `▾`; `Enter` opens a picker, each choice named with its group (`report  list`) |
| **input**: text or a number | an input with its label before it | an outlined text field with its label | `Label  [value]`; `Enter` edits it on the bottom line |
| **rows**: a list of records | a row each, its fields side by side; remove and move buttons; **Add a …** below | the same, stacked on a phone | a row each, its fields in columns; `a` adds, `e` changes, `D` removes, `K` `J` move |
| **form**: one row's fields together | inline in the row | inline in the row | a **modal** with every field, each on a shaded, underlined bar (`Ctrl+N` `Ctrl+P` or `↑` `↓` between them, `Tab` completes or goes on) |
| **picker**: a long list to pick from (icons, actions, holidays) | a drawer | a bottom sheet | a picker box |
| **action**: does something now | a button | a button | a key, named in the border |

Under each control, a line or two of what it does, faint; its problems under it, in the error colour.

**Applying.** On every front end, edits to a page wait until they are applied: **Apply** and **Revert edits** at the
foot of the page on the web and the phone, `Ctrl+S` and `u` in the terminal; a page with edits has a dot after its
name, and keeps its edits while another page is shown. A setting that is a single action (Turn Kanban off,
Sync now, Write them) happens at once.

Boards have no settings page: a board is its own entry in `boards`, made and changed on the Boards page, and there is
no setting for all boards.

## The map

Each line is a page, a part of it (a heading), or a setting: its name as shown, its control, then where it is kept.
`settings.json` keys are written from the root (`appearance.cardIcons`), a device's own from its file
(`web/devices/<id>.json`). *Not on* says which front end leaves it out and why.

- **Meghnaad**: the app itself
  - **General**
    - Home page: choice (next; Reports; Boards; Kanbans while Kanban is on; "not found" for one that is gone),
      `homepage`
    - `settings.json`: the whole file, with its problems as the linter finds them (`lintSettings`)
      - web, phone: a box to edit it in, Copy, Reset
      - terminal: where it is, its problems, `E` opens it in `$VISUAL` or `$EDITOR`
    - Updates: *phone only*, the app's own update row
  - **This device**: what this device keeps for itself (`web/devices/<id>.json`)
    - Device ID: input, with Change and Delete this device's settings. *Not on the terminal: its `settings.json` is the
      machine's own already.*
    - Home page: choice, as in General, with Use everyone's, `homepage`. *Not on the terminal: as above.*
    - Reminders: action, opens the reminder options (`reminders`; the terminal's are `tui.reminders`, `N`). *On the
      terminal it is the page's only part.*
    - AI agents: switch, `web.agents`. *Web only: WebMCP is a browser's.*
    - Keys: this device's keys, as in Keybindings. *The terminal's are its own already: Keybindings → Terminal.*
    - Unlock with a passkey, Lock now, Forget this device: actions. *Static web app only.*
  - **Appearance**
    - Theme: choice of `auto`, `light`, `dark`, `mono`, with the roles in their colours, `tui.theme`; Shading: switch,
      `tui.shading`. *Terminal only: the web and the phone follow the system's light or dark, and the taskrc's colours.*
    - Cards
      - Icons in cards: switch, `appearance.cardIcons`
      - Nerd Font: switch, `tui.nerdFont`; every icon the app draws comes from the terminal's Nerd Font (the card
        fields, the pages in the top bar and in the settings list). *Terminal only: the web and the phone draw Lucide
        icons.*
    - IDs
      - The short uuid when a task has no ID: switch, `appearance.idFallback`
    - Progress bars: rows, one per numeric UDA (switch, then min and max inputs), `appearance.progress`
    - Transformations: rows (match, replace, the fields it applies to as switches, condition; move up and down),
      `appearance.transformations`. *The terminal's form takes the fields as words (`description project`), and has no
      moving yet.*
  - **Keybindings**
    - Search keys: input
    - Show default keys: switch
    - General, Web, Mobile, Terminal: a part each (`keys`, `web.keys`, `mobile.keys`, `tui.keys`), rows of key and
      action, Take out; Add a key: picker of the actions, then the key
    - Commands: rows of the `:` words, what they take and do (`commands.json`)
  - **Kanban**: `kanban`
    - While it is off
      - UDA: input, `kanban.uda`
      - Automatic: Draw the kanbans on a kanban report: switch; Set up automatically: action
      - Manual: Turn on: action
    - While it is on
      - Every kanban: Report (choice), Filter (input), Completed since (input), Columns (rows: name, filter, on move;
        move left and right)
      - Each kanban: Name (input), Address (input, `/kanban/<id>`), Its own report, filter, Completed since, columns
        (a switch each, then the setting), Delete this kanban: action
      - Add a kanban: action
      - The UDA's lines and Write them: action
      - Turn Kanban off: action
  - **Commands**
    - Commands: rows (name, label, command, icon picker), `taskCommands`
    - Aliases: rows, one per taskrc alias (its name, an icon picker), `aliasIcons`. *The terminal lists them without
      the icon: it draws no alias icons.*
  - **Sync**
    - phone: Connection (S3, a TaskChampion sync server or a git branch: a form), Keep settings and hooks in a bucket
      (switch), Sync now and Stop syncing (actions)
    - terminal: when to sync: choice, `tui.sync` (`task sync` in the background)
    - *Not on the web: the Worker or the page syncs on its own; the static app's connection is set when it starts.*
- **Taskwarrior**: what `task` itself reads. *Not on the terminal: its backend is the real `task`, and the taskrc is
  edited as it always is.*
  - **taskrc**: the taskrc files, a workspace: a tree of files, an editor, + New (a file, a folder, uploads, Import from
    a link…, Add holidays…), Apply and Revert edits
  - **Urgency**: rows of coefficients (a value input, Back to default, remove), and a new one (name, kind choice,
    coefficient)
  - **Hooks**: the Lua script, an editor; Secrets: rows (name, hosts it may go to, value)
  - **tasksh**: what tasksh's review needs, its status, and setting it up
