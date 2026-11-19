# 0025: Commands are what keys run; actions are presets of them

- **Status:** done <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-20
- **Layer:** 2 (UI), the shared keymap (`tc_core::keymap`, reached through `helpers`) and its assets
- **Front ends:** web, Android (both have a Keybindings tab), terminal (reads keys through the same engine code; it has
  no keys editor)
- **Supersedes / related:** 0020 (keys run functions, the `:` commands), 0023

## Why

A key runs an action (`tasks.done`) or, since 0020, a `:` command with its words (`filter +home`). Underneath, a command
is a name for an action. That is backwards from how people think of it: the command is the thing a key does, and a named
action is one ready-made command. The Keybindings tab offers only the actions, so a key that runs `filter +home` can be
written only in `settings.json`, and nothing checks it. A malformed one is skipped while reading, and only an unknown
name is reported; wrong words (`done 3`, `sort` alone) pass.

Only 31 of the 105 actions have a command today. An action with no command can't be shown as "the command it runs", and
can't be used as a template for a new one.

## What

- **Every action has a command** that runs it: a name following vim's conventions, or an existing command with a word.
  The table under Design lists them. `docs/commands.md` lists them all (made from `commands.json`), each with what it
  does and the words it takes.
- **The Keybindings tab, adding a key:**
  - The search list offers every preset (the actions, as now), and **Add a command** at the end.
  - Picking a preset fills an editable **Command** field with the command it runs (`tasks.done` → `done`). Picking Add a
    command leaves the field empty. Either way the form is the same: the command, then the key.
  - Under the field: the action it runs and what that does. Or, while it is malformed, why: an unknown name, a short
    form used past its length, words given to a command that takes none, words missing where they are needed (for
    example, `lock` takes only `pin` or `key`). **Add key** is off while the command is malformed, so a malformed
    command can't be saved from here.
  - A key is saved as the command (`"g h": "filter +home"`), edited or not.
- **Reading `settings.json`, on every front end:** a binding whose command is malformed is skipped (its key does
  nothing). It is shown as a **warning at the top of the Keybindings tab**, naming the key, the section and why, with
  a button to take it out. The phone shows the same warning in its Keybindings tab; the terminal shows it where it
  shows the other settings problems.
- **Action names still work** in `settings.json` (`"x": "tasks.done"`): each reads as its preset command, so no file
  needs migrating.

## Not doing

- No change to what any key does by default, and no change to the default keys asset beyond what the commands need.
- No keys editor in the terminal.
- No new behaviour behind a command: each new command runs an action that already exists.
- No `:` command for `app.commandLine`: it is the `:` itself (the one action left without one).

## Design

### Names (vim's conventions)

- Each command is one lowercase word, with the shortest abbreviation vim would allow, written as vim writes it
  (`tabn[ext]`).
- A family shares a stem, as `tabnext`, `tabfirst`, `bnext` and `bfirst` do.
- A command's options are its words, as `:sort r` takes one.
- Where an existing command with a word says it, that is the preset, rather than a new name.

Proposed:

| action(s) | command |
|---|---|
| `page.tasks` … `page.settings` | `tabnext Tasks` … `tabnext Settings` (existing `page.go`) |
| `app.prompt` | `prompt` |
| `app.sidebar` | `sidebar` |
| `app.back` | `back` |
| `app.stopTimer` | `stopall` (the started task, whichever it is; `stop` is the task under the cursor) |
| `app.reminders` | `reminders` |
| `app.add` | `add` with no words (the form; `add <task>` adds it, as now) |
| `app.filter` | `filter` with no words (the cursor in the filter; `filter <f>` sets it, as now) |
| `move.first` | `1` (existing `move.row`) |
| `move.pageDown`, `move.pageUp` | `pagedown`, `pageup` |
| `move.halfPageDown`, `move.halfPageUp` | `halfdown`, `halfup` |
| `move.sectionNext`, `move.sectionPrev` | `snext`, `sprevious` (as `bnext`/`bprevious`) |
| `sidebar.open` | `report` with no words (the report under the cursor) |
| `sidebar.hidden` | `hidden` |
| `info.details` … `info.raw` | `info details`, `info recurrence`, `info timelog`, `info annotations`, `info unknown`, `info raw` |
| `tasks.clearCompleted` | `delete completed` |
| `tasks.command` | `command` (no words: choose from a list, as vim's `:command` lists them; `command <name>` runs it) |
| `tasks.export`, `boards.export` | `export` |
| `tasks.markRange`, `tasks.markAll` | `mark range`, `mark all` |
| `sort.add`, `sort.remove`, `sort.reverse`, `sort.group` | `sort <field>+`, `sort <field>--`, `sort <field>=`, `sort <field>/` (the key under the cursor when the field is left out: `sort =`) |
| `sort.raise` … `sort.cancel` (the sort editor) | `sort raise`, `sort lower`, `sort reset`, `sort apply`, `sort cancel` |
| `lists.open`, `boards.list.open`, `settings.boards.open` | `open` (what is under the cursor; see Open questions) |
| `calendar.prev`/`next`, `boards.prev`/`next` | `previous`, `next` (see Open questions) |
| `burndown.daily`, `weekly`, `monthly` | `burndown day`, `burndown week`, `burndown month` |
| `boards.close` | `back` |
| `console.clear` | `clear` |
| `settings.editFile` | `edit settings` |
| `settings.general.home`, `settings.sync.change` | `home`, `syncwhen` |
| `settings.theme.preset`, `settings.theme.shading` | `theme`, `shading` |
| `settings.commands.add` / `edit` / `delete` | `command add` / `command edit` / `command delete` |
| `settings.keys.add` / `remove` / `none` / `defaults` / `search` | `key add` / `key remove` / `key none` / `key defaults` / `key search` |

### The engine (`tc_core::keymap`)

- `Command` gets `takes`: none, optional or required words, and, for a few, the words allowed (`lock`: `pin`, `key`;
  `info`: the tab names; `burndown`: `day`, `week`, `month`). A command may also name a different action for some of
  its words, or for none (`add` alone → `app.add`). The test that keeps `COMMANDS` and `commands.json` in step also
  checks `takes`.
- `call_of` stays the one reading of an expression. A new `check(expression) -> Result<Call, String>` gives the reason
  a command is malformed; `call_of` is `check(..).ok()`.
- `preset_of(action) -> Option<String>`: the command an action runs, for the panel to fill the field with.
- `resolve` and `bindings` use `check`: a malformed binding is skipped, and its key, section and reason go to `problems`
  as a warning. `bindings` stops listing it as a row.
- `with_binding` saves the command text as given; a binding whose value is an action name keeps reading as before.
- New helpers through `tc_core::helpers`: `key_command_check` (the field) and `key_preset` (the fill).

### The front ends

- **Web:** `KeysPanel.svelte`'s add form gets the Command field, filled from `key_preset` and checked with
  `key_command_check` as typed; Add a command goes at the end of the search list; the problems move to a warning at the
  top, each with Take out.
- **Android:** the same in `KeysPanel.kt`, through the same helpers over JNI. Its existing tests are ported.
- **Terminal:** nothing to edit; its keys come from `resolve`, so it skips the same bindings, and its settings problems
  line shows them.
- **Docs:** `commands.json` gains the new commands (`since: "unreleased"`), and `docs/commands.md` is made from it.
  `docs/using/settings.md` (Keybindings) and the 0020 notes are updated.

## Acceptance criteria

- [x] Every action in the keymap asset but `app.commandLine` has a command (a test walks the asset).
- [x] In the web and Android Keybindings tabs:
  - picking a preset fills the Command field with its command;
  - Add a command opens the same form empty;
  - a malformed command shows why, and Add key stays off;
  - a key is saved as the command.
- [x] A malformed binding in `settings.json` is skipped by every front end and shown as a warning at the top of the
  Keybindings tab (web, Android) and in the terminal's settings problems, with a way to take it out.
- [x] `settings.json` files with action names keep working unchanged (tests with the current shape).
- [x] Tests: engine (`check`, `preset_of`, `resolve`/`bindings` skipping and warning), web panel, Kotlin panel, TUI.
- [x] Docs: `docs/commands.md` regenerated; `docs/using/settings.md`; spec 0020's notes point here.
- [x] Size: the engine wasm measured before and after.

## Open questions

Decided 2026-11-20, with the person:

- Names follow vim's conventions.
- A key is saved as its command, even when a preset is picked unedited; action names in old files still work.
- Contextual names are contextual: `open`, `next`, `previous`, `export` and `back` run the action of the context the key
  is pressed in (the next month on the calendar, the next board on a board).
- `sort` takes a field with a suffix: `due+` ascending, `due-` descending, `due--` takes it out, `due=` turns its
  direction round. A bare word (`raise`, `lower`, `reset`, `apply`, `cancel`, `replace`) is the sort editor's, acting on
  the key under the cursor; a field always has a suffix, so the two never clash.
- `sort` words always edit the sort in place, in order (`sort due- project--`), a comma list included; replacing the
  whole sort takes an explicit word: `sort replace due+,urgency-`. (`:sort due+` used to replace; it now edits.)
- `due/` groups by the key (the `/` task writes in a sort), a fifth suffix in place of the editor's `group`.

Still open:

- **The table above:** any name you'd rather have; it can change until the commands ship.

## Implementation notes

2026-11-20:

- Engine (`tc_core::keymap`): `Command` is `bare` (the actions it runs alone, several for one of several places),
  `subs` (a word with an action of its own: `info details`, `sort raise`, `sort =`) and `words` (`Words::None`, `Any`,
  `OneOf`, `Page`, `Sort`). 54 commands. `check(expression, groups, known)` gives the call or why it is malformed;
  `call_of` is `check(..).ok()`; `preset_of(action)`; `row_name` (`done` and `tasks.done` are one row); `command_line_in`
  with a context's groups (`Keymap.groups`). `resolve` skips a malformed binding into `Keymap.bad` (and a line in
  `problems`); `bindings` lists `bad` for the warning and leaves those lines out of `problems`; each `Binding` has its
  `command`. `with_binding` takes the command as given and an optional group; `without_key` takes one key out;
  `check_command` (helper `key_command_check`) answers the panel with the call, what it does and each place
  (`{group, action}`). Helpers `key_preset`, `keymap_unset`; `keymap_set` takes a sixth argument, the group.
- `:sort` words: `tc_core::sortspec::edit` and `edit_words` (`+ - -- = /`, `replace`).
- `:1` is `move.first`, so a count before a key bound to it is still a row (`5gg`).
- A context keeps a binding as its action where it is one alone (`sort raise` → `sort.raise`, `next` bound for boards →
  `boards.next`), so screens matching on an action name find it; one with words is kept as written.
- Terminal: `:sort` words edit the sort; `:` commands get the context's groups. Departure: the spec said the terminal
  has no keys editor, but it has one (Settings → Keybindings); it now saves the command of the action picked, as the
  others do. It has no command field.
- Web: `KeysPanel.svelte` has the Command field, filled from the preset, checked as typed, **Where** for a command of
  several places, **Add a command** at the end of the search, the warning with **Take out** at the top; rows show the
  command, then the action. `ConsoleInput` passes the context's groups.
- Sizes (gzip -9): engine 896,526 (0.8.20's release, built in CI) → 907,000 bytes (this branch, built here), about
  +10 KB. The Worker links none of the keymap; its 1.1 KB difference is CI against a local build.
- Shipped in 0.8.22 for the engine, the terminal and the web.
- Android (after 0.8.22): `Keymap.kt` has `Binding.command`, `BadKey`, `withBinding(.., group)`, `withoutKey`,
  `checkCommand` (`CheckedCommand`, `KeyPlace`); `KeysPanel.kt` has the Command field, the check, **Where** as buttons
  (the app has no dropdown of its own), **Add a command**, and the warning card with **Take out**. Its tests call those
  functions rather than `KeysState`, which needs a whole `Store`.
