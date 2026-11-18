# 0017: Reminders keep coming while the static web app is locked

- **Status:** in progress <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-19
- **Layer:** 2 (UI)
- **Front ends:** web, the static flavour only (the Worker flavour has no lock; the phone and the terminal app have their
  own reminders)
- **Supersedes / related:** 0013 (the web app as static files, which added the lock), 0003 (custom reminder)

## Why

The static web app locks its tabs after a spell without use (*Lock after this many idle minutes*, 30 by default): the
key of the tasks and the forge token are dropped and every tab reloads with nothing of them left. The reminders went
with them. Someone who keeps the tab open in the background while working elsewhere is locked out after half an hour and
from then on hears nothing: no due, scheduled or wait reminder, until they come back and unlock, which is when they no
longer need one.

A locked tab should still remind. It needs only a little of each task for that, never the key or the token.

## What

1. **What a locked tab keeps.** On locking, each open tab keeps the reminder list it last had: for each task its
   description, its due, scheduled and wait dates, `remind_in`, and whether it is running. Nothing else of the tasks,
   and never the key or the token. It is kept in that tab only (its `sessionStorage`), and is gone when the tab is closed.
2. **What it does with it.** The reminders come as before (*Settings → Reminders*: system notifications, the lead time,
   the hour for all-day dates), from that list, as of the lock. The locked screen says so under the unlock form:
   *Reminders keep coming for N tasks, as they were when this tab locked.*
3. **Clicking a reminder** while locked brings the tab to the front, at the unlock form. Once unlocked, it opens that
   task.
4. **Unlocking** goes on as today: the engine starts, the reminders take their list from it again, and what was
   reminded while locked is not said twice.
5. **No list in a locked tab after**: closing the tab, *Forget this device*, or opening a new tab while all are locked.
   Each of those has no reminders until unlocked, as today.

## Not doing

- Fetching anything while locked: the list does not change until the tab is unlocked, so a task finished on the phone
  meanwhile can still be reminded of. (Fetching needs the key.)
- Keeping the list anywhere but that tab: not `localStorage`, not IndexedDB. It is carried across the lock's reload in
  that tab's `sessionStorage`, which the browser may write to its profile for session restore, and is removed on unlock.
- A service worker or push, for reminders with every tab closed.
- The Worker flavour, the phone and the terminal app.

## Design

Agreed 2026-11-19.

- **The lock still reloads.** Emptying the page by hand (the store, the review, the selection, the request caches) would
  have to grow with every new cache; the reload empties all of it by design. Before it reloads, a tab writes its
  reminder list to its own `sessionStorage` (`web/src/lib/lockedReminders.ts`), only when reminders are on, and only
  the fields a reminder reads (uuid, status, description, due, wait, scheduled, start, `remind_in`).
- **Who knows the list.** The notifier hands each poll's list to `lockedReminders` (which holds it in memory while
  unlocked); `session.ts` calls the hooks registered with `onLock` before it reloads, and `main.ts` registers the one
  that keeps the list. `session.ts` imports nothing of the app.
- **The locked page.** The gate (`StaticGate.svelte`) finds the list, says *Reminders keep coming for N tasks, as they
  were when this tab locked.*, and checks it every 15 s with the notifier's own rules (`dueNotices`) and its own record
  of what was said (`tw-notified` in `localStorage`), so nothing is said twice either side of an unlock. Without
  permission for system notifications, the notice is shown on the locked page.
- **A click** focuses the tab and remembers the task; once unlocked, the app opens it.
- **Unlocking** removes the list from `sessionStorage`; *Forget this device* does too.
- **Decided:** a count only on the locked screen; running tasks stay in the list; a tab opened while locked gets no
  list.
- **No change to `tc-core`**, so neither WebAssembly grows.

## Acceptance criteria

- [ ] After the idle lock (or *Lock now*), a reminder whose time comes while locked is shown, as a system notification
      when allowed, else in the page.
- [x] A locked tab holds no key, no token and nothing of the tasks but the reminder list (the lock reloads the tab; a test
      checks what the list keeps).
- [x] The locked screen says reminders keep coming, and for how many tasks.
- [ ] Clicking a reminder while locked shows the unlock form; after unlocking, that task is open.
- [x] After unlocking, no reminder that was shown while locked is shown again.
- [ ] Closing the tab, *Forget this device*, or a tab opened while locked: no reminders until unlocked.
- [x] Tests: the list kept on lock (only reminder fields, only when reminders are on), the locked page reminding
      every 15 s and saying each once, the lock's hooks and reload, forgetting, a click while locked.
- [x] Docs: `docs/using/` (reminders, the static app's lock), `docs/architecture.md` if the lock's description there
      changes.

## Open questions

None left: see Design.

## Implementation notes

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.

### 2026-11-19

Built, as the agreed design (the lock still reloads; the list crosses it in the tab's `sessionStorage`).

- `web/src/lib/lockedReminders.ts` (new): `remember` (the notifier's last poll), `keepForLock` (only when reminders are
  on, only the fields a reminder reads), `keptList`, `dropList`, `dueNow` (the notifier's `dueNotices`, with its
  `tw-notified` record), `remindWhileLocked` (every 15 s; a notification's click is kept for `takeClicked`).
- `session.ts`: `onLock(fn(forgetting))` hooks run once before the reload; *Forget this device* sends `forget` (and
  `lock`, for tabs of an older version), and a locked tab follows it too. `main.ts` registers the hook and drops the
  list on unlock; `App.svelte` opens a clicked task once the address is followed; `notifier.svelte.ts` hands each poll
  over and shares the storage keys.
- `StaticGate.svelte`: the count line and the in-page notice when notifications are not allowed.
- Tests: `lockedReminders.test.ts` (8), the gate's (`StaticGate.test.ts`), the hooks and the reload in `session.test.ts`.
- **Built together with 0017, 0018 and 0019** (they share `session.ts`, `vault.ts`, the gate and the Device panel).
- **Sizes**: no change to `tc-core`, the engine or the Worker. The static site's JavaScript (Vite build with
  `VITE_FLAVOUR=static`, gzip -9, all chunks) 354,033 → 359,138 bytes (+5.1 KB): the session, vault and passkey code
  now in their own chunk loaded with the gate, and +55 bytes in the app chunk.
- **Checks**: `npm run lint`, `npm run check`, all web tests (923), `mdbook build docs`.
- **Left**: seen by a person in a browser: a reminder arriving on the locked page, a click opening the task after the
  unlock, and a closed tab, *Forget this device* or a tab opened while locked reminding of nothing (each is covered by
  the parts' tests, not end to end). Two criteria were reworded to the built design, with the person's yes.
