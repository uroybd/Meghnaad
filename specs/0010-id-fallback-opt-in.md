# 0010: Short uuid for a missing ID, as an option

- **Status:** done <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-17
- **Layer:** 2 (UI), with a choice passed to a shared helper
- **Front ends:** all (web, Android, terminal)
- **Supersedes / related:** 0009 (`settings.json` kept whole)

## Why

A completed or deleted task has no ID. The real `task` shows `-` in its ID column; we show the first eight characters
of its uuid instead, dimmed, on every front end (`tc_core::cells`, the `id` cell). That is handy for some and noise for
others, and it isn't what `task` prints. It should be your choice, and off unless you turn it on.

## What

1. **A setting.** **Settings → Appearance** gets *Fall back to UUID.short when ID is missing (e.g. completed or
   deleted)*, off by default, kept in `settings.json`'s `appearance` so it is the same on every front end. The phone has
   it on its Appearance panel, the terminal app on its Settings page.
2. **Off (the default).** A task with no ID shows `-` in the ID column, as `task all` does:

   ```
   ID St UUID     Description
   -- -- -------- -----------
    - C  43082644 one
   ```

3. **On.** It shows the short uuid, dimmed, as today.
4. **Either way**, the cell still carries the task's uuid: the tooltip (the web) and copying the cell show the full
   uuid, as before.

## Not doing

- Changing the `uuid` column, or how other places (task info, dependencies, links) show a uuid.
- A taskrc setting: this is the app's, not Taskwarrior's.

## Design

Agreed 2026-11-17.

- **The engine.** `tc_core::cells::Ctx` gains `id_fallback: bool` (serde default `false`). The `id` cell with no ID is
  `Cell::text("-")` when it is off and the short uuid, dimmed, when it is on; `uuid` is set either way, so the tooltip
  and copying are unchanged. Nothing else in the engine changes.
- **The setting.** `appearance.idFallback`, parsed with the rest of `appearance` (`parseAppearance` on the web and
  Android, the terminal's settings), saved through 0009's overlay so the rest of the file stays.
- **Passing it.** The web's `formatTable` puts `id_fallback` in the context it sends to `format_cells`; Android's
  `formatCells` the same; the terminal's `model::ctx` from its settings.
- **Where it is set.** Web: a checkbox on the Appearance tab, *Fall back to UUID.short when ID is missing (e.g.
  completed or deleted)*. Android: the same on `AppearancePanel.kt`. Terminal: a row in General (beside the progress
  bars), `Space` or `Enter` toggling it and saving.
- **Tests.** The cell both ways (`tc-core`); the setting's parse and save on each front end; the web table showing `-`
  and the uuid in its title.
- **Order.** Built after 0009, so its save goes through the overlay.

## Acceptance criteria

- [x] With the setting off (and absent), a done or deleted task's ID cell is `-` on every front end; on, it is the
      short uuid, dimmed.
- [x] The cell's uuid (tooltip, copy) is unchanged either way.
- [x] The setting is on the Appearance tab (web), the Appearance panel (phone) and the Settings page (terminal), and
      saving it keeps the rest of `settings.json`.
- [x] Tests: the cell both ways in `tc-core`; the setting's parse and save on each front end.
- [x] Docs: `docs/using/settings.md` (Appearance), `docs/terminal.md`.
- [x] Size: engine wasm measured before and after.

## Open questions

None left.

Decided (2026-11-17, by the person):

- The key is `appearance.idFallback`, `true` or `false` (absent is `false`); other choices would not be useful.
- Off by default for everyone, existing files included: after the update a task with no ID shows `-` until the
  setting is turned on.

## Implementation notes

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.

2026-11-17: built, after 0009.

- **Engine.** `tc_core::cells::Ctx.id_fallback`: with no ID the cell is `-`, or the short uuid dimmed when it is on, and
  it carries `uuid` either way. Tested both ways in `cells.rs` (`identity_and_state`).
- **Web.**
  - `Appearance.idFallback` is parsed in `parseAppearance`; a value that is not a boolean is an appearance error, so a
    lint warning.
  - It is saved by `serializeAppearance` through 0009's merge: written when on, and taken out when turned off if it was
    understood.
  - `formatTable` sends `id_fallback`; `ReportTable` passes it.
  - The checkbox is under a new **IDs** heading at the top of the Appearance tab.
  - Tests: `appsettings.test.ts` (parse, save on and off) and `ReportTable.test.ts` (`-` with the uuid in the tooltip,
    and the short uuid dimmed).
- **Android.** The same in `AppSettings.kt`, `Format.kt` (`Ctx.idFallback` → `id_fallback`), `Cells.kt` and
  `AppearancePanel.kt`. Tests are in `FormatTest.kt` and `AppSettingsTest.kt`.
- **Terminal.**
  - `model::Look { rules, id_fallback }` replaces the bare `rules` in `model::ctx` and `model::report`.
  - `AppSettings::id_fallback` and `id_fallback_error` read the setting, and `with_id_fallback`/`save_id_fallback` edit
    `appearance` in place.
  - General has a second row (`general_sel`): `j`/`k` go through the two rows before moving on, and `Space`/`Enter`
    (`settings.general.home`, now described as either) toggles it on that row.
  - The default keymap's description changed, and the phone's copy was regenerated.
- **Size.** Engine wasm went from 858,857 to 859,017 bytes gzipped (+160).
