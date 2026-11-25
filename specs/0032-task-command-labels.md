# 0032: A label for each task command

- **Status:** done
- **Created:** 2026-11-26
- **Layer:** 2 (UI)
- **Front ends:** web, Android, terminal
- **Supersedes / related:** task commands (`taskCommands` in `settings.json`); board controls' `label` and
  `controlLabel` / `rangeLabel` (`web/src/lib/boards.ts`, `Boards.kt`), the same idea for a board's controls. Task
  `06efbdfc`.

## Why

A task command's name is a word typed at the console (`kanban_next`, `fail`), so menus show that word as it is. A menu
reads better with words a person would write (`Move to next column`, `Fail`), without making the name, which people
type and which kanban moves and chains refer to, any longer.

## What

- A task command in `settings.json` may have a `label`:
  `{ "name": "kanban_next", "command": "…", "label": "Move to next column" }`.
- Where a menu lists task commands it shows the label. Without one (or with an empty one) it shows the name as words: each
  run of `_`, `-` or `.` is a space and the first letter is upper case (`kanban_next` → `Kanban next`,
  `follow-up.later` → `Follow up later`), as a board control without a label is shown.
- The menus: the web app's task menu (`MoreMenu.svelte`) and the bulk bar's commands, the phone's task menu and bulk
  commands, and the terminal app's task commands box (`c`), which shows the label with the name beside it, the word that
  is typed to narrow the box.
- The name is still what is typed (the console, `:` lines, chains, kanban moves) and what completion offers.
- Settings → Task commands on the web and the phone has a Label field for each command, its placeholder the fallback. In
  the terminal app the Task commands form asks for it after the line; the Task commands section shows it.
- A label that is not a string is reported with the file's other problems, and the command is used without it.
- (Asked by uroybd while building.) A name can't be a command already: one `task` would run for that word (Taskwarrior's
  commands, hidden ones too, a unique abbreviation such as `sync`, a report), tasksh's `review` (from `rev`), or a
  console's `clear`/`cls`. The label may say the same. The kanban's own move command `review` becomes `inreview`,
  labelled "Review", and settings version 4 renames it in files that have it.

## Not doing

- Labels for taskrc aliases (they have an icon in `aliasIcons`, but no label); they keep their name, made words as above.
- Splitting `camelCase` names into words.
- A label for a kanban column or a board (they have names that are already labels).

## Design

The label is layer 2 only (a new optional key, no change of shape). Which words are commands is the engine's knowledge:
`tc_core::helpers::command_named(word, config)` asks the engine's own command matcher, with the taskrc's reports where a
front end has them (the settings panels on web and phone; the reader uses the default reports). The rename is
`tc_core::settings_version` 3 → 4. Each front end gets the same small function, ported with its tests as task commands are:

- **web:** `TaskCommand.label?: string` in `appsettings.ts` (read, checked: a string or absent; written back when set),
  `commandLabel(c)` beside it (label trimmed, else the name made words). `MoreMenu.svelte` and `BulkBar.svelte` show it;
  `CommandsPanel.svelte` gets a Label box.
- **Android:** `TaskCommand.label: String?` in `AppSettings.kt` (read, checked, written), `commandLabel` beside it; the
  task menu, bulk commands and `CommandsPanel.kt` as on the web.
- **terminal:** `TaskCommand.label: Option<String>` in `crates/tui/src/commands.rs` (serde, kept on save),
  `TaskCommand::label()`; the `c` box (`ui.rs` `commands`) shows `label  name`; Settings → Task commands shows the label.

## Acceptance criteria

- [x] A command with a label shows it in the menus on the web, the phone and the terminal; one without shows its name as
      words (`kanban_next` → `Kanban next`).
- [x] Typing still uses the name everywhere; the terminal's `c` box narrows by name and by label.
- [x] The label is kept when the file is saved again (web, phone, terminal), and a non-string label is reported.
- [x] Settings → Task commands on the web and the phone edits it (and the terminal's form).
- [x] A name that is a command already is refused on every front end, and old settings' `review` becomes `inreview`.
- [x] Tests: `appsettings.test.ts` (read, check, fallback), `AppSettingsTest.kt` (the same), the terminal's
      `commands.rs` tests (fallback, kept on save) and a test of the `c` box.
- [x] Docs: `docs/using/settings.md` (task commands), `docs/terminal.md`, `docs/using/kanban.md`.

## Open questions

- Should the fallback also split `camelCase` (`kanbanNext` → `Kanban next`)? Left out for now, as board controls don't.

## Implementation notes

- 2026-11-26: spec written; built on web (`appsettings.ts` `commandLabel`, `nameProblem(name, config)`, `MoreMenu`,
  `CommandsPanel` Label box), Android (`AppSettings.kt`, `TaskCard.kt` menu, `CommandsPanel.kt`) and terminal
  (`commands.rs` `label()`/`label_of`, `settings.rs` reader and `name_problem`, a `CommandLabel` prompt, the `c` box shows
  label, name, line). The name rule was added at uroybd's request: `helpers::command_named` over `cli::command_named`
  (the engine's `match_command`); settings version 4 (`review_command_v3_to_v4`). Tests pinned to version 3 moved to 4.
  Departure: the terminal edits the label in its form rather than only in the file.
