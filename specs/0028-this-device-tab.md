# 0028: This device as one tab of Settings

- **Status:** done <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-21
- **Layer:** 2 (UI)
- **Front ends:** web, Android (not the terminal app, as in 0027)
- **Supersedes / related:** supersedes 0027's switch (What 1, 2's read-only fields, 3's framing on every tab, 8's
  addresses); the rest of 0027 (what a device can have, its file and ID, the first-time offer) stands

## Why

0027 showed the whole of Settings twice, under a switch: **Everyone** and **This device**, laid out the same, with all
but a few fields read only under This device. A device has only a few things of its own, so the switch was a detour:
looking through every tab for what can be changed per device. uroybd asked for a section of its own instead (2026-11-21).

## What

1. **No switch.** Every tab of Settings is everyone's `settings.json`.
2. **A tab, This device,** in the Meghnaad group after General, with only what a device can have of its own: its Device
   ID (change, delete its settings), its home page (its own, or everyone's again), its reminders (always its own: a
   line saying how they are set and a button opening them), its keys, and its file as text. On the static web app, its
   lock and trust settings (0018, 0019, 0022) are there too.
3. **Its keys:** only those it has of its own, each with **use everyone's**; **Add** in each section gives it one with
   the usual picker. The full Keybindings tab stays everyone's.
4. **Elsewhere,** a home page or key this device has its own of says **This device uses its own**, linking to the tab.
5. **Its address** is the tab's: `/settings/device` (`meghnaad://settings/device`); the old
   `/settings/device/<tab>` opens it.

## Not doing

- Anything new a device can have of its own (0027's Not doing stands).

## Design

The General and Keys panels take a `device` flag (web: a prop; Android: `GeneralState(device = true)`,
`KeysState(device = true)`) in place of the store's switch; the tab (`ThisDevicePanel.svelte`, `ThisDevicePanel` in
`SettingsPage.kt`) draws them in device mode, with the reminders between. `store.settingsDevice` and `Place.device` are
gone; `device` is one of the settings tabs.

## Acceptance criteria

- [x] No Everyone / This device switch; no read-only tab (web and Android).
- [x] This device shows the ID, home page, reminders, its own keys (with Add and use everyone's) and the file.
- [x] Links from Everyone ("This device uses its own") open the tab; the old addresses open it.
- [x] Route round-trip tests; docs (`docs/using/settings.md`, `pages.md`, `deploy.md`) updated.

## Open questions

(none)

## Implementation notes

2026-11-21. Built on the web and Android as designed. Checked on the emulator: the tab draws, and its key sections say
"None of its own" with Add. Found on the way (Android): a tab not opened yet could show the not-applied dot, because its
draft only followed the saved settings while it was shown; the settings page now has every tab follow them.
