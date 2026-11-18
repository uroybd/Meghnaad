# 0020: Keys run functions, with vim-like motions

- **Status:** in progress <!-- draft | ready | in progress | done | dropped -->
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

- [ ] `:tabn Tasks`, `:tabn 3`, `:tabnext`, `:5`, `:q`, `:done` on the command line run the function; `:` anywhere else,
      `: tabn` (a space), or `:` before an unknown word sends the line to `task` unchanged.
- [ ] The `:` key, from any view, opens the command line with `:` typed; `/` opens it empty.
- [ ] A key bound to an expression in `settings.json` runs it, in the contexts it is bound in; an action name bound as
      before still works.
- [ ] A key with two meanings in one context is refused when the keymap loads, and the Keybindings section says which.
- [ ] `5j` moves five rows; `gg` goes to the first; `g1` opens Tasks (`tabn 1`); `Esc` drops a pending count or key,
      which the status line shows while pending.
- [ ] `?` and the Keybindings section list the functions, their arguments and each binding's expression.
- [ ] Tests: parsing expressions (quotes, numbers, unknown names, vim's abbreviations), the `:` rule, counts and pending
      keys, refusing a double meaning, each function of the first set.
- [ ] Docs: `docs/terminal.md` (functions, `:`, counts), `docs/using/settings.md` (Keys: expressions).
- [ ] The web app: the task table has a chosen row that the motions move, the task commands act on it, and `:` opens
      the command line with `:` typed; the same `:` lines and bindings work as in the terminal app.
- [ ] The table of commands, abbreviations, parsing and the pending keys are `tc_core::keymap`'s, tested there once;
      each app tests only what its commands do.
- [ ] Sizes: the engine's WebAssembly measured before and after.

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

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.
