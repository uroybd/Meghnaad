# 0018: Trust this device (the static web app)

- **Status:** done <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-19
- **Layer:** 2 (UI)
- **Front ends:** web, the static flavour only (the Worker flavour signs in with Cloudflare Access and has no lock; the
  phone keeps its key in the Keystore already)
- **Supersedes / related:** 0013 (the web app as static files: the sealed access and the lock), 0017 (reminders while
  locked, for devices that are not trusted)

## Why

The static web app asks for the encryption secret on every visit, and locks after a spell without use. That is right on
a shared or borrowed computer and a nuisance on one's own: every new tab, reload or browser restart is a password, and a
locked tab gives no reminders. On a device you trust you should be able to say so once, and have the app open unlocked
from then on, there and nowhere else.

## What

1. **Choosing it.** The unlock form (and the connect form) gets *Trust this device: stay unlocked here, without the
   password*. *Settings → Device* has the same switch, to turn it on or off later. It is this browser's choice alone:
   kept in this browser, never in `settings.json` or on the branch, so no other device follows it.
2. **On a trusted device.**
   - A new tab, a reload or a browser restart opens unlocked, with no password.
   - The idle lock does not apply (*Lock after this many idle minutes* is shown as off while the device is trusted).
   - Reminders work whenever a tab is open, as on the Worker flavour.
3. **Lock now** locks, as today: the key and the token leave every open tab. The device stays trusted, but stays locked
   (a reload too) until it is unlocked, with the passkey when one is set up (0019), else the password.
4. **Turning it off** (the switch in *Settings → Device*) removes what was kept for it; the next visit asks for the
   password again, and the idle lock applies again.
5. **Forget this device** removes the trust with everything else it removes today.

## Not doing

- Syncing the choice, or a list of trusted devices anywhere.
- Trusting for a time (*for 30 days*), or trust that ends by itself.
- Protecting what is kept from someone who can use this browser profile: they can open the tasks. That is what trusting
  means, and the switch says so.
- Any change to the Worker flavour, the phone or the terminal app.

## Design

Agreed 2026-11-19.

- **What is kept.** The engine encrypts in Rust and needs the key's 32 bytes, and the app needs the token, so both have
  to be kept between visits. They are kept sealed with a **device key**: an AES-GCM `CryptoKey` made in this browser
  with `extractable: false`, stored in IndexedDB. Page code can seal and open with it but never read its bytes out,
  which keeps the raw key and token out of anything a script could copy away. It is not protection at rest: the
  standard lets a browser keep such a key in its profile files unencrypted, so a copy of the profile may open the
  tasks, and a script running on the page can still use the key. Trusting a device trusts its browser profile. Kept
  beside the sealed access in the `meghnaad-engine` database (`web/src/engine/vault.ts`): the device key
  and the sealed `Unlocked` (access and key).
- **Opening.** `session.ts` (`openDevice`) tries, in order: another open tab (as today), the trusted copy (open it with
  the device key, then `engine.unlockWith`) unless the device was locked with *Lock now*, then the unlock form. A copy
  that does not open (the browser cleared the device key) is removed and the form shown.
- **Lock now** sets a mark in `localStorage` (`meghnaad-locked`), which any unlock clears.
- **The switch's warning:** *Anyone who can use this browser can open your tasks.*
- **The idle lock.** `watchIdle` skips locking while the device is trusted; `DevicePanel.svelte` shows the minutes as off
  with a line saying why.
- **Turning it off and forgetting.** Turning it off deletes the device key and the trusted copy. `forgetDevice` already
  deletes the whole database, which takes them too.
- **No change to `tc-core`**, so neither WebAssembly grows.

## Acceptance criteria

- [x] *Trust this device* on the unlock and connect forms, and the switch in *Settings → Device*; neither is in
      `settings.json` or on the branch.
- [x] On a trusted device, a reload, a new tab and a browser restart open unlocked with no password.
- [x] On a trusted device the idle lock does not lock, and Settings shows it as off.
- [x] *Lock now* locks every tab, a reload stays locked, and the device stays trusted; it unlocks with the passkey or
      the password.
- [x] Turning trust off removes what was kept; the next visit asks for the password.
- [x] *Forget this device* removes the trust.
- [x] What is kept cannot be opened without the device key (a test that the stored bytes are sealed, and that the key is
      not extractable).
- [x] Tests: the order of opening (tab, trusted copy, form), turning it off, forgetting, a copy that will not open.
- [x] Docs: `docs/using/` (the static app: unlocking, trusting a device), `docs/architecture.md` (what the static flavour
      keeps on a device).

## Open questions

None left: see Design and What.

## Implementation notes

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.

### 2026-11-19

Built, as the agreed design.

- `vault.ts`: `trustDevice` (a non-extractable AES-GCM key from `generateKey`, sealing the access and the key with
  `persist.ts`'s `sealBytes`, now exported with `openBytes`), `deviceTrusted`, `trustedCopy` (removes a copy that does
  not open), `distrust`. `forgetDevice` already deletes the whole database.
- `session.ts`: `openDevice` (another tab, then the trusted copy unless the `meghnaad-locked` mark is set, then the
  form), `setTrusted` (off also removes a passkey checked by signature), the mark set by `lock` and cleared by any
  unlock, `connect`/`unlockWithSecret` with `trust`, and the idle check skipping a trusted device.
- `StaticGate.svelte` (the checkbox on both forms) and `DevicePanel.svelte` (the switch, the idle minutes shown as off).
- Tests: `vault.test.ts` (the key is not extractable, nothing readable is kept, a damaged copy is removed),
  `session.test.ts` (the order of opening across reloads, Lock now staying locked, the idle lock, turning trust off,
  forgetting), `StaticGate.test.ts`, `DevicePanel.test.ts` (new).
- **Departure**: the design first said a copy of the profile would open nothing. The standard lets a browser keep such a
  key in its profile unencrypted, so the spec, the switch's warning and `docs/deploy.md` say trusting a device trusts
  its browser profile.
- **Built together with 0017, 0018 and 0019** (they share `session.ts`, `vault.ts`, the gate and the Device panel).
- **Sizes**: no change to `tc-core`, the engine or the Worker. The static site's JavaScript (Vite build with
  `VITE_FLAVOUR=static`, gzip -9, all chunks) 354,033 → 359,138 bytes (+5.1 KB): the session, vault and passkey code
  now in their own chunk loaded with the gate, and +55 bytes in the app chunk.
- **Checks**: `npm run lint`, `npm run check`, all web tests (923), `mdbook build docs`.
- Not seen by a person in a browser yet: worth a look on the deployed site (trust, reload, Lock now, reload again).
