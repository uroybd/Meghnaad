# 0014: Sync without holding up the screen

- **Status:** in progress
- **Created:** 2026-11-18
- **Layer:** both (the hosts of layer 1, `crates/browser` and `crates/android`, with a shared piece in `tc-core`; and
  the web app and Android app around them)
- **Front ends:** web (both flavours), Android. The terminal app is left out: its sync is `task sync` in the background
  and never holds its screen.
- **Supersedes / related:** 0011 (git as the sync backend), 0013 (the static web app)

## Why

A sync with a git branch is slower than one with R2: each step is a request to the forge's API (GitHub, Gitea,
Forgejo), a few hundred milliseconds each, one after another. A pull with nothing new is about one request, and a push
after a write reads `meta` and makes a commit, two or three more. While a sync runs, the engine is held for all of it:

- **Web:** the engine's Web Worker answers one message at a time (`queue` in `web/src/engine/engine.worker.ts`), and
  `Engine::sync(&mut self)` (`crates/browser/src/lib.rs`) keeps the replica until TaskChampion's `Replica::sync` has
  made its last request. A command, or a page reading what to draw, sent during a sync waits for it.
- **Android:** `LocalEngine.sync` runs inside `synchronized(this)`, the lock every engine call takes, so a tap during a
  sync waits for it too.

Syncs run after every write, every 30 seconds while the app is looked at, and when it is looked at again, so this is
felt often. Both apps already start from the copy they keep without waiting for a sync; the tasks are usually there
already, and only the network stands in the way.

The static site also reads its settings from the branch on every unlock and reload (`GET /api/config` before the
engine starts, then `/api/settings` and the hooks list), each a fresh request, though they rarely change.

## What

From the outside, nothing new to learn; things stop waiting:

- Typing a command, ticking a task or opening a page while the app syncs answers at once, as it does when no sync runs.
  The sync indicator still shows a sync under way.
- After a write, the change is pushed a moment later (about 2 seconds after the last of a run of changes) instead of at
  once, so a burst of edits costs one push. It is pushed at once when the tab is hidden, the page closed or the Android
  app sent to the background. What isn't pushed yet is kept on the device as today and goes with the next sync.
- On the static site, unlocking draws the app at once from the settings kept on the device and checks the branch's in
  the background, redrawing if they changed.
- Pull to refresh on Android, **Retry**, and starting with nothing kept on the device still wait for their sync, since
  that is what was asked for.

## Not doing

- Changing TaskChampion or the sync format. The stores, layouts and encryption stay as they are, and `task sync` sees
  no difference.
- Pushing without the engine held. TaskChampion marks local changes as sent only when the store has accepted the new
  version, inside `Replica::sync`; detaching the push from that could lose or repeat changes.
- Caching settings in the Worker flavour (the Worker answers them, and R2 is quick). Decided 2026-11-18: leave it.
- Changing Android's background sync (WorkManager). Decided 2026-11-18: not needed.
- Making git requests faster (fewer round trips per commit, parallel requests). Worth doing separately.
- The terminal app.

## Design

### 1. Fetch first, then sync briefly (`tc-core`, both engines)

A sync is split in two:

1. **Fetch, with the engine free.** Read the replica's base version (a moment's hold), then fetch from the store,
   with no replica involved: the chain of versions after the base, and that the newest one has no child. This is the
   slow part of an idle sync.
2. **Sync, with the engine held.** Run `Replica::sync` as today, against a wrapper around the store,
   `tc_core::prefetch::Prefetched<S: Server>`, that answers `get_child_version` from what was fetched and sends
   everything else (`add_version`, snapshots) to the store. If the store moved after the fetch, `add_version` says
   `Conflict`; the wrapper then forgets what it fetched and TaskChampion's own retry goes to the store, so the result is
   the same as today, only slower in that rare case.

An idle sync then holds the engine for no request at all, and a sync after a write only for the push. This rests on
how TaskChampion 3.1 syncs (`src/taskdb/sync.rs`, read 2026-11-18): it asks `get_child_version` from the base until
there is no child, applying each, and with no local changes stops there ("no changes to push to server") without
`add_version`. A rejected push (`ExpectedParentVersion`) starts its loop again from the new base, which the wrapper
then sends to the store. A replica with nothing in it first asks `get_snapshot`, which the wrapper passes through: a
first start waits, as it does now.

- **Web:** a new export runs the fetch without borrowing the `Engine` (a wasm-bindgen object can't be borrowed by a
  second call while an `async` one holds it), given the base version and the connection, and returns what it fetched.
  The Worker runs it outside its queue and then queues `sync` with the result. `client.svelte.ts` keeps one sync at a
  time, as now. This covers both flavours: the Worker's `RemoteServer` (`/api/tc/*`) and the static site's git branch.
- **Android:** `LocalEngine.sync` reads the base and fetches outside `synchronized`, through a JNI function that takes
  the connection and doesn't touch the engine's handle, then takes the lock only for the sync. A change of store
  (`switch`) is unchanged: it is rare, asked for, and already waits.

### 2. Push a moment after a write (web, Android)

`pushSoon` waits until about 2 seconds have passed since the last write, then syncs. It is flushed at once on
`visibilitychange` to hidden and on `pagehide` (web), and when the activity stops (Android, `ON_STOP`). A pull asked for
in the meantime (the timer, the tab coming back) pushes too, as a sync always does. Ported to Kotlin with its tests, as
the sync rules already are (`Sync.kt` follows `client.svelte.ts`).

### 3. Settings kept on the device (static site only)

The last `config`, `settings.json` and list of defined hooks are kept in IndexedDB beside the replica, sealed as it is
(`persist.ts`, AES-GCM under the key of the tasks). Unlocking starts the engine from them and draws; the branch's are
read in the background and, where they differ, applied and the page drawn again (`onChanged`). With nothing kept (the
first unlock on a device), it reads them first, as now. Writing a setting still goes to the branch and waits for it.

## Acceptance criteria

- [ ] Measured first, on a real GitHub branch: how long the engine is held per sync, idle and after one write, on the
      web (static site) and Android. Measured again after; the numbers go in the implementation notes.
- [x] An idle sync makes no request to the store while the replica is held: a `tc-core` test with a store that records
      when each request comes.
- [x] A store that moves between the fetch and the sync (another device pushed) still ends in the same state as today's
      sync: tests with a store that adds a version in between, for a pull and for a push that meets a `Conflict`.
- [x] Web: a command sent while a fetch is under way is answered before the fetch ends (a test of the Worker's queue
      with a slow fetch).
- [x] Android: an engine call made while a fetch is under way doesn't wait for it (a test of `LocalEngine` with a slow
      fetch, or of the split in `crates/android`).
- [x] Pushing after a write waits about 2 seconds after the last write, and goes at once when the tab is hidden or the
      app stops: tests in `client.svelte.ts`'s and `SyncController`'s suites, with a fake clock.
- [x] Static site: unlocking with settings kept on the device draws without a request to the branch first, and a change
      on the branch is picked up and drawn after (Vitest, with the engine mocked).
- [x] `cargo test` (with `tc-core`'s git sync tests), the Android `gitsync` test against a real forge (`--ignored`), and
      `./scripts/interop-local.sh` pass.
- [x] Docs: `docs/architecture.md` (Sync: the fetch and the brief hold; the static site's kept settings),
      `docs/using/different-from-the-desktop.md` (Syncing: a write is pushed a moment later).
- [x] Size: engine and Worker wasm measured before and after.

## Open questions

None. Answered 2026-11-18: the push waits 2 seconds; the Worker flavour keeps no settings on the device; Android's
background sync stays as it is.

## Implementation notes

2026-11-18, built (uncommitted):

- **tc-core.** `live::BaseVersion`, a shared handle on `LiveStorage`'s base version (as `Generation` is for the
  change counter), so a host reads it without the replica. `prefetch.rs`: `fetch` (the chain of children from a base,
  and that the last has none) and `Prefetched::boxed`, the store handed to `Replica::sync`. Beyond the design, after
  its own push is taken it answers the child query TaskChampion makes before stopping with "none" (the next sync, from
  that version, pulls anything pushed in that moment), so a push holds the engine for the push alone. Tests in
  `tests/prefetch.rs`.
- **Browser engine.** `Engine::source`, the free `fetch`, and `Engine::sync(base, fetched?)`; `Reader` (static build:
  `Branch` is now `Clone`) reads a settings route with no engine borrowed. `tc-engine.d.ts` follows.
- **Web.** `queue.ts` (`serial`, `splitSync`) is the worker's turn, tested in `queue.test.ts`; the worker's `sync` and
  `refresh` run through it. `client.svelte.ts`: `PUSH_AFTER_MS` (2 s), flushed on `visibilitychange` to hidden and
  `pagehide`; a sync that runs meanwhile carries the push. Static site: `KEPT_ROUTES` (`/api/config`, `/api/settings`,
  `/api/hooks/defined`) kept sealed in IndexedDB (`persist.ts`: `loadKept`, `keepAnswer`, `forgetKept`); the first read
  of each in a page is answered from what was kept, then `refresh` reads the branch outside the turn, and a change is
  given to the engine (`config`) and told to `onSettingsChanged`, which the store answers by reading that setting
  again. Any write to a settings route clears what was kept; `forget` does too.
- **Android.** `Engine::source`, `engine::fetch`, `Engine::sync_fetched` (`sync` is unchanged for its callers); JNI
  `nativeSource` (under `LocalEngine`'s lock, a moment), `nativeFetch` (no engine handle at all, so `forget()` freeing
  the handle cannot race it) and `nativeSyncFetched`. A change of place (`switch`, or tasks synced elsewhere) is not
  fetched ahead. `SyncController`: `pushSoon` waits `PUSH_AFTER_MILLIS`, `pushNow` from `MainActivity.onStop`.
- **Measured** (Android engine, release, a scratch Forgejo on this machine, median of 5): the engine held per idle sync
  11.4 ms before, 0.4 ms after (the 11.2 ms fetch now runs beside it); per sync after one write 183.9 ms before,
  160.5 ms after (the pull and the query after the push are gone; the commit stays). With GitHub each request is a few
  hundred milliseconds, so an idle sync saves about one, and a push about two.
- **Sizes** (gzip -9): browser engine 861,745 → 865,258 bytes (+3.5 KB); static engine 962,005 → 966,450 (+4.4 KB);
  Worker 769,980 → 769,902 (unchanged).
- **Checks:** `npm run lint`, `cargo test --workspace` and `lua_hooks`, `npm --prefix web test` (874), `npm run check`,
  the script tests, the Kotlin unit tests and debug APK, `./scripts/interop-local.sh` (all checks passed, against a
  scratch SeaweedFS; this `aws` CLI needs `AWS_REQUEST_CHECKSUM_CALCULATION=when_required` for it), the Android `gitsync` tests against the scratch Forgejo
  (`--ignored`, the task CLI one skipped), `mdbook build docs`.
- **Left:** the first criterion's numbers from a real GitHub branch, on the static site and the phone (needs your
  token; the Forgejo numbers above are the same measure on a fast forge). Settings sharing on Android
  (`filesync`) still runs under the lock after the tasks' sync; it is separate from this spec's tasks sync and could
  get the same split. On the static site, a hook script's source and secrets are still read from the branch when the
  hooks are prepared after unlocking (only when hooks are defined).
