# 0027: Settings of one device, over everyone's

- **Status:** in progress <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-21
- **Layer:** 2 (UI)
- **Front ends:** web, Android (not the terminal app: its `settings.json` is already the desktop's own and never reaches
  the store)
- **Supersedes / related:** 0009 (`settings.json` kept whole), 0020 (`settings.json` versioned, keys per group), 0003
  and 0017 (reminders), 0018 (trust this device: the static app already tells devices apart)

## Why

`settings.json` is one file for every device: the web app in each browser and the phone read the same one from the
store. Some settings suit one device and not another. The page the app opens on and the keys that suit a keyboard
differ between a laptop and a phone, and whether reminders pop up is a question for each device. Today, changing the
home page or a key for the phone changes it everywhere. uroybd wants a device to keep its own value for these, the
rest still coming from `settings.json`, and to see at a glance which settings are the device's own.

## What

1. **Two views of the same Settings.** Settings has a switch at the top, like a pair of tabs: **Everyone**
   (`settings.json`, as today, the default) and **This device**. The screens, tabs and fields are laid out exactly the
   same in both views. Only where a value is read from and saved to changes.
2. **What a device can have of its own.**
   - **the home page** (where the app opens), and
   - **the keys** (the key bindings), and
   - **reminders**: whether they are on, and their settings. Reminders are always the device's own. They are set
     only under This device, and under Everyone the reminders fields say "Set on each device" and cannot be
     changed there.

   Every other field shows under This device as it does under Everyone, but read only, with a line saying it is set
   under Everyone. It is there so the layout is the same, not to be changed.
3. **What This device shows.** Each field shows the value this device uses:
   - its own value, if it has one (an *override*), else
   - the value in `settings.json`, else
   - the app's default.

   A field with an override has a highlighted border around the whole field, its name and its value together, so
   the overrides stand out at a glance.
4. **Making and removing an override.** Changing the home page or a key and saving it under This device makes an
   override. An overridden field can be put back, which removes its override, so `settings.json`'s value (or the
   default) applies again.
5. **Under Everyone, a field this device overrides says so** ("This device uses its own"), with a link to it under
   This device. So a change saved there that this device won't follow doesn't seem to do nothing.
6. **The device's settings exist only once something is saved for it.** Until then, nothing is stored for the device:
   This device shows Everyone's values with nothing highlighted, and reminders are off.
   - **The first time** the person switches to This device, and the store already has some devices' settings (another
     device saved them, and this one has synced since), they choose: **use one of these** (listed by Device ID), or
     **a new one for this device**.
   - **A new one** gets its ID on the first save. The ID is a random string, shown prefilled in a field so the person
     can keep it or type their own, then save.
   - **Using an existing one** shares it: two devices on one ID have the same overrides ("my laptops"). Typing an ID
     that exists, there or later, does the same.
   - The ID stays editable under **This device → General**, as the field **Device ID**. Changing it renames the
     device's file.
7. **The file as text.** Under This device → General, the device's file can be edited whole as text, as
   `settings.json` can under Everyone (0009): **Copy**, paste, **Apply**, linted the same way.
8. **Its address.** The view is part of the page's address (`route.ts` `Place`), for example `/settings/device/keys`
   next to `/settings/keys`. So it can be linked, and the back button returns to the view you were in.

## Not doing

- **Any other setting per device.** Only the home page, the keys and reminders, for now. Boards, task commands,
  kanbans, appearance and the rest stay one for all devices. A new one is added here by a later spec, not on the way.
- **The taskrc and its tabs** (taskrc, urgency, hooks, tasksh: the Taskwarrior group). They are Taskwarrior's
  configuration, layer 1, one for all devices as `task` has it.
- **The terminal app.** Its `settings.json` lives on the desktop and is already the desktop's own.
- **Secrets.** The phone's sync settings and keys, and anything else kept out of `settings.json` today, stay where
  they are.

## Design

Proposed, for uroybd to agree.

- **The file.** A device's settings are a file in the store beside `settings.json`, `devices/<deviceId>.json`
  (`web/devices/<deviceId>.json` in a bucket, as `web/settings.json` is). It holds a partial `settings.json` with the
  same keys and shapes: `"version"`, `"deviceId"`, and only what the device overrides (`homepage`, the keys under
  `keys` / `tui`'s keys as `settings.json` has them, `reminders`). It is read through the same parser and migration
  as `settings.json` (`parseSettings`, `tc_core::settings_version::migrate`), so a shape change migrates both.
- **The merge.** What a device uses is `settings.json` with the device's file laid over it:
  - the home page, whole;
  - the keys, binding by binding: a key the device binds is its own, the rest are `settings.json`'s. Each key's row
    is the field that shows the border;
  - reminders, from the device's file only.

  One merge function in each front end is ported with its tests: web `appsettings.ts`, phone `AppSettings.kt`.
  Everything that reads settings today reads the merged result.
- **Reminders move.** Their settings (on or off, how long before) are in the browser's `localStorage` today
  (`lockedReminders.ts`, `notifier.svelte.ts`) and in the phone's preferences. They become the device file's
  `reminders`. A device with no file keeps reading the old place. The first save of a device file carries the old
  value into it, and the old place is then no longer read.
- **Which device this is.** The device remembers only its ID: in `localStorage` on the web, in the app's preferences
  on the phone. No ID means no device settings yet.
- **Routes.**
  - The Worker and the phone serve `GET /api/settings/devices` (the IDs there) and
    `GET/PUT/DELETE /api/settings/devices/<id>`, beside `/api/settings`. They go in `tc_core::webapi`, which the Worker
    and the phone share.
  - The static app reads and writes the files on its branch as it does `settings.json`.
  - The same size limit and lint apply as for `settings.json`, and secret-like names are refused. An ID is checked as
    a file name: letters, digits, `-` and `_`.
  - Renaming is a write of the new file and a delete of the old one.
- **The screens.**
  - The Settings page takes the view, Everyone or This device, from its `Place`.
  - Each settings field takes whether it is overridden (the border), whether it can be overridden at all (else read
    only under This device), and a way to put it back.
  - Save writes `settings.json` under Everyone, the device's file under This device.
  - The first switch to This device opens the choice of item 6 in a drawer, not a modal.
  - Android mirrors this in `SettingsPage.kt`.

## Acceptance criteria

- [x] Settings has the Everyone / This device switch on the web and the phone; the view is in the address and comes
      back with the back button (round-trip test in `route.test.ts`, a row in `docs/using/pages.md`)
- [x] Under This device, the home page, each key and reminders show the device's value, else `settings.json`'s, else
      the default; an overridden field has a highlighted border around its name and value; every other field is read
      only there
- [x] Saving the home page or a key under This device makes an override; putting it back removes it, and the field
      shows `settings.json`'s value again
- [ ] Reminders are set only under This device; under Everyone their fields say they are set on each device; an
      existing reminders setting carries over into the device's file on its first save
- [x] Under Everyone, a field this device overrides says so, with a link to it under This device
- [x] Nothing is stored for a device until its first save; that save shows a generated Device ID in a field the person
      can change before saving; **This device → General** has the **Device ID** field afterwards, and changing it
      renames the file
- [x] The first switch to This device, with devices' settings already in the store, offers to use one of them or make
      a new one; two devices on one ID share its overrides
- [x] The device's file can be edited whole as text under This device → General, linted as `settings.json` is
- [x] The app uses the merged settings everywhere: an override changes this device only, and another device still
      follows `settings.json`
- [x] Tests: the merge, the device file's parse, lint and migration, and the reminders carried over (web unit tests,
      ported to Kotlin); the routes' read, write, list, rename and refusals (`tc_core::webapi`); a settings page test
      for the border, read-only fields and putting a field back
- [x] Docs: `docs/using/settings.md` (the two views, what a device can have, Device ID), `docs/architecture.md` (the
      device file and its routes, in the DATA list too), `docs/using/pages.md` (the address), the reminders guide

## Open questions

- **Reminders under Everyone.** Neither front end has reminders in Settings: they are the bell's dialog (web) and Menu →
  Reminders (phone), which were already each device's own. So there is no reminders field under Everyone to say "Set on
  each device", and that half of the fourth criterion is not met as written. Either the criterion drops it (the dialogs
  are the device's own by nature), or a Reminders row is added to Settings for it. uroybd to say which.

## Implementation notes

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.

- **2026-11-21.** Built for the web and the phone.
  - Routes in `tc_core::webapi` (`DEVICES_PREFIX`, `device_id_problem`, `device_key`; list, get, put with `?from=`
    rename as one `write_many`, delete), tested in `crates/tc-core/tests/device_settings.rs`. The Worker routes them
    through `config_route`, which now passes the query on. The static app reaches them through the branch's
    `WebSettings` as before. Checked on an isolated `wrangler dev`: list, write, read, rename, a bad ID refused, delete.
  - The phone shares `web/devices/` with the bucket (`filesync.rs` `SETTINGS`), a rename or delete going across as a
    taskrc file's does; tested.
  - Web: `deviceSettings.ts` (merge, key diff, home page, reminders, IDs), `deviceAsk.svelte.ts` + `DeviceDrawer.svelte`
    (the ID on the first save, the choice on the first switch, with **Delete** per device: the open question, answered
    "Good"), the store's `deviceId` / `deviceRaw` / `effectiveSettings` (keys and home page read from it), the view in
    `Place` (`/settings/device[/<tab>]`), the switch and read-only tabs (`<fieldset disabled>`) in `SettingsPage.svelte`,
    General (home page, Device ID, delete, the file as text) and Keybindings (rows framed, put back, the note under
    Everyone). Reminders: `notifier.save` also writes them into the device's file once it has one, and the app adopts
    the file's.
  - Phone: `DeviceSettings.kt` (the port, with `DeviceSettingsTest`), `DevicePrompt.kt`, the store, `Route.kt`,
    `SettingsPage.kt` (segmented switch; other tabs dimmed and taking no touch), `GeneralPanel.kt`, `KeysPanel.kt`,
    `Reminders.kt` (`adopt`, and saving into the device's file).
  - Sizes: the browser engine is unchanged (907,088 bytes gzipped, as measured here with names); the Worker
    774,591 → 778,743 (+4.1 KB).
  - Left: the reminders half of the fourth criterion (see Open questions).

