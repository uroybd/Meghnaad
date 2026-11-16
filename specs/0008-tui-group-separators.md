# 0008: Group separators in the terminal app

- **Status:** done <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-17
- **Layer:** 2 (UI)
- **Front ends:** terminal (the web app and Android have headers already: `web/src/lib/groups.ts`, `Groups.kt`)
- **Supersedes / related:** [Grouping](../docs/using/tasks.md#grouping); 0007 (bulk selection)

## Why

A `/` on a sort key (`project+/`) groups a report: Taskwarrior leaves a blank line where the value changes, and the
web app and the phone draw a header above each group (`Project: Home`). The terminal app draws neither, so a grouped
report reads as one long list, and only the sort box says `(grouped)`.

## What

1. **A separator.** On the Tasks page, a report whose sort has a `/` gets a horizontal line above each group with the
   group's name in it, the web's header text:

   ```
   ── Project: Home ─────────────────────────────────────
     3  Pay rent          …
     7  Fix the tap       …
   ── Project: Work · Outcome: fail ─────────────────────
   ```

   `(none)` where the value is empty, ` · ` between columns when several keys have a `/`. The value is the cell as the
   report draws it (`project.parent`, a date in its format), and a UDA is named by its label.
2. **Grouping from the sort box.** In the `o` box, a key toggles the `/` on the chosen sort key (as the web's **Group**
   button does), so grouping needs no typing.
3. **Moving.** `j` `k` skip the separators (they are not rows); `[` `]` jump to the previous and next group.

## Not doing

- Folding a group away.
- Separators on other pages (Projects, Tags, boards).
- Anything in `task`'s own output (`i`, `:`): that stays as `task` prints it.

## Design

Agreed 2026-11-17.

- **Where groups start.** The terminal reads `export`, which carries no breaks, so it works them out as the engine does
  (`tc_core::run`, `break_value`): a group starts at a row whose raw value of any `/` sort key differs from the row
  above. `model::breaks(tasks, keys) -> Vec<bool>` compares the export's JSON values of each key's base attribute (a
  missing value is empty). The sort in force is what `sort_in_force` already gives; `sortspec::parse` gives the keys
  with `brk`.
- **What a separator says.** `crates/tui/src/groups.rs`, a port of `web/src/lib/groups.ts` (and `Groups.kt`) with its
  tests: for each group, `Label: value` per `/` key, joined with ` · `, `(none)` when empty. The value is the cell from
  `tc_core::helpers` (`format_cells`) for that key's column as the report shows it, or the plain column when the report
  doesn't; a UDA is named by its label. `Report` gets `heads: Vec<Option<String>>`.
- **Drawing.** `ui.rs` `table`: before a row with a head, a line `── Project: Home ───…` across the table's width, in
  `color.label`. Separators are lines, not rows: `selected` still counts rows, and the scroll counts lines, so the
  highlighted row stays in view with its separator above it.
- **Moving.** `j` `k` and the page moves step over rows as now. `[` `]` on the Tasks page call `section_step`, which for
  `"tasks"` goes to the first row of the previous or next group (nothing on an ungrouped report).
- **The sort box.** `sort.group` (`b`) in the `sort` context of `default.json`, with its `about`, the phone's copy and
  `WORDS`: it turns `brk` on or off for the chosen key; the box shows `(grouped)` as it does now. A report's own `/`
  needs nothing.
- **Tests.** The heads (ported); `breaks` against the real `task` 3.5.0 (scratch data, `rc.report.list.sort:project+/`
  and a UDA): rows of `export` split where `task list` puts its blank lines; a buffer test of the drawn separator and
  the highlighted row under it; `b` in the sort box adding and removing the `/`.

## Acceptance criteria

- [x] A grouped report shows a separator with the group's name above each group, the web's text for the same report.
- [x] The sort box toggles grouping on the chosen key.
- [x] `j` `k` move over rows only; `[` `]` go between groups.
- [x] Tests: the heads, ported with the web's tests; the TUI drawing a grouped report (a buffer test); a test against
      the real `task` 3.5.0 that the groups start where `task` puts its blank lines.
- [x] Docs: `docs/terminal.md` (the table, the sort box keys).

## Open questions

None left.

Decided (2026-11-17, by the person):

- The look is a line with the group's name in it, as in **What** 1.
- Grouping is on whenever the report's own sort has a `/`, with nothing to turn on; the sort box key only adds or
  takes away a `/`.
- The sort box key is not `/` (that is the command line's); `b` was chosen for it (see **Design**).
- `[` `]` on the Tasks page go to the previous and next group's separator, and do nothing on an ungrouped report.

## Implementation notes

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.

2026-11-17: built.

- **Where groups start.** `crates/tui/src/groups.rs` `breaks(tasks, keys)` compares each grouped key's base attribute
  as `export` writes it, with a missing value equal to an empty one, as `break_value` does.
- **Separator text.**
  - `groups::heads` is the port of `groups.ts`, with its four tests. `label_for` names a column the report doesn't
    show by its UDA label, else by the attribute with a capital.
  - `model::report` works the heads out in `group_heads`: each grouped column's cell, by the report's own column spec
    when it shows it, else a plain column. It puts them in `Report.heads`.
- **Drawing.** A row that starts a group gets `top_margin(1)`, which ratatui counts when it scrolls. After the table is
  drawn, `ui::separators` writes `── <head> ───…` in `color.label` into those lines, starting from the table state's
  offset. This departs from "the scroll counts lines" in the design only in the mechanism.
- **Moving.** `[` `]` on the Tasks page go through `section_step` to `groups::step`: to the next group's first row, or
  back to this group's start and then the one before. They do nothing on an ungrouped report.
- **Sort box.** `sort.group` (`b`) turns `brk` on or off for the chosen key. It is in `default.json` (with its `about`)
  and `keys.rs` `WORDS`, and the phone's copy was regenerated.
- **Tests.** The unit tests are in `groups.rs`.
  - `tests/groups.rs` runs the real `task` 3.5.0 on scratch data, with `project+/,outcome+/` and a UDA labelled
    `Result`. The sizes of the groups match `task mine`'s blank lines, and the heads are the expected text.
  - It also draws the app into a `TestBackend` (each separator above its first row) and checks `]`, `j` and `[`.
- **Docs.** `docs/terminal.md`: the table, `[` `]`, and `b` in the sort box.
