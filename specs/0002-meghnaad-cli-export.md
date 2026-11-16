# 0002: CLI Export

- **Status:** in progress <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-16
- **Layer:** 2 (UI)
- **Front ends:** all. One Export dialog, the same on every platform (web, Android, terminal app), with JSON, CSV and TSV; the terminal also gets the `meghnaad export` command. All from the same `tc_core::table_export`.
- **Supersedes / related:** the Export dialog (`tc_core::table_export`, commit 15f8490); the rename of `meghnaad-tui` to `meghnaad` (made before this spec)

## Why

Currently, we have an advanced export wrapper that allows 2 formats and control on which data to get. It will be nice to expose this in cli so that other programs can work on them.

## What

1. an `meghnaad export` command that is backwards compatible with `task export`
2. Have additional options to control stuff before exporting like ui.
3. Output is in stdout.
4. An additional format that is easily parsable by xargs like program (with unix standard separator so that they can detect columns properly.)

## Not doing

- A CLI on the web app or the phone. The Export dialog only gains TSV as a format: no header switch, no NUL ends (both
  are for pipes, not files).
- An Export button anywhere on Android or in the terminal app where the web app has none.
- A different dialog per platform: the choices, their order and their defaults are the same everywhere; only the drawing
  and how the file is saved differ.
- The context, and any option for it (the Export dialog has none): like `task export`, it is never applied.
- Any command but `export`: `meghnaad` with no arguments still opens the terminal app, and nothing else is passed
  through to `task`.
- Writing a file: the output is always standard output (redirect it).
- `import`, or reading an export back.
- The user's field transformations (spec 0001): an export is the data, not what a screen draws.
- Dates in `dateformat`, local time, or any other per-user date format, in CSV or TSV.
- A trailing report name (`meghnaad export next`): the report is always `-r`.
- The ASCII unit and record separators (0x1F, 0x1E) as a format: hardly any tool reads them by default.

## Taskwarrior parity

`task export` has no options of its own: `task <filter> export [report]`, with `rc.json.array` choosing a list or one
task to a line. It **ignores the context**, with or without a report name (`CmdExport`, `src/commands/CmdExport.cpp`;
seen below, and what `table_export` already assumes). Run on a scratch `TASKRC` and data directory, with the context `w`
(`project:work`) active and one task in `work`, one in `home`:

```text
$ task --version
3.5.0
$ task export | grep -c '"uuid"'
2
$ task export next | grep -o '"description":"[a-z]*"'
"description":"one"
"description":"two"
$ task rc.json.array=off export | head -1
{"id":1,"description":"one","due":"20261119T180000Z","entry":"20261116T090406Z",...}
$ task export nosuch
Unable to find report that matches 'nosuch'.        (exit 2)
```

`meghnaad export` keeps all of this: no context, and with no options of ours it is byte for byte what
`task … export [report]` prints, its exit status and its errors included. Only the options below are added.

## Design

**The program.** `meghnaad export …` in the terminal app's binary (`crates/tui`, the program `meghnaad`; the crate
keeps the name `meghnaad-tui`). `main.rs` sends `export` to a new `crates/tui/src/export.rs` before anything opens the
terminal; `meghnaad` alone opens the app as now. It runs the real `task` (`task.rs`, `MEGHNAAD_TASK`, `TASKRC`,
`TASKDATA` honoured) and reshapes what `export` prints with `tc_core::table_export`, as the Export dialog does. Layer 2:
it is what a shell script around `task export` would do, and it adds nothing to how `task` runs.

**The command line.** `meghnaad export [options] [--] [task words…]`.

- Our options are read only before the first word that is not one of ours (or up to `--`). Every word after that goes to
  `task` untouched, because a filter word can start with `-` (`-home` leaves out the tag `home`) and would otherwise be
  taken for an option. A word starting with `--` that is not ours is a mistake, since no task word starts so.
- The task words are the filter and `rc.` overrides, as you would put them before `export`. The report is `-r NAME`
  (below), not a trailing word, so a filter word is never mistaken for a report name.

| Short | Long | What |
| --- | --- | --- |
| `-f` | `--format json\|csv\|tsv` | The output. JSON when left out. |
| `-F` | `--fields all\|table\|a,b,c` | Every field the tasks have (the default), the report's columns (needs `-r`), or the ones named, in that order. |
| `-r` | `--report NAME` | Export as `task <filter> export NAME`: the report's filter, sort and limit apply. |
| `-a` / `-A` | `--array` / `--no-array` | JSON only: a list, or one task to a line. As `json.array` says when left out. |
| `-H` | `--no-header` | CSV and TSV: no header line. |
| `-0` | `--null` | TSV only: each task ends with NUL instead of a newline, for `xargs -0`. |
| `-h` | `--help` | The usage. |

An unknown `--` option, a format or field list that is not one of these, `table` without a report, or `-a`/`-0` with a format
they do not apply to is an error on standard error with exit status 2, before `task` runs.

**What runs.** `task [task words] export [report]`, its standard error and exit status passed through. With no options
of ours, standard output is what `task` printed, untouched. With options, it is parsed (`table_export::parse`), the
fields chosen (`chosen_fields`; `table` reads the report's `columns` from `_show`, as the app does), and written in the
format.

**The formats.**

- **JSON**: `export`'s own values (dates as `20261119T180000Z`), only the fields chosen, a list or a task to a line
  (`table_export::to_json`).
- **CSV**: RFC 4180, as the Export dialog writes it (`to_csv`), but with the dates written for a spreadsheet (below).
- **TSV**: one task to a line, fields joined by a tab, a header line first unless `-H`. A tab, a newline, a carriage
  return or a backslash in a value is written `\t`, `\n`, `\r`, `\\`, so a line is always one task and a tab always
  ends a field (`cut -f`, `awk -F'\t'`, `sort -t`, `while IFS=$'\t' read`). With `-0` the line ends with NUL instead.
- **Lists** in CSV and TSV as in the dialog: tags and depends joined by spaces, annotations by their text joined by `;
  `.

**Multiline values.** A description, an annotation or a string UDA can hold line breaks (and tabs), and every format
keeps them without losing the task boundary:

- **JSON**: as `export` writes them (`\n` inside the string).
- **CSV**: the value in quotes with its line breaks as they are (RFC 4180). A spreadsheet shows them as line breaks in
  one cell; this is the format for a spreadsheet when values have several lines.
- **TSV**: escaped (`\n`, `\r`, `\t`, `\\`), so one line is always one task and `cut`, `awk` and `read` never see half a
  task. A spreadsheet opening a TSV shows the `\n` as it is; the docs say to use CSV there. To get the text back,
  `printf '%b'` (or any `\`-unescape) undoes it exactly.
- **Annotations** in CSV and TSV are one cell, joined by `; `; an annotation's own line breaks stay inside it (quoted in
  CSV, escaped in TSV).

**Dates in CSV and TSV.** A date field (one `export` writes as `YYYYMMDDTHHMMSSZ`: `entry`, `due`, `wait`, a date UDA,
…) is written `YYYY-MM-DD HH:MM:SS` in UTC. Excel, LibreOffice and Google Sheets all read that as a date and time
whatever the locale, it still sorts as text, and it is the same on every machine. A `duration` stays as `export` writes
it. Which fields are dates comes from the taskrc (the built-in date attributes and `uda.<name>.type=date`), not from
guessing at the text. It is done in `tc_core::table_export`, where `to_csv` is, and the Export dialog's CSV and TSV
(through the `export_file` helper) get the same dates. JSON is unchanged everywhere.

**TSV in the Export dialog.** `table_export::Format` gets `Tsv` beside `Json` and `Csv`, with `to_tsv` used by both the
CLI and `render`: a file `<name>.tsv`, type `text/tab-separated-values`, with its header line and newline ends (the
CLI's `-H` and `-0` stay CLI-only). The web dialog gets a third radio, TSV (`ExportDialog.svelte`, `Format` in
`exportFile.ts`), and remembers it as it does the others.

**The Export dialog on Android.** A port of the web app's, as the app's other screens are (AGENTS.md: the view logic is
ported with its tests, the Taskwarrior knowledge is the engine's through `callHelper`):

- `ExportFile.kt` from `exportScope.ts` and `exportFile.ts`: the same helpers (`export_line`, `export_fields`,
  `table_fields`, `json_array`, `export_file`), nothing worked out in Kotlin. They take the settings as the engine sent
  them, so `ConfigResponse` keeps that whole object (`raw`) beside what it parses.
- An Export button beside the count under a table, as on the web: the Tasks page (`TasksView.kt`), a report in the
  console (`ResultView.kt`) and a board's table (`BoardGrid.kt`, whose entry now carries the words it ran), in the
  page's context.
- `ExportDialog.kt`: the command line it ran, then format (JSON, CSV, TSV), JSON's list or line choice, and fields (all,
  the table's, or ticked), as on the web. It saves through the app's file saver (`store.files.save(name, mime, text)`,
  as `FileView` does), and remembers the last format and fields in `Prefs`.

**The Export dialog in the terminal app.** The same dialog, drawn in ratatui, ported from the web app's with its tests
(`crates/tui`, beside `model.rs`; nothing new in `tc-core`):

- `e` opens it, by default, on the Tasks page's report (`tasks.export`) and on a board (`boards.export`; a box asks
  which table when the board has several), where the web app has its button: new actions in
  `web/public/keymaps/default.json` under the terminal's contexts, as AGENTS.md asks for keys; `e` is free in both
  (only Settings → Commands uses it), and it can be rebound in Settings → Keys. Inside the dialog the keys are fixed, as
  in the app's other boxes that take typing (the form, task commands): `↑` `↓` (`j` `k`, `Tab`), `Space`, `Enter`,
  `Esc`, and typing on the path line.
- The dialog: the command line it will run (`<filter> export <report>`, with the page's context in front, as the web
  app's: `table_export::export_line` through the helper), then format, JSON's list or line choice, and fields (all, the
  table's, or ticked from a list), in the web app's order with its defaults.
- It saves to a path the person can edit, filled in with the engine's file name in the current folder (the engine names
  it: `table_export::file_name`, the name `export` gives its file); an existing file is only replaced after a second
  `Enter`. It remembers the last format and fields in `settings.json` (`tui.export`), as the app keeps its other choices.
- This dialog's export takes the page's context, as the web app's and Android's do; `meghnaad export` on the command
  line does not (above). The dialog exports what the screen shows; the command is `task export`.

**Docs.** `docs/terminal.md` gets an Export section (the options table, the formats, `xargs` and spreadsheet examples);
`docs/using/pages.md` gives the dialog's TSV and its CSV and TSV dates (UTC, in that form); `docs/using/phone.md` says
the phone has the Export button; `docs/terminal.md` gives the dialog's key; `docs/architecture.md` names `export.rs`;
the README's terminal section mentions it.

## Acceptance criteria

Proposed; edit as you see fit.

- [x] `meghnaad export [task words]` with no options of ours prints exactly what `task [task words] export` prints, with
      its exit status, with a context active or not.
- [x] Every option in the table works in its short and long form, and our options stop at the first task word or `--`
      (`meghnaad export -f csv -home` exports with the filter `-home`).
- [x] `-r NAME` applies the report's filter, sort and limit; `-F table` gives its columns' fields; an unknown report is
      `task`'s error and exit status.
- [x] JSON keeps `export`'s values; `-a`/`-A` override `json.array`.
- [ ] CSV and TSV write dates as `YYYY-MM-DD HH:MM:SS` in UTC, date UDAs included, and a CSV opened in a spreadsheet
      shows them as dates.
- [x] The terminal app has the Export key (`e` by default) where the web app has its button, and its dialog has the same choices, order
      and defaults, makes the same file for the same choice, and saves it to the path given (asking before replacing).
- [ ] Android has the Export button where the web app has it (report tables, board sections), and its dialog makes the
      same files as the web app's for the same choice (JSON, CSV, TSV; all, table or ticked fields) and saves them.
- [x] The web Export dialog offers TSV (a `.tsv` file, `text/tab-separated-values`, escaped as the CLI's, with a
      header), and its CSV and TSV write the same dates; its JSON is unchanged.
- [ ] A description, annotation and string UDA with line breaks (and tabs) come out whole: as `export` wrote them in
      JSON, quoted in CSV (a spreadsheet shows one cell with line breaks), escaped in TSV (one task per line), and
      unescaping the TSV gives the original text back.
- [x] TSV escapes tab, newline, carriage return and backslash; `-H` drops the header; `-0` ends tasks with NUL
      (`meghnaad export -f tsv -F uuid -H -0 | xargs -0 …` works).
- [x] Bad options exit 2 with a message on standard error, and `task` is not run.
- [x] Tests: unit tests for the option parsing, TSV escaping and date formatting (`tc_core::table_export`); the web
      export tests (`exportFile`, `ExportDialog`) for TSV and the dates; Kotlin tests ported from them
      (`ExportFileTest.kt`, which covers the scope too), `testDebugUnitTest` and `assembleDebug` passing; terminal app tests
      for its dialog (choices, the file, the path and the replace question); `tests/real_task.rs` cases against a
      scratch `TASKRC`/`TASKDATA` (skipped without `task` 3.5.0) comparing with `task export` output.
- [x] Docs: `docs/terminal.md` (the `meghnaad export` command and the dialog's key), `docs/using/pages.md` (TSV, the
      dates), `docs/using/phone.md` (the Export button), `docs/architecture.md`, the README's terminal section,
      `meghnaad --help`.
- [x] Size: engine wasm measured before and after.

## Open questions

- None open.

## Implementation notes

2026-11-16: Before this spec, the terminal app's program was renamed from `meghnaad-tui` to `meghnaad` (the crate keeps
its name). The Homebrew formula becomes `meghnaad`, the tap gets `formula_renames.json` (`meghnaad-tui` → `meghnaad`)
and loses `Formula/meghnaad-tui.rb`, and the formula installs a `meghnaad-tui` link for now; release archives are
`meghnaad-<version>-<platform>.tar.gz`. Nothing of this spec is built yet.

2026-11-16: Answers to the first review: What reads `meghnaad export`; no context and no option for it (`--context`
dropped, as the Export dialog has none), so with no options it is `task export`; the report only by `-r`; the dialog's
CSV takes the same spreadsheet dates; dates in UTC.

2026-11-16: TSV comes to the Export dialog too (asked in review). The web app has the dialog; Android has none, so the
earlier "web and Android" was wrong and is corrected; whether Android gets the dialog is an open question.

2026-11-16: Multiline values (raised in review): kept quoted in CSV, escaped in TSV so a line is a task, and a criterion
for them.

2026-11-16: Android gets the Export button and dialog in this spec (asked in review), ported from the web app's with
its tests; front ends are now all.

2026-11-16: One Export dialog on every platform (asked in review): the terminal app gets it too, over its report and
board sections, saving to a path; the `meghnaad export` command stays as designed beside it.

2026-11-16: The terminal dialog's key is `e` (`tasks.export`, `boards.export`). Status set to in progress; building
starts.

2026-11-16: Built on every platform; all checks pass (format, lint, 1,027 Rust tests, the Lua hooks, 801 web tests,
svelte-check, the script tests, both wasm checks, 397 Kotlin tests and `assembleDebug`, `mdbook build docs`).

- Engine (`crates/tc-core/src/table_export.rs`): `Format::Tsv`, `to_tsv` (escapes `\t` `\n` `\r` `\\`, header and
  line end chosen), dates in CSV and TSV as `YYYY-MM-DD HH:MM:SS` UTC for the built-in date attributes and date UDAs
  (`sheet_date`, `sheet_cell`), `to_csv` takes the settings for that, and `file_name` (the engine's `export_name`, made
  `pub(crate)`) for a front end whose `export` is the real `task`'s.
- Terminal: `crates/tui/src/export.rs` (`meghnaad export`; `main.rs` sends `export` there and returns an `ExitCode`),
  `task.rs` gains `raw` and `inherit`; `app/export.rs` (the dialog, `Mode::Export`, `Mode::ExportPick` for a board),
  drawn in `ui.rs`, `?` lists its keys; board tables keep the words they ran (`boards::Data::Table`); `tui.export` in
  `settings.rs`. Keys `tasks.export` and `boards.export` (`e`) in `default.json`, copied to the phone.
- Web: TSV in `ExportDialog.svelte` and `exportFile.ts`; tests in `exportFile.test.ts` and a new `ExportDialog.test.ts`.
- Android: `ExportFile.kt`, `ExportDialog.kt` (button and dialog), `ConfigResponse.raw`, `Store.exportTasks` and the
  kept choice; the button in `TasksView.kt`, `ResultView.kt` and `BoardGrid.kt`; `ExportFileTest.kt`.
- Tests against the real `task` 3.5.0: `crates/tui/tests/export.rs` (output, messages and exit status equal to
  `task export`'s with a context active; TSV, CSV, `-0`, `-home` as a filter word); `tests/export_dialog.rs` (the
  dialog over a stand-in `task`, with its own config folder: `e`, TSV, saving, the second `Enter` to replace, the kept
  choice, a board's two tables).
- Size: engine wasm 846,370 → 847,611 bytes gzipped (+1.2 KB). The Worker was not
  measured.
- Not ticked yet, as nobody has looked: a CSV opened in a spreadsheet showing the dates as dates and a multiline value in
  one cell (both criteria otherwise tested), and the Android dialog on a device or the desktop preview (its logic is
  tested and the APK builds; no Compose UI test drives it).
