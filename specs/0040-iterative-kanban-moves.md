# 0040: Iterative tasks on a kanban

- **Status:** done
- **Created:** 2026-11-27
- **Layer:** both (the rewrite is the engine's; the boards and their editors)
- **Front ends:** all (web, Android, terminal)
- **Supersedes / related:** spec 0026 (kanbans), 0035 (iterative tasks).

## Why

A kanban's automatic moves set `status:pending`, which `task` 3.6.0 refuses on an iterative task ("You cannot change the
status of an iterative task. Clear iter to end the series instead.", exit 2); its open columns filtered on
`status:pending`, which an iterative task doesn't match; and completing one on a board made the next with the column's
UDA copied, so it landed in Review again. Checked on scratch data with `task` 3.6.0. `task` asks nothing when an
iterative task is edited (its only question is "End the iterative series…?" when `iter` is cleared), so there is no
"this one or all" to ask (uroybd, 2026-11-27).

## What

- On an iterative card, every move statement (the On move, the task commands it runs written out, the person's own)
  leaves out its `status:` modifications; a statement left with nothing is skipped.
- A `done` there clears what the board's moves set: UDAs given a value (`name:`), tags added (`-tag`). One command:
  `task` takes `done progressState: -doing`, and the next task starts without them.
- The column editor shows, muted, the line a move runs on an iterative task where that differs.
- The automatic open columns filter on `+PENDING …`; settings version 7 changes the three filters the setup wrote.

## Design

`tc_core::board_move`: `board_clears`, `iterative_statement`, `config_with_udas`; helpers `board_clears` and
`iterative_statement` for the web and the phone. Migration `settings_version::open_columns_v6_to_v7`. Web:
`kanban.ts` (`boardClears`, `forIterative`, `iterativeMove`), `KanbanPage.svelte`, `KanbanPanel.svelte`. Phone:
`Kanban.kt`, `KanbanPage.kt`, `KanbanPanel.kt`. Terminal: `kanban.rs`, `app/kanbans.rs`, `kanbanui.rs`.

## Acceptance criteria

- [x] Moves and `done` on an iterative card, on the three; the muted line in each editor.
- [x] `+PENDING` columns and the migration.
- [x] Tests: `board_move::tests`, `version_6_automatic_open_columns_take_iterative_tasks`, the kanban tests of the three.
- [x] Docs: `docs/using/kanban.md`, `docs/using/iterative.md`.
