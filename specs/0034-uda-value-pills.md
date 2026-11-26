# 0034: A UDA's choices drawn as pills

- **Status:** done
- **Created:** 2026-11-26
- **Layer:** 2 (UI), with the cell's shape from `tc_core::cells`
- **Front ends:** all (web, Android, terminal)
- **Supersedes / related:** the kanban card's UDA badge (spec 0026, `KanbanPage.svelte`, `TaskCard.kt`,
  `kanbanui.rs`); tags as chips (`Cell.chips`); `color.uda.<name>.<value>` (`tc_core::color`); the rule in AGENTS.md
  that the taskrc's colours are a subtle tint.

## Why

A UDA with choices (`uda.size.values=S,M,L`, `uda.progressState.values=inprogress,review`) is a small set of states, but
every front end draws its value as plain text, the same as a free-text UDA. A pill per value, each value its own colour,
lets a row or a card be read at a glance, as tags already are.

## What

uroybd: "For uda with choices show values as distinct badge/pill style in an UI".

- Wherever a report cell, a card field or a task's info shows a UDA that has `values` in the taskrc, its value is a
  pill: rounded, a soft background of its own colour, the value's text. A UDA without `values` is drawn as now.
- Each value of a UDA keeps its colour everywhere and on every front end: it goes by where the value is in
  `uda.<name>.values` (the first value the first colour of a small palette, and so on round).
- Where the taskrc gives the value a colour (`color.uda.size.L=red`), the pill is that colour, as a tint (AGENTS.md:
  taskrc colours are never full colour).
- On a kanban card, the UDA's badge (its label, then its value) takes the value's colour on its value half.
- The empty value (`uda.x.values=a,b,` allows none) shows nothing, as now.

## Not doing

- `priority`: it already has its own colours (the card's ribbon and the priority pill), and stays as it is.
- Editing the value from the pill (a picker on click): the form already offers the choices.

## Design

- **The cell.** `tc_core::cells::Cell` gets `choice: Option<i64>`: the value's place in the UDA's `values`, `-1` for a
  value not among them (a grey pill), `None` for a UDA without choices or an empty value. Made once in the engine for
  the report and the card; the info, which has no cell, works out the same place from the UDA's `values`. A test in
  `cells.rs`.
- **The colour.** A value takes a chart's default colour for its place (blue, red, green, yellow, magenta, cyan, then
  the bright blue, red and green: `DEFAULT_SERIES`), so a pill and a chart of the same UDA agree; `color.uda.<name>.<value>`
  wins, as a tint. Web `choiceColor` (`colors.ts`), phone `choiceColor` (`BoardColors.kt`), terminal `theme::choice`
  (the badge's ground, the value in its colour, bold; grey in `mono`). Drawing stays in each UI, per AGENTS.md.
- **Where.** Web: `ReportCell.svelte` (the table, the cards, the info). Android: `TaskCard.kt` and the table cell.
  Terminal: the report table's cell, the card's badge (`kanbanui.rs`) and the info.
- **Docs.** `docs/using/` (reports and kanban), `docs/terminal.md` (Colours), the theme roles.

## Acceptance criteria

- [x] A UDA with `values` shows its value as a pill in the report table, on a kanban card and in a task's info, on the
  web, the phone and the terminal.
- [x] The same value has the same colour on all three; two values of a UDA differ (up to 8).
- [x] `color.uda.<name>.<value>` in the taskrc colours its pill, as a tint.
- [x] A UDA without `values`, an empty value and `priority` are drawn as before.
- [x] Tests: `tc_core::cells` (the place of a value, none for the rest), a web component test, a Kotlin test, a
  terminal drawing test.
- [x] Docs: as in Design.
- [x] Size: the browser engine measured before and after.

## Open questions

- Answered by uroybd: always on (no setting); a value not in `values` is a grey pill; all three front ends at once.
- In the table, a UDA column whose values are long (`inprogress`): the pill is as wide as its text (the terminal's
  column is two wider for the padding).

## Implementation notes

- 2026-11-26: draft written from uroybd's request; Design proposed. uroybd answered the open questions (always on, a
  grey pill for a value not in the list, all three front ends) and said to build it.
- 2026-11-26: built. Engine: `Cell.choice` and the UDA's `values` in the cells context (`format.ts`, `Format.kt`,
  `model.rs` pass them). Web: `ChoicePill.svelte` in `ReportCell` (table, cards) and `TaskInfo`, the kanban badge's
  value half in `KanbanPage`. Phone: `ChoicePill.kt` in `ReportTable`, `TaskCard` (pill, and the badge's value half)
  and `TaskInfo`. Terminal: `theme::choice`/`theme::pill`, the table cell (`ui.rs`), the card (`Card.choices`,
  `kanbanui.rs`) and the info (`Shown::Choice`). Tests: `cells.rs`, `ChoicePill.test.ts`, `colors.test.ts`,
  `BoardChartsTest.kt`, `kanbanui.rs`, `info.rs`. Docs: `docs/using/tasks.md`, `kanban.md`, `docs/terminal.md`.
  Browser engine: 909,849 → 910,964 bytes gzip (+1,115).
- 2026-11-26: uroybd asked, while this was built, that short string UDAs sit inline on cards: a transformation's
  link counted as Markdown and so as a paragraph, measured before the transformation. Now a tree of one line is
  measured by the words it shows (`inlineText`, web `markdown.ts`, phone `Kanban.kt`) and drawn in line; the limit is
  30 characters (uroybd), on the web, the phone and now the terminal (`Card.free`, `SHORT_TEXT`).
