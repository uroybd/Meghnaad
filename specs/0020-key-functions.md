# 0020: Keys run functions, with vim-like motions

- **Status:** done <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-19
- **Layer:** 2 (UI)
- **Front ends:** terminal and web, together. The phone is left out: it has no keys (its `mobile` keymap is empty).
- **Supersedes / related:** the keymap (`tc_core::keymap`, `web/public/keymaps/default.json`), the terminal app's hints
  (v0.8.9: a pane's own keys in its border)

## Why

A key today runs one fixed action (`tasks.done`, `page.tasks`), so what a key can do is only what someone thought to
name, and nothing takes an argument: there is a key for each page rather than one way to go to a page. Someone who works
from the keyboard, as in vim, wants to say what a key does, to repeat it (`5j`), and to run the same thing from the
command line without a key at all.

## What

1. **Functions.** The app has a fixed set of functions, each with a name and arguments, written as a call:
   `tabn 3`, `tabn Tasks`, `5`, `+3`, `done`, `filter +home`. Each is listed (`?` and the Keybindings
   settings section) with what it takes and what it does. Today's actions are functions with no arguments, so every
   binding written so far keeps working.
2. **Binding a key to an expression.** In `settings.json` (and the Keybindings section), a key in a context is bound to
   an expression, a function and its arguments: `"tasks": { "g1": "tabn 1", "gh": "filter +home" }`.
3. **Focus decides the keys.** Which bindings apply is the context of the pane the keys go to (as today: the sidebar,
   the table, a Settings section, the info view). A context's own key is not also a global key: the keymap refuses a
   binding that would give one key two meanings in a context, and says which.
4. **Running a function from the command line.** A line that starts with `:` joined to a function name runs it:
   `:tabn Tasks`, `:tabn 3`, `:filter +home`, as vim's command line does. Only there: a `:` anywhere else in a
   line, one followed by a space, or one before anything but a function name is left alone, and the line goes to
   `task` as it does now. The `:` key, from any view, opens the command line with `:` already typed; `/` opens it empty,
   for `task`.
5. **Vim-like motions.** A count before a motion repeats it (`5j`, `3]`); keys of more than one press (`gg`, `g1`) wait
   for the next press, showing what is pending in the status line; `Esc` drops a pending count or key.

## Not doing

- User-defined commands or scripts: the commands are the app's own, a fixed API.
- Several commands in one binding (`filter +home; sort due+`).
- Marks (`ma`, `'a`) and repeating the last change (`.`): later.
- vim-taskwarrior's column cursor (`Tab` between columns, a field's `m`, `s` by the column, `<`/`>`, `H`/`L`), its
  bookmarks (`B`) and history (`J`/`K`): later.
- The phone: it has no keys.
- Changing what `task` runs: a function runs the app's actions, which send `task` command lines as they do now.
- Modes (insert, visual) beyond what the app has: marking with `V` stays as it is.

## Design

Agreed 2026-11-19 (the decisions are under Open questions).

- **Shared, in `tc_core::keymap`** (reached through `tc_core::helpers`, as the keys are; pure functions): the table of
  commands (name, vim's shortest abbreviation, the arguments each takes, the contexts it works in), resolving an
  abbreviation, parsing a `:` line or a binding's expression into a command and its arguments, and the keys pressed so
  far: given what is pending and the next key, whether it is a count, waits for another key, or runs a command (with
  its count). What each command does stays in each app.
- **The web app** gains a chosen row on the task table (moved with the keys, drawn as the terminal app's), the task
  commands on it (`done`, `start`, `info`, ...), and the `:` key, which opens the command line `/` opens, with `:`
  typed.

- **The expression** is parsed in `tc_core::keymap` (shared, as the keys are), reached through `tc_core::helpers`: a
  function name (a short lowercase word), then its arguments, split as the engine splits a command line (a word needs
  quotes only when it has a space: `tabn Tasks`, `filter 'project:big house'`). Numbers and words only. A binding's
  value is either today's action name (`tasks.done`) or such an expression (`done`, `tabn 3`); an action name keeps its
  meaning, so no binding written so far changes.
- **The functions** are each front end's own, as actions are today: a table in the terminal app (beside `keys::WORDS`)
  of name, shortest abbreviation, the contexts it works in, its arguments (a number, a word, one of a list) and what it
  does. A function that does what an action did runs that action's code, so nothing is done twice. Names follow vim
  where vim has the command, abbreviated as vim allows (`tabn[ext]`: any of `tabn` to `tabnext`):

  | Does | Function |
  | --- | --- |
  | next, previous page | `tabn[ext]`, `tabp[revious]` / `tabN[ext]` |
  | page by number or name | `tabn 3`, `tabn Tasks` (a name is ours; vim takes a number) |
  | first, last page | `tabfir[st]`, `tabl[ast]` |
  | row by number, first, last | `5`, `1`, `$` |
  | rows down, up | `+3`, `-3` |
  | quit | `q[uit]` |
  | undo | `u[ndo]` |
  | sync | `w[rite]` |
  | reload | `e[dit]` |
  | filter, sort | `filt[er] +home`, `sor[t] due+` |
  | help | `h[elp]` |
  | delete the task (`task` confirms as it does) | `d[elete]` |
  | mark | `ma[rk]` |
  | what vim has none of | `done`, `start`, `stop`, `modify`, `annotate`, `info`, `add`, `report`, `context` |

  A function that works on a task works on the chosen row (or the marked ones, as the keys do); one that does not apply
  where it is run says so on the status line.
- **Focus.** The context of the pane the keys go to decides the bindings, as now. Loading the keymap refuses a key that
  would mean two things in one context (a view's key the same as a global one there), and the Keybindings section says
  which.
- **`:`**: a global key (`app.function`, default `:`) opens the command line with `:` typed; `/` (`app.prompt`) opens it
  empty. On Enter, a line whose first word is `:` joined to a known function's name runs the function; any other line
  goes to `task` unchanged. `Tab` completes a function's name and its listed arguments.
- **Counts and longer keys** (the terminal app's key handling): a digit not bound in the context starts a count, which
  repeats a motion (`5j`, `3]`) and is given to functions that take one; a press that begins a longer binding (`g` of
  `gg`, `g1`) waits for the next; the status line shows what is pending; `Esc` drops it.
- **Defaults**: `default.json` keeps today's keys, adds `gg` (`1`, the first row) and `g1`..`g9` (`tabn 1`..`tabn 9`),
  and writes as expressions the bindings that take arguments.
- **Help**: `?` and the Keybindings section list the functions with their arguments, and each binding's expression.
- **No change to `tc-core`'s engine**; the keymap's parsing grows a little (measured in the engine's WebAssembly, which
  carries the helpers).

## Acceptance criteria

- [x] `:tabn Tasks`, `:tabn 3`, `:tabnext`, `:5`, `:q`, `:done` on the command line run the function; `:` anywhere else,
      `: tabn` (a space), or `:` before an unknown word sends the line to `task` unchanged.
- [x] The `:` key, from any view, opens the command line with `:` typed; `/` opens it empty.
- [x] A key bound to an expression in `settings.json` runs it, in the contexts it is bound in; an action name bound as
      before still works.
- [x] A key with two meanings in one context is refused when the keymap loads, and the Keybindings section says which.
- [x] `5j` moves five rows; `gg` goes to the first; `g1` opens Tasks (`tabn 1`); `Esc` drops a pending count or key,
      which the status line shows while pending.
- [x] `?` and the Keybindings section list the functions, their arguments and each binding's expression.
- [x] Tests: parsing expressions (quotes, numbers, unknown names, vim's abbreviations), the `:` rule, counts and pending
      keys, refusing a double meaning, each function of the first set.
- [x] Docs: `docs/terminal.md` (functions, `:`, counts), `docs/using/settings.md` (Keys: expressions).
- [x] The web app: the task table has a chosen row that the motions move, the task commands act on it, and `:` opens
      the command line with `:` typed; the same `:` lines and bindings work as in the terminal app.
- [x] The table of commands, abbreviations, parsing and the pending keys are `tc_core::keymap`'s, tested there once;
      each app tests only what its commands do.
- [x] Sizes: the engine's WebAssembly measured before and after.

## Open questions

Decided 2026-11-19:

- `:` runs a command only as the first character of the line, attached to its name (`:tabn Tasks`); in no other place
  or way. (`!` was first, then `:` instead, since `task` reads a leading `!` as "not".)
- `:` is a global key: from any view it opens the command line with `:` already typed, as in vim; `/` stays the command
  line for `task`.
- Command names are vim's own where vim has the command (`:tabn`, `:q`, `:u`, `:w`, `:5`), with vim's abbreviations;
  short lowercase words for what vim has none of (`:done`, `:start`).
- Motions: counts and longer keys (`gg`, `g1`) now; marks (`ma`, `'a`) and repeating the last change (`.`) later.
- One command per binding: several in one (`filter +home; sort due+`) is not in this version.
- The terminal and the web app in one go, with what can be shared in `tc_core::keymap`.

- **Default keys.** Decided 2026-11-19: follow vim-taskwarrior's defaults (a task list, not an editor, so vim's
  text-editing meanings don't apply; its keys are what vim users of Taskwarrior know). Checked against taskwarrior-tui,
  VIT and taskwiki too. Proposed changes to today's keys: sort `o` → `s`; start and stop become one key, `+`, which
  toggles the chosen task (starts it, or stops it when it is started); on marked tasks there is no toggle: starting them
  (`:start`, the bulk action) passes over the ones already started, and stopping them (`:stop`) the ones not started;
  modify stays `m` until there is a column to modify (`M`, the whole task, is added); the pages `1`..`9` → `g1`..`g9`
  (digits become counts); first row `g` → `gg`. Kept, as vim-taskwarrior has them: `a`, `A`, `d`, `D`, `f`, `r`, `R`,
  `c`, Space, `u`, Enter, `S`, `q`. Also decided: `p` duplicates the task, `X` clears the completed ones (`task`
  confirms as it does), `x` removes an annotation and `o` opens one (a link in it); its column cursor (`Tab`/`S-Tab`,
  `m` a field, `s` by the column, `<`/`>`, `H`/`L`) is later, so `m` and `M` both modify the task and `s` asks for the
  sort; bookmarks (`B`) and history (`J`/`K`) are later.

## Implementation notes

2026-11-20: spec 0025 makes commands what keys run: every action has one, a key is saved as its command, and a
malformed one is skipped and warned of. See [0025](0025-commands-are-the-base.md).

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.

- 2026-11-19: Built, terminal and web together.
  - **Shared** (`crates/tc-core/src/keymap.rs`): key sequences (`g g`, written `gg` or `g1` for two characters),
    `COMMANDS` (vim's names and shortest forms, then ours), `call_of` (an expression: a command and its words, a row
    `5` `$` `+3` `-3`, or an action's own name), `command_line` (the `:` rule), `Pending` and `step` (counts, keys of
    two presses, Esc), and in `resolve`: expressions bound as actions are, in the group of the action they run;
    a key that is the start of a longer one, and a key two user bindings of different groups share in a context, are
    said. Helpers `key_step` (a context's keys, not the whole keymap: 1 KB less), `key_command_line`, `key_call`,
    `key_commands` (since removed, below). Tests in `keymap.rs` and `helpers.rs`.
  - **Defaults** (`web/public/keymaps/default.json`, copied to the phone): as decided. Terminal: pages `g1`..`g9`,
    `gt` `gT`, `gg`, `:` is `app.commandLine` and `/` alone is `app.prompt`, sort `s`, `+` toggles, start and stop
    without keys, `m` `M` modify, `x` denotate, `o` open link, `p` duplicate, `X` clear completed. New actions
    `page.next/prev/first/last/go`, `move.row`, `tasks.toggle/denotate/open/duplicate/clearCompleted`,
    `app.commandLine`. Web: the same pages, `j` `k` `gg` `G`, and the task keys on Tasks (`move` added to its context).
  - **Terminal** (`crates/tui`): `app/calls.rs` does what a call asks; `normal()` steps through pending keys and shows
    them on the status line; `:` lines run in `console()`; `Tab` completes a command's name; `?` lists the commands
    and the user's command keys. Tests in `app/tests.rs`, `tests/bulk.rs`, `tests/reminder.rs` (keys updated).
  - **Web**: `lib/keyCalls.ts` (moved `runKey` out of `App.svelte`); the chosen row is the focused `tr[data-uuid]`;
    `App.svelte` steps keys and shows what is pending at the bottom right; `ConsoleInput` runs `:` lines; `KeysHelp`
    lists the commands. Tests in `keyCalls.test.ts`.
  - **2026-11-19, What 2's shape**: the first build put the expression where an action's name goes
    (`"filter +home": "g h"`); reworked to What 2: each section of keys is groups, each a key → what it runs (an
    action or a command), `null` unbinding (`"tasks": { "g h": "filter +home", "d": null }`). A bound key adds to the
    action's defaults; a key bound in a group still takes the default of a group that comes before it where both act.
  - **Versioned settings**: `settings.json` now says its `"version"` (none = 1). `tc_core::settings_version::migrate`
    runs each migration in turn (1 → 2: `keymap::keys_v1_to_v2`, the old action → keys sections to group → key →
    action, with the default keys they replaced, the user's shared ones included for a front end's own, unbound). The
    terminal app (`settings::migrated`, on load), the web store and the phone's `loadAppSettings` run it through the
    `settings_migrate` helper and save the result; `resolve`, `bindings` and `with_binding` migrate what they are given
    too, so an unsaved old file still reads right. The web skips it until the default keys are fetched.
  - **2026-11-19, the commands described once**: the user asked for the commands' text out of the engine, in one JSON
    that feeds every front end and a generated docs page with an anchor per command, and a `since` to leave out what a
    build does not have. `web/public/keymaps/commands.json` (summary, takes, doc, since; copied to the phone by
    `gen-lucide.mjs`), `docs/commands.md` from `scripts/commands-doc.mjs` (a test catches drift), linked from `?` and the
    Keybindings sections (web, terminal, phone, each listing the commands now). `COMMANDS` lost `takes` and the
    `key_commands` helper went; a test keeps names and short forms in step. `since` is `unreleased` until the bump
    (`version.mjs sync` stamps it, `just release` commits it, the Release workflow refuses a tag with one left).
  - **Sizes**: engine 880,556 → 887,662 bytes gzip (+7.1 KB): the command table, the step and the checks; the
    group shape and `settings_version` brought it to 893,044; moving the commands' text out, 892,884 (+12.3 KB in all).
  - **Left**: the Keybindings section lists each binding's expression but not the commands (the help `?` does); in the
    terminal's info view and menus, `gg` is not read (they look up one key; Home works). The web's `G` reaches the
    last row drawn (a long report draws more as it scrolls). Checked by tests, not yet by a person in a browser or a
    terminal.

