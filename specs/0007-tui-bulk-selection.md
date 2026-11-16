# 0007: Select several tasks in the terminal app

- **Status:** done <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-17
- **Layer:** 2 (UI)
- **Front ends:** terminal (the web app has this already: [Bulk actions](../docs/using/tasks.md#bulk-actions); Android
  is left out here)
- **Supersedes / related:** the web's `selection.svelte.ts` and `BulkBar.svelte`; 0008 (group separators)

## Why

On the Tasks page every quick key (`m`, `d`, `s`, `x`, `D`, `A`, `c`) acts on the one highlighted task. To change
several at once you have to leave the list and type a filter at `:`, which means knowing a filter that picks exactly
those tasks. The web app lets you tick rows and act on them all; the terminal app should too.

## What

On the Tasks page:

1. **Marking.** `space` marks the highlighted task (or unmarks it) and moves down a row. A marked row shows a marker in
   front (`●`) and its own colour. `V` marks every row from the last one marked to the highlighted one (vim's visual
   line). `*` marks every row in the report, or unmarks them all when they are all marked. `Esc` clears the marks.
2. **The count.** While any are marked, the status line says how many (`3 marked`) and the hint line names the keys
   that act on them.
3. **Acting on them.** With tasks marked, the quick keys act on all of them, as command lines on their uuids in
   batches of 120 tasks, as the web does (so `undo` takes back a batch at a time):
   `d` is `task <uuids> done`, `s` start, `x` stop, `D` delete, `A` annotate (one note, added to each), `c` a task
   command (`<uuids> <name>`). Taskwarrior's own questions (`bulk`, `confirmation`, the per-task
   `(yes/no/all/quit)`) open in the box the quick keys already use. `m` opens [the form](../docs/terminal.md#the-form)
   empty: only the fields filled in are changed, on every marked task, as one `modify`; an emptied field is cleared.
4. **The command line.** `:` with tasks marked starts the line with them (their IDs as a list like `1,3-5`, and the
   uuid of any without one), as the web's **CLI** button does, so you type the rest.
5. **How long they last.** Marks live while their tasks are in the list: another filter, report, sort or page drops
   the ones that are no longer shown, and leaving the Tasks page clears them. After an action the list reloads and the
   marks are cleared.

## Not doing

- Marking on other pages (Projects, Tags, boards).
- Android: its own spec if wanted.
- A new engine command: everything is a `task` command line on uuids, as today.

## Design

Agreed 2026-11-17.

- **The marks.** `crates/tui/src/app/selection.rs`: `Selection { uuids: Vec<String>, anchor: Option<String> }`, a port
  of `web/src/lib/selection.svelte.ts` with its tests: `toggle`, `toggle_all`, `keep` (the uuids still in the list),
  `clear`, plus `range(from, to)` for `V` (from the anchor, the last one marked, to the highlighted row). It lives on
  `App` beside `selected`. `keep` runs wherever the report is loaded again (`App::load` and friends); leaving the Tasks
  page clears it.
- **The keys.** `tasks.mark` (`space`), `tasks.markRange` (`V`), `tasks.markAll` (`*`) and `tasks.unmark` (`esc`, in
  the `tasks` context, so it is tried before `app.back`, and only when something is marked) in
  `web/public/keymaps/default.json` with their `about`, the phone's copy, and `keys.rs` `WORDS`.
- **Acting.** `task_action` (`app/actions.rs`) takes its targets from the marks when there are any, else the
  highlighted row, and leaves `tasks.info` and `tasks.add` on the highlighted row. Many targets go through
  `App::bulk(words)`: the uuids in batches of `BATCH = 120` (the web's `bulk.ts`), each as one `quick` line
  (`<uuids> done`), one after the other, so each batch's questions open in the box as a single task's do. A refusal or
  `quit` stops the batches still to go. After the last, the list reloads and the marks are cleared.
- **`A` and `c`.** `A` asks for the note once, then `<uuids> annotate <note>` per batch. `c` picks the command once,
  then `<uuids> <name>` per batch.
- **`m`.** The form (`modify_form`) gets a "many" mode: it opens with every field empty and remembers which ones you
  touched; on save, a touched field with a value is `field:value`, a touched one left empty is `field:` (cleared), and
  the words go as `<uuids> modify …` per batch. Tags are `+tag` and `-tag` as typed.
- **`:`.** With marks, the prompt opens with `promptFor` from `bulk.ts`, ported with its test (`idList`: `1,3-5`, and
  the uuid of any task with no ID), then a space.
- **Drawing.** `ui.rs` `table`: a marked row has `●` in a one-cell gutter before the first column and is drawn with
  the `selected` role's shading lightened (the `marked` role, added to the theme with a default for each preset). The
  status line reads `3 marked` and the hint line lists the keys that act on them.
- **Tests.** The selection, `idList` and `promptFor` (ported from the web); `bulk` splitting 250 into 120, 120, 10; a
  test against the real `task` 3.5.0 (scratch data) that marks three tasks with `space` and `V`, presses `d` and finds
  them completed.

## Acceptance criteria

- [x] `space`, `V`, `*` and `Esc` mark, mark a range, mark all and clear, and the marked rows are drawn as such.
- [x] With tasks marked, `d` `s` `x` `D` `A` `c` act on all of them, and `task`'s questions are asked in the box.
- [x] `m` with tasks marked changes only the fields filled in, on all of them.
- [x] `:` starts with the marked tasks.
- [x] Marks follow the list: dropped when their tasks leave it, cleared after an action and when leaving the page.
- [x] Tests: the selection and the ID list (ported with the web's tests); a test against the real `task` 3.5.0 on
      scratch data marking three tasks and completing them.
- [x] Docs: `docs/terminal.md` (the keys table and a "Bulk actions" section), the screenshot of `?` if it changes.

## Open questions

None left.

Decided (2026-11-17, by the person):

- The keys are `space`, `V`, `*` and `Esc`, as in **What**.
- `Enter` (task info) and `a` (add) ignore the marks.
- A long selection goes as the web's does: in batches of 120 tasks, so `undo` takes it back a batch at a time. **What**
  3 says so (its wording changed with the person's yes).

## Implementation notes

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.

2026-11-17: built.

- **The marks.** `crates/tui/src/app/selection.rs`:
  - `Selection` (`toggle`, `toggle_all`, `range`, `keep`, `clear`), `id_list`, `prompt_for` and `batches` (`BATCH = 120`).
  - The web's `selection.test.ts` and `bulk.test.ts` cases are ported, plus a range test.
  - `App::marks` keeps only what is in the list on each `reload` (both load paths). `App::open` clears them for any
    page but Tasks.
- **The keys.** `tasks.mark` (`space`), `tasks.markRange` (`V`) and `tasks.markAll` (`*`) are in `default.json` with
  their `about`, in the phone's copy, and in `keys.rs` `WORDS`.
  - *Departure:* there is no `tasks.unmark` on `esc`. The keymap's test `the_defaults_have_no_clashes_and_shadow_nothing`
    forbids two actions sharing a key in one context.
  - Instead, `app.back` on the Tasks page clears the marks first when there are any. The behaviour is the same as
    **What** 1 asks.
  - `app.back`'s `about` is unchanged, so the `?` screenshot still matches.
- **Acting** (`app/actions.rs` `task_action`):
  - With marks, `d` `s` `x` `D` go through `App::bulk` (`app/job.rs`): one `Quick` job with a step per batch, joined by
    `&&`, so a refusal or `quit` stops the rest. Questions open in the box as before.
  - `A` opens `PromptKind::AnnotateMany`. `c` passes the uuids to the commands box (titled "N marked tasks"), which runs
    `<batch> <name>` lines joined by `&&` through `console`.
  - `Enter` and `a` stay on the highlighted row.
- **`m`.** `Form::many(uuids, cfg)`:
  - The fields are the web's bulk Modify set: no description, no start.
  - `Field::touched` is set by typing (`edit_field`) or by `Tab` completion.
  - `many_changes` sends `name:value` (empty clears), tags as typed (`+` by default) and `depends:x`. `statements` are
    the batches.
  - `command()` shows `<N tasks> modify …`. The title says "modify N tasks: only the fields typed in".
- **`:`.** With marks, the prompt opens with `prompt_for` of the marked rows in list order.
- **Drawing.**
  - `ui.rs` `table` adds a one-cell gutter only while something is marked: `●` in the accent, and the row patched with
    the new `marked` role (`on #ddf4ff` light, `on #0c2d6b` dark; `theme::ROLES`, both presets, the Theme section's
    chips, and the `docs/terminal.md` colours table).
  - The status line starts with `N marked`. The bottom line lists the acting keys and `Esc unmark` (`keyhelp.rs`
    `page_keys`).
- **Tests.**
  - The selection, ID list, prompt and batches unit tests, plus `form.rs`
    `many_tasks_get_only_the_fields_typed_in_a_batch_at_a_time`.
  - `tests/bulk.rs` runs the real `task` 3.5.0 on scratch data. `space`, `j` and `V` mark three tasks, which are drawn
    with `●`, and the status says "3 marked".
  - The same test checks that `:` opens with `1-3 `, that `*` toggles all, and that Esc clears. Then `d`, answering `a`
    to `task`'s bulk question, completes exactly the three, and the list reloads with the marks cleared.
- **Docs.** `docs/terminal.md`: the four keys in the Tasks table, a "Bulk actions" section, and the `marked` role.
