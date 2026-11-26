# 0037: A mark after a repeating task's title

- **Status:** done
- **Created:** 2026-11-27
- **Layer:** 2 (UI)
- **Front ends:** all (web, Android, terminal)
- **Supersedes / related:** spec 0035 (iterative tasks), 0036 (the Repeat choice).

## Why

A recurring task had a repeat icon in the web's and the phone's reports, and nothing on a card or in the terminal; an
iterative task had none anywhere, so the two kinds could not be told apart from a plain task at a glance.

## What

After the title, in reports and on cards (kanban, the phone's task cards, the web's task cards for dependencies):

| Kind | Web and phone (Lucide) | Terminal |
| --- | --- | --- |
| Recurring (template or instance) | `repeat` | `↻` |
| Iterative (open, with an `iter`) | `iteration-cw` | `↪` |

The web's has a tooltip ("Recurring task: repeats weekly", "Iterative: done, it makes the next (monthly); number 3"),
and the same words for a screen reader. A completed task of a series has no mark.

## Not doing

- A setting to change the marks; `recurrence.indicator` stays the `recur.indicator` column's.

## Design

Web: `RepeatMark.svelte`, in `ReportCell.svelte`, `KanbanPage.svelte` and `TaskCard.svelte`. Phone: `repeatMark` and
`RepeatMark` in `Repeat.kt`, in `ReportTable.kt` and `TaskCard.kt`. Terminal: `model::Repeats` per report row, drawn after
the description cell (`ui.rs`) and a card's title (`kanbanui.rs`).

## Acceptance criteria

- [x] Each kind has its mark in reports and on cards, on the three front ends.
- [x] Tests: `RepeatMark.test.ts`, `RepeatTest.kt`, `model::tests::a_task_repeats_as_its_export_says`,
  `kanbanui::tests::a_repeating_task_s_card_has_its_mark_after_the_title`.
- [x] Docs: `docs/using/recurring.md`, `docs/using/iterative.md`.
