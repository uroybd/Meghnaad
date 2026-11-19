# 0022: A PIN on a trusted device (the static web app)

- **Status:** in progress
- **Created:** 2026-11-19
- **Layer:** 2 (UI)
- **Front ends:** web, the static flavour only. The Worker flavour signs in with Cloudflare Access and has no lock. The
  phone has none: its system already offers a PIN for the whole device or the app.
- **Supersedes / related:** 0018 (trust this device), 0013 (the lock), 0017 (reminders while locked), 0019 (passkey
  unlock).

## Why

A trusted device opens unlocked and never locks on its own (0018). That is the point of trusting it, and it is also why
reloading to get the latest tasks is painless there. But a trusted laptop left open on a desk shows every task to
whoever passes by. The full lock is too heavy for that: it drops the key, needs the secret or a passkey back, and a
locked tab stops being the app.

A PIN is the light lock in between: a screen over the app after some minutes without use, gone again with a few key
presses, with the app (its edits, the page you were on, sync and reminders) running on underneath. It is a loose,
mostly visual block, not a security feature: it keeps a passer-by's eyes off the tasks, nothing more.

## What

1. **Setting it.** *Settings → Device*, on a trusted device only:
   - **PIN**: press exactly four keys, then press them again to confirm. There is no Apply or OK: the PIN is saved the
     moment the second four match, and a mismatch clears both to start again. **Remove PIN** takes it off.
   - **PIN lock after this many idle minutes** (default 5; 0 locks only with **Lock with PIN now**).
   - Both are this browser's alone, kept in it as the trust is, never in `settings.json` or on the branch.
2. **What counts as a key.** PINs are for fast input, so a key is one of:
   - the four arrow keys (`ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight`);
   - a lowercase letter, `a` to `z`;
   - a digit, `0` to `9` (the number row or the keypad).

   A letter pressed with Shift or Caps Lock counts as its lowercase one, so Caps Lock never keeps you out. Every other
   key is ignored on the PIN screen, except Backspace, which takes the last key back.
3. **When it locks.** After the idle minutes with no pointer, key, wheel or touch in any tab of the app, or at once
   with **Lock with PIN now** (Settings, and the `:` command line). All open tabs lock and unlock together.
4. **The PIN screen.** A blurred overlay over the whole app, with four dots that fill as keys are pressed:
   - it checks as soon as the fourth key is pressed: there is no OK button, and Enter does nothing;
   - Backspace takes the last key back;
   - a wrong PIN shakes the dots and clears them, as often as you like;
   - **Forgot the PIN?** locks fully (the key is dropped, as **Lock now** does), and the secret or a passkey opens the
     app again; the PIN can then be set anew.

   Underneath, the app keeps running: sync, reminders (as notifications), unsaved edits. Nothing under the overlay can be
   clicked, typed into or reached with Tab, and screen readers are told only of the PIN screen.
5. **Turning trust off** removes the PIN too: a device that is not trusted has the full lock and nothing in between.

## Not doing

- Security. The PIN seals nothing, and the trusted copy (0018) still opens without it. Anyone who can open the developer
  tools, or read this browser's storage, gets past it. It is a screen in front of the app, not a key to it.
- A limit on wrong tries, or a lock-out.
- A PIN in place of the secret or a passkey on a device that is not trusted.
- A PIN synced between devices.
- The Worker flavour (it has no lock of its own).
- An on-screen key pad, and locking when the tab is hidden.
- The Android app.

## Design

TBD, a proposal:

- `web/src/engine/pin.ts`: the PIN kept in this browser (a SHA-256 of it, only so it is not lying there as typed) and
  the idle minutes; `setPin`, `clearPin`, `checkPin`, and `pinKey(e: KeyboardEvent)`, which names the key
  (`a`, `7`, `↑`) or says it is not one. The idle watch shared with `session.ts`'s (`watchIdle`), which today stops at a trusted device; the
  PIN lock and its unlock go to the other tabs over the same BroadcastChannel.
- `web/src/lib/PinLock.svelte`: the overlay, keys read on `keydown` at the window in capture, so nothing under it sees
  them; `inert` on the app's root while it shows.
- `DevicePanel.svelte`: the PIN and its minutes, shown while the device is trusted.
- A reload of a PIN-locked tab opens PIN-locked (the lock state is kept in this browser, as `meghnaad-locked` is).

## Acceptance criteria

- [x] On a trusted device a PIN of exactly four keys can be set (pressed twice, saved on the eighth key with no button),
  changed and removed in Settings → Device;
  it is not offered on a device that is not trusted, and turning trust off removes it.
- [x] The keys taken are the arrows, `a`–`z` (Shift and Caps Lock read as lowercase) and `0`–`9`, number row or keypad;
  every other key is ignored, with tests over `KeyboardEvent` values.
- [x] After the idle minutes (or Lock with PIN now) every tab shows the blurred PIN screen; the right PIN clears it in
  every tab, with the page, its edits and sync as they were.
- [x] A reload of a PIN-locked tab is still PIN-locked.
- [x] A wrong PIN clears the dots; Forgot the PIN? locks the device fully.
- [ ] Nothing under the overlay can be focused, clicked or typed into. (Keys: tested. Clicks and focus: the native modal
  dialog's, still to be checked by a person in a browser.)
- [x] Tests: `pin.ts` (keys, set, check), the overlay (keys, Backspace, wrong PIN, every tab), the device panel.
- [x] Docs: the static app's lock and trust section (it is in `docs/deploy.md`, not `docs/using/`), and `:lock` in
  `docs/commands.md` (made from `commands.json`). No address was added, so `pages.md` is unchanged.

## Open questions

None left. Decided 2026-11-19:

- **No on-screen pad.** Which keys make up the PIN is the person's choice; they pick ones easy to press on their own
  keyboard. A PIN with arrows on a device whose touch keyboard has none is theirs to avoid.
- **It locks only after the idle minutes** (or Lock with PIN now), not when the tab is hidden.
- **Not on the Android app for now.** Phones and launchers offer a PIN of their own, and this web app is the one with
  less protection around it.

## Implementation notes

2026-11-19:

- `web/src/engine/pin.ts`: `pinKey` (arrows as `↑ ↓ ← →`, `a`–`z` with capitals read as lowercase, `0`–`9`; nothing
  with Control, Alt or Meta, nor through an input method), the PIN kept as a SHA-256 in `localStorage`, its minutes
  (default 5) and the PIN-locked mark.
- `web/src/engine/session.ts`: `onPin`, `pinLock`, `pinUnlock` and `lockNow`. The tabs follow each other over the vault's
  BroadcastChannel (`pin` and `unpin` messages). The idle watch now runs on a trusted device too: there it puts up the
  PIN screen, never the full lock. The full lock clears the PIN-locked mark; turning trust off and forgetting the device
  remove the PIN.
- `web/src/lib/PinLock.svelte`, mounted by `App.svelte` in the static flavour: a `<dialog>` opened with `showModal()`.
  It sits in the top layer above any open drawer, makes the page under it inert, and is blurred with `::backdrop`. Escape
  can't close it. Keys are read at the window in the capture phase and stopped, so the app's own keys never see them.
- Asked for by the person, 2026-11-19: a reload of a trusted device with a PIN opens on the PIN screen (from the trusted
  copy, or by a reload beside other open tabs; a new tab beside open ones does not). The screen is frosted glass: the
  dialog covers the screen and blurs what is behind with `backdrop-filter`.
- `web/src/lib/PinSettings.svelte`, under *This device* while the device is trusted.
- `:lock` (action `app.lock`): with nothing after it the PIN screen where there is a PIN, else the full lock; `:lock pin`
  and `:lock key` ask for one (asked for by the person, 2026-11-19). The lock button in the top bar is always the full
  lock. It is a shared command (`tc_core::keymap::COMMANDS`, `commands.json`, `since:
  unreleased`). The static web app runs `lockNow`. The web app on a Worker and the terminal say there is nothing to lock.
  The phone only lists commands.
- Not seen in a browser: the static flavour needs a real forge branch and token, and there is no local stand-in. The
  behaviour is covered by tests (keys, set, check, every tab, reload, `:lock`, the screen and the settings), but the look
  of the blur and the native modal's blocking of clicks and focus are left for a person to check.
