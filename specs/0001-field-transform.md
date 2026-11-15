# 0001: Field Transformation

- **Status:** in progress <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-16
- **Layer:**  2 (UI)
- **Front ends:** all (web, Android, TUI)
- **Supersedes / related:** none

## Why

We are trying to add a little more interactivity in the UI. This is feature for powerusers.

## What
1. A new section named `Transformations` in Settings > Appearance.
2. Every entry will have a regex matcher, and a regex replacer, and fields where we will run them, uda included.
3. Transformed text will be run in the markdown renderer before rendering as field so that link, highlight, bold etc works.
4. Only text fields are supported.

### An example

Let's say we will match JIRA ticket like this `BOR-23434` and create link like `[BOR-23434](https://jira.host.com/boards/BOR-23434)`

## Not doing

N/A

## Taskwarrior parity

This is a purely frontend stuff which may have helper in `tc-core` (your decision) and it is a strictly frontend-level transformation

## Design

_Agreed 2026-11-16._

**Stored** in `settings.json` (`web/settings.json` in the bucket, the folder on the phone, `~/.config/meghnaad` for the
TUI), next to `progress`. No new route: it travels with the settings as `appearance` already does.

```json
"appearance": {
  "transformations": [
    { "match": "\\b(BOR-\\d+)\\b", "replace": "[$1](https://jira.host.com/boards/$1)", "fields": ["description", "annotations", "jira"] }
  ]
}
```

- An ordered list; each rule runs on the output of the one before it.
- `fields` is required and lists names (whitelist): `description`, `annotations`, or the name of a string UDA. There is no
  wildcard. A field that no rule names is never run through a regex, so it costs nothing.
- `match` uses Rust `regex` syntax (the crate `tc-core` already compiles in, so every front end matches the same way);
  `replace` takes `$1` / `${name}`. A rule with a bad regex, an empty `fields` or an unknown field is reported in
  Settings (one line each, like `appearanceErrors`) and skipped; the rest still apply.

**Engine (`tc-core`, display only):** a new pure module `transform` (compile the rules once per call, then
`apply(field, text)`), reached three ways:

1. `cells::Ctx` gets an optional `transformations`. In `format_cells`, a whitelisted description, annotation or string
   UDA cell is transformed and then parsed as Markdown into `blocks` (a description cell gets `blocks` only when the result
   isn't plain, so untouched descriptions look as they do now). `text` keeps the original value, so editing and copying
   are unaffected.
2. The `markdown` helper takes an optional `[field, …]` beside the texts and the rules, for the task detail view
   (`TaskInfo` on the web, `TaskInfo.kt`).
3. `transform_check(rules)` returns the problems as lines, used by the settings editors.

The stored task never changes: commands, `export`, filters and sync see the original text.

**Web:** a `Transformations` section in `AppearancePanel.svelte`: a list of rules (match, replace, field checkboxes
drawn from `description`, `annotations` and the taskrc's string UDAs), add / remove / reorder, and errors shown inline.
`appsettings.ts` parses and keeps it; the detail view draws the description with `MarkdownNodes` when it has a tree.

**Android:** the same section in `AppearancePanel.kt`, parsing in `AppSettings.kt` (ported with its tests);
`Markdown.kt` already draws the trees.

**TUI:** reads `appearance.transformations` from its `settings.json` and passes it to `format_cells` / `markdown`. It draws
the inline parts of a tree (link, bold, italic, code) in report rows and the info view; a link is shown by its text and
written as an OSC 8 terminal hyperlink. No editor in the TUI (see open questions).

**Checkboxes:** ticking a box in a transformed field uses the line number from the transformed text. If a rule changes
the number of lines, ticking is turned off for that field rather than flipping the wrong line.

**Size:** the browser engine grows only by the new module (regex is already in). The Worker is not touched.

## Acceptance criteria

Each one checkable, by a test or by looking. The change is done when every box is ticked.

- [x] The JIRA example rule links `BOR-23434` in the description, in an annotation and in a whitelisted string UDA, in
      report tables and in the detail view, on web and Android.
- [x] A field the rule does not list is not transformed.
- [x] A bad regex is reported in Settings and does not stop the other rules.
- [x] `export` and editing show the original text.
- [ ] The TUI reads the rules from its `settings.json` and draws the links in report rows and the info view.
- [x] Tests: `tc-core` unit tests for `transform` and `format_cells`; web tests for `appsettings` and the Appearance panel;
      Kotlin `AppSettings` port tests; a TUI rendering test.
- [x] Docs: the `docs/using/` appearance guide, `docs/architecture.md` (the new module).
- [x] Size: engine wasm measured before and after (the Worker is not touched).

## Open questions

- TUI editor: the TUI only reads the rules from `settings.json` (agreed with the design); an editor there is not built.
- TUI info view: it is `task <uuid> info` as `task` prints it, not built from fields, so the rules don't reach it (see the
  notes). Drop "and the info view" from the TUI criterion, or build an info view from `export`?
- A per-rule `name` or an on/off switch: not proposed; add later if wanted.
- A live preview (sample text → result) in the editor: not proposed.

## Implementation notes

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.

**2026-11-16.** Built on all three front ends.

- Engine: `crates/tc-core/src/transform.rs` (`Rule`, `Rules::new/apply/tree`, `check`), on `regex::bytes` with Unicode
  off as `rx.rs` reads patterns. `cells::Ctx` takes `transformations`, compiled once per `format`; the description cell
  (`description` and `description.desc`), its notes and string UDA cells get `blocks` from `Rules::tree`. The `markdown`
  helper takes optional fields and rules; `transform_check` is new. The unused `cells::markup` went.
- Web: `appsettings.ts` (`Transformation`, `parseTransformations`, shape only), `markdown.ts` (`trees` with fields and
  rules, `transformProblems`), `format.ts` / `ReportTable.svelte` pass the rules, `Description.svelte` draws a
  description in `TaskInfo` and the detail drawer's title, and the editor is in `AppearancePanel.svelte`.
- Android: the same in `AppSettings.kt`, `Format.kt`, `Cells.kt`, `Markdown.kt` (`DescriptionText`), `TaskInfo.kt`,
  `DetailScreen.kt`, `TaskCard.kt` and `AppearancePanel.kt`.
- TUI: `settings.rs` (`transformations()`), `model::report`/`ctx` take the rules, the app rebuilds the report when they
  change, `md.rs` draws a tree as lines and turns links into OSC 8 hyperlinks after the frame is drawn
  (`CellDiffOption::ForcedWidth(1)` per cell, so the layout holds; an address with a control character is not linked).
  The Settings page shows the count and the problems. A terminal that does not know OSC 8 ignores it, so no fallback.
- Departure: the TUI's info view is `task info` verbatim, so it is not transformed; that criterion is left open.
- Checkboxes: where a rule changes the number of lines, `line` is taken off the tree, so a box can't flip the wrong line.
- Size: engine 840,510 → 846,323 bytes gzipped (+5.8 KB; baseline includes the uncommitted export work). The Worker does
  not call `cells` or the `markdown` helper; not measured.
- Checks: `cargo test` (1014), `lua_hooks`, web (798), svelte-check, scripts, both wasm checks, Kotlin `testDebugUnitTest`
  (394) and `assembleDebug`, lint and `mdbook build docs` clean. Locally the Kotlin tests needed JDK 21 and an unstripped
  dylib (`CARGO_PROFILE_ANDROID_STRIP=false`): the stripped one fails to load on this macOS ("mis-aligned LINKEDIT").
