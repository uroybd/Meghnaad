# 0004: Filter and sort before exporting

- **Status:** dropped <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-16
- **Layer:** 2 (UI)
- **Front ends:** all. The Export dialog on the web, on Android and in the terminal app, and `meghnaad export`.
- **Supersedes / related:** [0002](0002-meghnaad-cli-export.md) (the Export dialog everywhere and `meghnaad export`)

## Why

An export is the table it was opened from: its report's filter and sort. Often the file wanted is a narrower or
differently ordered slice of that table, and today the only way to get it is to change the page first, or to write the
`rc.` override by hand on the command line.

## What

1. The Export dialog takes an extra filter and a sort before it exports, on every front end.
2. `meghnaad export` takes the same.
3. The command line the dialog shows is the one that runs, filter and sort included.

## Not doing

- Remembering the filter or the sort between dialogs: they belong to the table the dialog was opened from (format and
  fields stay remembered, as now).
- Group breaks (`/` in a sort) or anything else that only changes how a report is drawn.
- The report's `limit`: `task` 3.5.0's `export` ignores it, and so does this.
- A sort picker or column chooser: the sort is typed, as in the taskrc.
- Saving the filter and sort into the report (`config report.<name>.sort`).

## Taskwarrior parity

Nothing changes in the engine; this is about which command line the front ends send. What `task` 3.5.0 does with the
lines they will send, on a scratch `TASKRC` and data directory (four tasks, `bravo` done):

```text
$ task export                                          # no report: by id, no sort
bravo alpha charlie delta
$ task export next                                     # the report's sort
charlie bravo alpha delta
$ task rc.report.next.sort=description- export next    # the sort overridden for this one line
delta charlie bravo alpha
$ task rc.report._x.columns=id rc.report._x.filter= rc.report._x.sort=description- export _x
delta charlie bravo alpha                              # a report made on the line: the same tasks as plain export
$ task rc.report.next.sort=nosuch+ export next
The 'nosuch' column is not a valid sort field.         (exit 2)
$ task rc.report.next.limit=2 export next              # limit is ignored by export
charlie bravo alpha delta
```

A filter with `or` needs its own parentheses before more words are added: `project:x or pri:H due.any:` reads as
`project:x or (pri:H and due.any:)`.

## Design

**The line.** The dialog's line is `export_line`'s today (`[( context )] <filter> export <report>`). With an extra
filter and a sort it becomes

```text
rc.report.<report>.sort=<sort> [( context )] ( <filter> ) ( <extra> ) export <report>
```

The table's own filter and the extra words are each put in parentheses only when there are both (so a line without an
extra filter is exactly today's). The sort is the `rc.` override the header-sort already uses for a table
(`sortspec::with_sort_override`), so it is `task`'s own sort, with `task`'s own error for a bad column. An empty sort
leaves the report's.

This is one pure function every front end would otherwise write again, so it goes beside `export_line` in
`tc_core::table_export` (`refine(line, extra, sort)`) and through the existing `export_line` helper, which takes two
more optional arguments. Nothing else in the engine changes.

**The dialog** (web `ExportDialog.svelte`, Android `ExportDialog.kt`, the terminal's `app/export.rs`), the same
everywhere, between the command line and the format:

- **Filter**: a line of text, empty to start. Its words are added to the table's filter.
- **Sort**: a line of text, empty to start, with the report's own sort shown as its placeholder (`report.<name>.sort`
  from the settings the dialog already has), written as in the taskrc (`due+,priority-`).
- The export runs again when a field is left or ↵ is pressed in it (the terminal: ↵ on its row), not on each key. The
  command line, the task count and the field list follow the new result; a failed line (a bad filter or sort) shows
  `task`'s error in place of the count and Save is off until it is fixed.
- Leaving both empty gives today's dialog and today's file.

**`meghnaad export`.** The filter is already there: the task words after our options are the filter. One new option:

| Short | Long | What |
| --- | --- | --- |
| `-s` | `--sort SPEC` | The order, written as in the taskrc. With `-r`, the report's sort is replaced for this line (`rc.report.NAME.sort=SPEC`). Without it, a report made on the line (`rc.report._meghnaad.columns=id rc.report._meghnaad.filter= rc.report._meghnaad.sort=SPEC … export _meghnaad`), which exports the same tasks as plain `export`, in that order. |

With no `-s`, the output stays byte for byte `task`'s, as spec 0002 promises. A bad sort is `task`'s error and exit
status, passed through.

## Acceptance criteria

- [ ] The Export dialog on the web, Android and in the terminal has Filter and Sort; the line shown is the line run,
      and the file has only the filtered tasks in the sorted order.
- [ ] A filter with `or` in the table's own filter keeps its meaning with an extra filter added.
- [ ] Both empty gives the same line and file as before.
- [ ] A bad filter or sort shows `task`'s error in the dialog, and Save is off.
- [ ] `meghnaad export -s SPEC` sorts with and without `-r`; without `-s` the output is unchanged.
- [ ] Tests: `table_export` (the line), web `ExportDialog.test.ts`, Kotlin `ExportFile` test, the terminal's
      `export_dialog.rs`, and the CLI's tests against the real `task` (with and without `-r`, a bad sort).
- [ ] Docs: the Export dialog in `docs/using/`, `docs/terminal.md` (`meghnaad export -s`), `docs/android.md` if it
      describes the dialog.
- [ ] Size: engine wasm measured before and after.

## Open questions

- The CLI already filters with its trailing words. Is a `--filter` option wanted anyway, or is `-s` the only addition?
- Should the dialog run the export again as you type (after a short pause) rather than on leaving the field or ↵?

## Implementation notes

- 2026-11-16: Dropped before building. `meghnaad export` already filters (the words after its options) and sorts with
  an `rc.` word (`-r next rc.report.next.sort=due+`, or a report made on the line for no report), so the CLI needs no
  change, and the Export dialogs need none either. Checked with the built `meghnaad` on a scratch `TASKRC`. The two
  sort lines are now examples in `docs/terminal.md`; the `-r` help, `docs/terminal.md` and 0002's design no longer say
  the report's limit applies (`task export` ignores it).
