# 0005: Stack a chart by a field

- **Status:** in progress <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-16
- **Layer:** 2 (UI)
- **Front ends:** all: web, Android (screens and the widget), terminal app
- **Supersedes / related:** boards (`tc_core::board`), series and stacking (`docs/using/boards.md`)

## Why

A chart's groups are its axis, and the colours within a bar are its series, which are written by hand. "Done per week,
stacked by project" means a series for each project, written and kept up to date as projects come and go.

## What

1. A bar, line or area chart can be **stacked by** a field: each of its values becomes a series of its own, in its own
   colour, on the same axis, stacked unless **Stacked** is turned off.
2. Series stay for what a field's values cannot say (filters of their own, a burndown's states, a measure of their own).
   A chart is stacked by a field or has several series, not both.

## Not doing

- Stacking by a date or a part of one (a heatmap's rows do that), or by groups you write (that is what series are).
- A pie, donut, polar, radar or heatmap stacked by a field.
- A chart stacked by a field and with several series at once.
- Clicking a stacked part to see its tasks (no chart part is clickable today).

## Design

**In `settings.json`** a section gets `stackBy`: `{ "field": "project", "only": ["a", "b"], "except": ["c"] }`
(`only` and `except` optional). `tc_core::board::Section::stack_by`, checked by `normalize`: a bar, line or area chart
only, a field that is not empty, and at most one series. With `stackBy` and no `stacked`, the chart is stacked.

**The engine** (`tc_core::board::aggregate`): `StackBy` and `split(series, stack)`, which turns the one series into a
series for each value of the field, its tasks those with that value (or, with groups you write, each group's tasks with
that value), its measure and span the series' own. It works as grouping by a field does: a task with none is under
`(none)`, one with several tags under each, and a date is refused. `only` and `except` pick values (by value or by
`(none)`). The series are ordered with the most tasks first (at the bottom); past `MAX_SERIES` the largest 11 are kept
and the rest go into one `Other`. The `board_chart` helper takes the `stackBy` as a fifth argument and splits before
`chart`; the terminal app calls `chart_by` directly.

**The front ends** pass the section's `stackBy` to `board_chart`, draw the datasets it returns as they draw series
(the legend and the colours come from the datasets, not from the section's series), colour a value of a UDA with its
`color.uda.<name>.<value>` as a one-series chart's bars are, and treat a chart with `stackBy` and no `stacked` as stacked.
The section editor (web `SectionForm.svelte`, Android `BoardEditor.kt`) gets **Stack by** (a field) with **Show only**
and **Leave out** for its values, for a bar, line or area chart with at most one series; adding a second series is off
while it is set. The terminal app has no board editor; it draws what `settings.json` says.

## Acceptance criteria

- [ ] A bar, line or area chart with `stackBy` draws a series for each value, stacked, in the web app, on Android (and
      its widget) and in the terminal app.
- [x] `only`, `except`, `(none)`, tags (a task under each), and the `Other` past 12 values work as described.
- [x] `normalize` refuses `stackBy` on other kinds, with an empty field, or with two series or more.
- [x] The web and Android editors set and clear Stack by, Show only and Leave out.
- [x] Tests: `aggregate::split` and `normalize` in `tc-core`, the helper, web `boards.test.ts` and `SectionForm.test.ts`,
      Kotlin `BoardsTest`/`BoardsEditTest`, the terminal's `boards.rs`.
- [x] Docs: `docs/using/boards.md` (Stack by, and when series are still the answer).
- [x] Size: engine wasm measured before and after; the Worker is untouched.

## Open questions

- None yet.

## Implementation notes

- 2026-11-16: Built.
  - **Engine** (`tc_core::board`):
    - `aggregate::StackBy`, `split` and `chart_by`, and `keys_of`, which grouping by a field now shares.
    - `Section::stack_by` and `Section::is_stacked`, with `normalize`'s checks and tidying.
    - The `board_chart` helper's fifth argument.
  - **Web:**
    - `boards.ts`: `StackBy`, `isStackableType`, `isStacked`, `stackKey`, and `loadBoard` passing the stack.
    - `BoardSection.svelte`: the series colours.
    - `SectionInfo.svelte`: a "Stacked by" line.
    - `SectionForm.svelte`: Stack by, Stack only and Stack leaving out.
    - `BoardView.svelte`: the reload key includes `stackBy`.
  - **Android:**
    - `Boards.kt`: the model, `isStacked`, `stackKey`, and `layChart` passing the stack.
    - `BoardsEdit.kt`: `withStack`, `seriesFull`, `withStacked`, and `withType` clearing the stack.
    - `BoardEditor.kt`: the editor fields.
    - `BoardColors.kt` and `BoardCharts.kt`: `stackColors`.
    - `BoardGrid.kt`, `BoardsPage.kt` (the reload key and the info panel) and the widget (`BoardWidget.kt`, `BoardRenderer.kt`).
  - **Terminal app:** `boards.rs` calls `chart_by` and `is_stacked`; `boardui.rs` has `drawn_series`, the legend and colours from the datasets.
  - **Departures:**
    - The terminal app draws bars side by side, stacked or not, as it already did. It doesn't colour a UDA's values, as it doesn't for a one-series chart either. The side-by-side bars are noted in `docs/terminal.md`.
    - Android offers the field as the editor's suggestion chips, not a list.
  - **Checks:** all pass.
    - `cargo test`: 1045.
    - Web: 814 tests in 89 files. `svelte-check` and the scripts' 34 tests pass.
    - Both wasm checks pass.
    - Kotlin: `:shared:testDebugUnitTest`, `:androidApp:assembleDebug` and `:androidApp:testDebugUnitTest`.
    - Lint and `mdbook build` are clean.
  - **Sizes:**
    - Engine: 849,674 → 855,925 bytes (+6,251).
    - Worker: 759,883 → 759,895 bytes (+12).
  - **Left:** the first criterion. The charts haven't been looked at on a screen on any platform (web, the phone and its widget, the terminal); only tests cover them so far.

