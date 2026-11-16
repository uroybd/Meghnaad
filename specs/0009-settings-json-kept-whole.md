# 0009: settings.json, editable, pastable and kept whole

- **Status:** done <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-17
- **Layer:** 2 (UI)
- **Front ends:** all (web, Android, terminal)
- **Supersedes / related:** [Settings](../docs/using/settings.md); 0010 (the ID fallback setting, which lives in this
  file)

## Why

`settings.json` is one file for every front end: the web app and the phone share it through the bucket, and the
terminal app keeps a copy that people paste in. Each front end reads only part of it (the terminal has no use for
sync options, the phone ignores `tui`), and today some of them rebuild what they write from what they understood. A key
one front end doesn't know, inside `appearance`, a board or a task command, is lost the next time another front end
saves, and a task command that can't be used is dropped on Apply. The file should survive every front end, and you
should be able to edit or paste it whole where you can see it.

## What

1. **Edit and paste it.** On every front end `settings.json` is under General. On the web and the phone it is the
   whole file in a text box, with **Copy**, a paste replacing it, and **Apply**; the terminal app opens it in your
   editor (`E` on its Settings page) and reads it again when it changes. The file is linted, as a linter would: an
   error (not a JSON object, over 64 KB, or a known key in the wrong shape, such as `appearance` that is not an object
   or a board without `sections`) stops Apply, and each is listed with where it is; a warning (an entry this front end
   can't use, such as a task command with a bad name) is listed and the file is saved with it. The terminal app lists
   the same in General after an edit.
2. **Nothing is lost.** A value a front end doesn't use (another front end's section, a key it doesn't know at any
   depth, an entry it can't read) is left as it is by every operation on every front end: saving a board, a task
   command, a key, the appearance, the home page, the terminal's own settings. A front end changes only what it
   edited.
3. **Unusable entries are kept.** An entry a front end can't use (a task command with a bad name, a malformed board)
   is listed, as now, and stays in the file until you remove it yourself.

## Not doing

- Merging edits made on two devices at the same time: the last save wins, as now.
- A schema or validation beyond "a JSON object of at most 64 KB" for the pasted file.
- Syncing the terminal app's copy with the bucket (it never reaches the bucket).

## Design

Agreed 2026-11-17.

- **Saving changes only what was edited.** Each front end writes through one function, `overlay(old, new)`, ported
  the same way to each (TypeScript in `appsettings.ts`, Kotlin in `AppSettings.kt`, Rust in the terminal's
  `settings.rs`), all tested against one fixture file at the root, `fixtures/settings-kept.json`, which the three
  test suites read by path:
  - Objects: `new`'s keys win; a key only in `old` is kept (one this front end doesn't know); a key `new` sets to
    `null` is removed (one it knows and you cleared). The serialisers write `null` for a known field you cleared rather
    than leaving it out.
  - Lists of objects (boards, sections, task commands, transformations): an entry of `new` is laid over the entry of
    `old` with the same `id` (boards, sections) or `name` (task commands), else the same place (transformations), so
    its unknown fields stay. An entry of `old` this front end couldn't read (the parse rejected it) is kept where it
    was. An entry you removed is gone.
  - Anything else (strings, numbers, lists of plain values such as tags or fields) is `new`'s.
- **Who calls it.** On the web, `serializeSettings`, `serializeBoards`, `serializeAppearance`, `serializeHomepage` and
  the keys' save; on Android the same in `AppSettings.kt`, `Boards.kt` and `KeysPanel.kt`; in the terminal
  `with_tui`, `with_tui_edit`, `written` (task commands) and the keys' save. The terminal already keeps the top level;
  it gains the depths.
- **The lint.** Each front end's parse already returns problems (`parseSettings`, `parseAppearance`, the board
  normaliser's errors, the keymap's). They are sorted into two kinds, with where each is (`boards[2].sections`,
  `taskCommands[0].name`):
  - **errors**: not JSON, not an object, over 64 KB, or a known top-level key of the wrong type (`homepage`,
    `appearance`, `keys` and `tui` not objects; `taskCommands` and `boards` not lists; `appearance.progress` not an
    object; `appearance.transformations` not a list; a board that is not an object or has no `sections` list);
  - **warnings**: everything else (a task command with a bad name, a board with a bad id, an unknown key name).
- **The box.** Web: `GeneralPanel.svelte`'s **settings.json** becomes a `<textarea>` (monospace, the whole file) with
  **Copy**, **Apply** and **Reset** (back to what is saved), and the lint under it. Apply with no errors `PUT`s the text
  as written to `/api/settings`, then the store reloads it. Android: the same on `GeneralPanel.kt`, through the
  engine's `api` call. Terminal: General lists the lint after a change (it already lists what is wrong).
- **Tests.** The overlay on each front end against the shared fixture (each kind of save, an unknown key at each depth,
  an unreadable entry, a removed one, a cleared field); the lint's two kinds on each; a web component test that a
  paste with an error can't be applied and one with a warning can.

## Acceptance criteria

- [x] Every front end has `settings.json` under General. On the web and the phone it is editable, a paste replaces
      it, and Apply saves it as written unless the lint finds an error; errors stop Apply and warnings don't, and each
      is listed with where it is. The terminal app lists the same after an edit in your editor.
- [x] A round trip on each front end keeps everything it doesn't use: a file with an unknown top-level key, an unknown
      key inside `appearance`, inside a board and its sections, inside a task command, and the `tui` section, saved
      through each kind of edit, comes back with all of them.
- [x] An unusable task command or board is listed and still in the file after any save.
- [x] Tests: the round trip above on the web (`appsettings.test.ts`), Android (`AppSettings` tests) and the terminal
      (`settings.rs`), the same fixture on each.
- [x] Docs: `docs/using/settings.md` (the editable file; "dropped when you apply" becomes "kept"), `docs/terminal.md`.

## Open questions

None left.

Decided (2026-11-17, by the person):

- Apply lints the file: errors stop it, warnings are listed and kept. **What** 1 and the first criterion say so
  (their wording changed with the person's yes).
- `settings.json` is under General on every front end.

## Implementation notes

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.

2026-11-17: built on all three front ends.

- **A departure from the design: a three-way merge, not `overlay(old, new)` with `null` for a cleared field.** `null` is
  already a value in the file (`"app.undo": null` in `keys`), so it can't also mean "remove". Each save is made twice
  instead: once with what was read, unchanged (the *base*, what the front end understood), and once with the edit.
  `mergeSettings(old, base, next)` keeps what is in the file and not in the base, which covers a key it doesn't know at
  any depth and an entry it couldn't read. It takes out what is in the base and not in the edit, and nothing else. A
  section taken out keeps what wasn't understood of it.
  - Lists are matched by `id`, then `name`, then place among the entries with neither, as designed. An unreadable entry
    is put back at its old index.
  - The base also writes the empty sections and lists that were in the file, since they were understood too. Without
    that, clearing the last progress bar would leave `"progress": {}`.
  - Two save-specific fixes after the merge: a board saved from the app loses its legacy `ranges`, because reading moves
    them into `controls` and the merge would otherwise name them twice; and `homepage` is exactly the choice, because
    it is one of two shapes.
- **Where it is.**
  - Web: `appsettings.ts` (`mergeSettings`, `lintSettings`, the `with*` builders behind each `serialize*`), and
    `GeneralPanel.svelte` (the box: Copy, Reset, Apply, the lint).
  - Android: `AppSettings.kt` (the same functions), `GeneralPanel.kt` (the box), and `Store.saveSettingsFile`.
  - Terminal: `settings.rs`. `merge`, `lint_errors`, and `read_commands` read task commands one by one, so a bad entry
    is a warning, not an unreadable file. Only `with_commands` merges; the other saves no longer re-serialise the
    commands, and `written` writes each entry's `name`, `command` and `icon` first. The General section lists errors
    line by line, then the command warnings.
- **Lint.** The errors are the design's list, plus `web` and `mobile` not being objects (they hold keys too).
  - A `homepage` that is not an object is now an error everywhere, where it used to be read as no choice.
  - The terminal app reads a file with an error as none, as it did with a file that was not an object, and won't save
    over it.
- **Tests.**
  - The shared fixture is `fixtures/settings-kept.json`: 12 merge cases, plus a file with unknown keys at every depth
    and an unusable command and board.
  - It is run by `appsettings.test.ts`, `SettingsKeptTest.kt` (androidUnitTest, which can read files) and
    `settings.rs`.
  - `GeneralPanel.test.ts` checks that a paste with an error can't be applied and one with a warning can.
  - Two older tests changed: "an unreadable command is dropped on save" became "kept", on the web and Android.
- **Docs.** `docs/using/settings.md` and `docs/terminal.md`. The Commands panels, web and Android, say an unusable
  entry is kept.
