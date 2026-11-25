# 0033: The terminal app's Settings as pages

- **Status:** in progress
- **Created:** 2026-11-26
- **Layer:** 2 (UI)
- **Front ends:** terminal; web for one switch (AI agents) that saved at once; the phone checked, already as the map
- **Supersedes / related:** `docs/settings-map.md` (the map these pages follow: pages, parts, controls, what the terminal leaves out); the Settings page of `docs/terminal.md`; Nerd Font (`tui.nerdFont`, 0.8.60); spec 0032 (the
  task command form); the web's `SettingsPage.svelte` and the phone's `SettingsPage.kt` (tabs, a page each).

## Why

The terminal's Settings page stacks seven sections on one screen, each held to a few rows (Task commands to ten, Boards
to six), so a long list scrolls inside a small box while the rest of the screen shows sections you aren't using. The
left pane shows the reports, which mean nothing on this page. The web and the phone give each part of the settings its
own page, as tall as it needs. Nerd Font is under General, though it is about how the app looks.

## What

The pages, their parts and their controls are those of [the settings map](../docs/settings-map.md); where this spec
and the map differ, the map wins and this spec is corrected.

The terminal's Settings follow the web app's Meghnaad tabs (`SettingsPage.svelte` and its panels): the same pages, in the
same order, with the same parts on each, the same kind of control for each setting, and the same few words under each
saying what it does.

### The section list

- On the Settings page the left pane is the **list of pages** instead of the reports, titled Settings: General, This
  device, Appearance, Keybindings, Kanban, Commands (the web's Meghnaad tabs and names), then Sync (the phone has it
  too). No Taskwarrior group: the terminal app has no taskrc, urgency, hooks or tasksh settings of its own (`task` is
  its backend; the taskrc is edited as it always is). No Boards page: boards have no setting but the list itself, and
  the Boards page lists them. With Nerd Font on, each page in the list has its icon before its name, as the pages in
  the top bar do.
- `j` `k` in the list move between pages and the main pane shows the one chosen at once; `l`, `Enter` or `Tab` give the
  main pane the focus, `h` or `Esc` give it back; `[` `]` go to the previous and next page from the main pane. A page
  with edits not yet applied has a dot after its name, as a web tab does.

### Each page

A page fills the main pane top to bottom and scrolls when it is taller than the screen, keeping the row chosen in view.
It is laid out as the web panel is: a heading for each part (the web's `h2`/`h3`), under it its controls, one a row, and
under each a faint line or two of what it does (the web's `dim` text, shortened), then its problems in the error colour.

The web's controls, drawn in the terminal:

| Web | Terminal |
| --- | --- |
| a checkbox (`label.pick`) | `[x] Card icons` / `[ ] Card icons`; `Space` or `Enter` switches it |
| a `select`, with groups (`optgroup`) | the value and a `▾`; `Enter` opens a picker of the choices under their group headings (Reports, Boards, Kanbans), typing narrows it |
| a text or number input | `Label  [value     ]`; `Enter` edits it in place on the row, `Enter` keeps it, `Esc` drops it |
| a list of rows (commands, columns, transformations) | a row each, its fields in columns as the web lays them out; `a` adds one, `e`/`Enter` changes it, `D` removes it, `K` `J` move it where the web has up and down |
| a row's form of several fields | a **modal** with every field at once (below) |
| Apply, Revert edits | as on the web and the phone: edits wait until applied. `Ctrl+S` applies the page (the border says `Ctrl+S apply` while there are edits, then `Saved`), `u` reverts its edits; leaving a page keeps them, as a tab does. An action (Turn Kanban off, Write them, Sync now) happens at once |
| the `settings.json` box under General | the file's path, its problems as the web lints them, and `E` to edit it in `$VISUAL`/`$EDITOR` |

The pages, part by part as on the web:

- **General:** Home page (a select, grouped: next, Reports, Boards, Kanbans; "not found" for one that is gone), with the
  web's line on what it is; then `settings.json` (path, problems, `E`).
- **This device:** a page, as on the web and the phone, holding what the terminal has of it: Reminders (the reminder
  options, `tui.reminders`, as `N` opens them). Device ID and its own home page are not here: the terminal's
  `settings.json` is the machine's own already (the map says so).
- **Appearance:** Theme (the terminal's own: a select of `auto`, `light`, `dark`, `mono`, with its swatches, and
  `[x] Shading` for `task`'s), then the web's parts: Cards (`[x] Card icons`, and `[x] Nerd Font`, the terminal's: every
  icon the app draws comes from the Nerd Font: card fields, the pages in the top bar), IDs (`[x] Short uuid when there is
  no ID`), Progress bars (a row per numeric UDA: `[x] estimate  min [0]  max [10]`), Transformations (a row each: match,
  replace, fields, condition; `a` adds one in the modal).
- **Keybindings:** a search box at the top and `[x] Show default keys`, then a heading per place (General `keys`, Web,
  Mobile, Terminal `tui.keys`) with its table of keys and actions, `a` adding a key to the heading's place and `D`
  taking one out, then Commands (the `:` words), as the web shows them.
- **Kanban:** off: the UDA (an input), Automatic (`[x] Draw the kanbans on a kanban report`, then Set up automatically)
  and Manual (Turn on), as the web; on: Kanban's settings (report, filter, completed since: inputs and selects), Columns
  (a row each: name, filter, move), then each kanban under its own heading (name, address, `[x] Its own …` for each
  setting it may override, its columns), Add a kanban, and the UDA lines with Write them.
- **Commands:** the web's one: a row each (name, label, command, icon), `a` adds one; then Aliases (the taskrc's, a row
  each with its icon), and the line on names (spec 0032).
- **Sync:** when to sync, a choice, with its line.

### Modals

A form of several fields opens as a box over the page, its fields laid out as the web's row is (a label over or before
each input): a task command (name, label, command, icon), a kanban column (name, filter, move), a transformation (match,
replace, fields, condition). Every field is checked as it is typed and its problem shown under it (the name rule of spec
0032, the line split, the regex); `Tab` and `Shift+Tab` move between fields; `Enter` on the last field, or `Ctrl+S`,
keeps the row in the page's edits (applied with the page, as on the web); `Esc` drops it. A field that completes today
(a command line, a filter, a move) keeps its completion. A one-value setting stays a picker or an inline input.

## Design

Layer 2 only: nothing in `tc-core` changes but a settings version step for renamed keys (below). The file's shape stays.

**Pages and the list (`crates/tui`).** `SettingsSection` becomes `SettingsPage` with the map's pages in its order:
General, ThisDevice, Appearance, Keys, Kanban, Commands, Sync (Theme and Boards go). On `Page::Settings`, `ui.rs`
`sidebar` draws the pages instead of the reports (a dot for one with edits, its Nerd Font glyph when on), and
`Focus::Sidebar` moves through them; the main pane draws the page chosen.

**One way to draw a page.** Each page is a function `fn rows(app) -> Vec<Row>`, from a small set of parts that are the
map's controls: `Heading`, `Hint` (faint), `Problem`, `Switch { label, on }`, `Choice { label, value }`,
`Input { label, value }`, `Item { cells }` (a row of a list), `Action { label }`. One drawer lays any page out at full
height and scrolls it to keep the chosen row in view; one key handler acts on the chosen row by its kind (`Space`/`Enter`
switches, `Enter` opens a choice's picker or edits an input in place, `a` `e` `D` `K` `J` on a list's rows). A page only
says what its rows are and what each changes, so the pages can't drift apart in how they look or behave.

**Drafts and Apply.** `App` holds a draft per page: the values it shows, starting as saved (`SettingsDrafts`: general's
home page; appearance's theme, shading, card icons, Nerd Font, ID fallback, progress bars and transformations; this
device's reminders; the keys; kanban's settings; the commands and alias icons; sync's mode). Every control changes the
draft only. A page has edits when its draft differs from what is saved. `Ctrl+S` applies the page: its keys are written
through `settings::save` with the existing `with_*` functions, which merge the edit into the file as it is now and keep
every other key (the three-way merge the web and the phone share, `fixtures/settings-kept.json`), then the file is read
again and that page's draft starts over; other pages keep theirs. `u` reverts the page's draft. An action (Write them,
Turn Kanban off, Sync now) runs at once, as on the web. Quitting with edits asks: apply them, drop them, or stay.

**Modals.** `Mode::Form { title, fields, at, target }`, each field an input with its check (a function giving its
problem as typed) and, for a command line, a filter or a move, the completion it has today. It replaces the chained
`PromptKind::CommandName` / `CommandLine` / `CommandLabel` and the kanban column prompts, and is new for a
transformation. `Tab` / `Shift+Tab` move between fields, `Enter` on the last or `Ctrl+S` keeps the row in the page's
draft (applied with the page), `Esc` drops it. The modal is drawn over the page, the fields one under another, labelled
as the web's row is.

**Keys.** The keymap asset's per-section contexts become per-page (`settings.general`, `settings.device`,
`settings.appearance`, `settings.keys`, `settings.kanban`, `settings.commands`, `settings.sync`), with the controls'
actions once in `settings` (`settings.apply` `Ctrl+S`, `settings.revert` `u`, `settings.activate` `Space`/`Enter`,
`settings.add` `a`, `settings.edit` `e`, `settings.delete` `D`, `settings.raise` `K`, `settings.lower` `J`) and a
`form` context for the modal. The per-section actions they replace (`settings.general.home`, `settings.theme.preset`,
`settings.commands.add`, …) go; what is a page's own stays (`settings.keys.search`, `settings.kanban.open`,
`settings.kanban.write`, `settings.kanban.off`, …). Settings version 5 (`tc_core::settings_version`) renames a person's
own bindings of the removed actions to the new ones, so no key is lost.

**Web.** `ThisDevicePanel.svelte`: the AI agents switch joins the page's draft and is saved by its Apply (`web.agents`
in the device's file), instead of saving as it is switched.

**Phone.** Nothing to change found: every tab's settings wait for Apply. Checked again while building.

## Acceptance criteria

- [x] The left pane on Settings lists the map's pages for the terminal; the main pane shows the one chosen, full height.
- [x] Edits wait for `Ctrl+S` on every page, with the dot and Revert as on the web; actions happen at once.
- [x] The web's AI agents switch waits for This device's Apply like the rest of the page.
- [ ] Each page has the web panel's parts in its order, each setting drawn with the control above for the web's control.
- [x] Appearance has the theme, Cards (with Nerd Font), IDs, progress bars and transformations; General the home page and the file.
- [x] A task command, a kanban column and a transformation are added and changed in a modal, checked as typed.
- [x] A long section (many task commands, keys, kanban rows) scrolls within its page and the selection stays in view.
- [x] Tests: `crates/tui` tests of the section list, the full-height drawing, the form (fields, checks, save, cancel).
- [x] Docs: `docs/terminal.md` (The Settings page), the keymap asset and `commands.json` if keys or `:` words change.

## Open questions

- Later, its own spec: one shared routine for applying settings (the edit merged into the file as read, the save, then
  the sync), in `tc_core`, which every front end calls, so applying, syncing and two devices saving at once behave the
  same everywhere. The merge is already one set of cases (`fixtures/settings-kept.json`); the routine is not.

## Implementation notes

- 2026-11-26: draft written from uroybd's request; revised to follow the web's pages (organisation, controls, layout)
  and `docs/settings-map.md`. uroybd's answers: Apply as the web on every front end; This device stays a page; Nerd
  Font draws the page list's icons too; no Boards page. Design proposed; waiting for a yes.
- 2026-11-26: uroybd said "Go ahead." Built:
  - **Terminal** (`crates/tui`): `SettingsPage` and the drafts in `app/settings_page.rs` (a `Draft` per page: the file as
    the page began and as it has it; `write_settings` edits the open page's draft on Settings, the file elsewhere;
    `act_on_settings` for the actions that happen at once; `apply` is `settings::apply`, a three-way lay-over of what
    the page changed onto the file as it is now). `settingsui.rs`: `Act`, `Row` and each page's rows, one drawer for any
    page (full height, wrapped hints, scrolled to the row chosen) and the side list (dots, Nerd Font glyphs).
    `app/settings_form.rs`: the modal (`Mode::SettingsForm`) for a task command, a kanban column and a transformation,
    each field checked as typed, completion in a command line or a filter. `Mode::Choose` is the picker for the theme
    and when to sync, `Mode::QuitAsk` the question on quitting with edits. The kanban, keys and reminders saves go
    through the draft; the chained `CommandName`/`CommandLine`/`CommandLabel` and column prompts are gone.
  - **Keys**: `settings.apply` (`ctrl+s`), `settings.revert` (`u`), `settings.activate` (`space`, `enter`) in the
    `settings` group; per-page contexts; `settings.appearance.add`/`.delete` (`a`, `D`) for transformations;
    `settings.boards.open` gone. `:` commands `apply`, `revert`, `rule add|delete` (`commands.json`, `since:
    unreleased`); `:open` uses the row chosen. Settings version 5 drops the keys bound in the `settings.boards` group.
  - **Web**: AI agents in `ThisDevicePanel.svelte` has its own Apply and counts as the page's edits
    (`ThisDevicePanel.test.ts`). **Phone**: nothing changed; its asset copies and version-pinned test follow.
  - **Departures from the Design**: the per-section actions stay rather than one generic `settings.add`/`edit`/
    `delete`/`raise`/`lower` (each page's own keys already were those, and the `:` commands name them);
    `settings.general.home`, `settings.sync.change`, `settings.theme.preset` and `settings.theme.shading` stay as
    actions for `:home`, `:syncwhen`, `:theme`, `:shading`, without the `space`/`enter` that `settings.activate` now has
    (`y` and `s` kept); `settings.commands.edit` and `settings.kanban.edit` lost `enter` for the same reason. Version 5
    drops the boards keys rather than renaming them (nothing replaces them). An input edits on the bottom line, not in
    place on the row; a choice's picker names each choice's group (`report  list`) rather than headings, and typing does
    not narrow it; the modal moves between fields with `↑` `↓` and `Tab` completes (or goes on where nothing completes).
    `docs/settings-map.md` says so where the terminal differs, and also that its Aliases have no icon and its
    transformations take their fields as words and do not move. On Settings, `u` reverts and shadows undo (the asset
    test allows that one shadow).
  - **Checked**: `npm run format`, `npm run lint`, `cargo test` (with `MEGHNAAD_CONFIG_DIR` on a scratch folder),
    `npm --prefix web test`, `npm run check`, `node --test scripts/*.test.mjs`, both wasm checks, `mdbook build docs`, the
    Kotlin tests and `assembleDebug`; the app driven in tmux on scratch data. `crates/tui/tests/settings_pages.rs` covers
    the drafts (shown on their page only, `u`, `Ctrl+S` over a file changed meanwhile, the quit question). The tests that
    need `task` 3.5.0 returned early here: this machine has 3.6.0.
  - **Left**: the criterion on the controls stays open on the three departures above that differ from **What** (the
    input in place, the picker that narrows under headings, `Tab` between a form's fields): uroybd to choose between
    building them and changing **What**. The screenshots in `docs/screenshots/terminal-keybindings*.png` still show the
    old stacked sections.
