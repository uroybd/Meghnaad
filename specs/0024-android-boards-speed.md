# 0024: Android performance: a screen never waits on a sync

- **Status:** in progress <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-19 (widened 2026-11-21)
- **Layer:** host (`crates/android` and the app's `LocalEngine`); no behaviour of `task` changes
- **Front ends:** Android
- **Supersedes / related:** 0014 (sync without holding up the screen); 0023 (the web app's reads); boards
  (`tc_core::board`)

## Why

Reported by uroybd:

- 2026-11-19: a board (dashboard) takes noticeably longer to appear on the phone than the same board in the web app.
- 2026-11-21: the first time a task's info panel opens after the app starts, it takes a few seconds; later opens are
  quick. Most noticeable with git sync, which is slow.

## What

A screen's read (a task's info, a report, a board's sections) is answered in about the time the engine takes to run it,
whatever a sync is doing at that moment, and what is left still slow is measured on a phone rather than guessed.

## Not doing

- Changing what a sync does or how often (every 30 s while the app is visible, 2 s after a write).

## Design

Proposal, from reading the code on 2026-11-21 (not yet measured on a phone; no device was attached).

### What makes the first info panel slow

Every engine call goes through `LocalEngine.call`, which takes the engine's lock (`synchronized`). A sync takes the same
lock for two stretches, and a read asked for meanwhile waits for them:

1. `nativeSource`, under the lock. On the first sync after the app starts there is no key kept yet, so `connect`
   fetches the salt from the store (with git, a request to the forge's API) and derives the key: 600,000 rounds of
   PBKDF2 (`crypto::PBKDF2_ITERATIONS`), which the code itself calls slow on a phone. This is the likeliest cause of the
   first-open delay: the app syncs as it starts, and the first info panel is usually opened during it.
2. `nativeSyncFetched`, under the lock: applying what was fetched (quick), then the network. A push (with git, several
   forge API calls), and then, when settings are shared, the settings files (`filesync::sync_files`: a listing and a
   request for each file that differs), on every sync, including the pull every 30 s with nothing new. With git this can
   hold every read for seconds, again and again, not only the first time.

Fetching what is new (`nativeFetch`) is already done without the lock (0014), and is not the problem.

### Proposal

- **Derive the key with no lock.** `source` only reads what it needs from the engine (where the replica last synced,
  its version, the key kept from the last sync); finding the salt and deriving the key move into `fetch`, which runs with
  the engine free. `connect` becomes a function of the settings and the kept key, not of the engine.
- **Sync the settings files with no lock.** The files sync works on the settings folder, not the replica, so it moves
  after the lock is let go, on a `FileStore` of the same folder; only telling the engine that `hooks.lua` came (so the
  next command reads it) takes the lock again, for a moment. A setting saved on the phone during that sync is sent by the
  next one, as when it is saved just after.
- **The push stays under the lock**: it needs the replica's waiting operations and records the new version. It is one
  request per sync with something to push (more with git), only after a write.
- **Measure.** Timings in the app's log (behind a debug flag, not in a release): the wait for the lock and the time
  under it for each call, each sync stretch, and each screen's first draw (a task's info, a report, each board
  section). Then, on a phone, the same for boards (the rest of the original question: the sections' reads one after
  another or together, JNI and JSON per section, drawing).

## Acceptance criteria

- [ ] With git sync, opening a task's info while the first sync after start is running answers as quickly as with no
      sync running (measured on a phone, with the log's timings).
- [x] No read waits on deriving the key, on the store's network for the key, or on the settings files.
- [x] A sync still pulls, pushes and keeps the settings files in step as before (the existing tests, and one for a file
      saved while the files sync runs).
- [ ] The timings exist in a debug build and nowhere in a release build.
- [ ] Boards measured on a phone, with what is slow written down here as the next step.

## Open questions

- A target in milliseconds for a board (the info panel is now about 10 ms on the emulator).

## Implementation notes

2026-11-21. Answered by uroybd: milliseconds are the measure; yes, keep the derived key across launches (Keystore).

Done (`crates/android`, `app/shared` jvmCommonMain):

- `Engine::source` no longer reaches the store: it takes the kept key (this run's, trusted; else the one on the phone,
  to be checked) and returns. `engine::fetch` does `connect` (salt, and PBKDF2 only when the salt differs from the kept
  key's) with the engine free. `connect` is a free function now.
- The key is kept in `<engine folder>/sync-key.enc` (`Kept { fingerprint, salt, key }` as JSON, sealed with the engine
  key from the Keystore). A key read from it is checked against the store's salt once (one request, engine free) before
  use, so a store made again with a new salt never gets versions sealed with the old key. `LocalEngine.sync` deletes the
  file when there is no connection (disconnected); Start over keeps it.
- The settings files moved out of `sync_fetched` into `engine::sync_files`, run by the new `nativeSyncFiles` with the
  engine free on `files::Guarded`: each write takes the engine's lock, and is skipped when the file changed since the
  sync read it (the person's save wins; the next sync sends it).
- `LocalEngine` runs one sync at a time (a coroutine `Mutex`): the `SyncWorker` started at launch used to overlap the
  app's own sync and derive the key a second time. `forget` takes the same lock, so the engine is never freed under a
  sync running without it.
- Timings (`Timings.kt`) in the log (tag `MeghnaadTimings`) when the app is debuggable; a release never sets the sink.
  The measuring code is in the release too, a null check per call.
- Departure: the form's API address now also takes `http://localhost` / `http://127.0.0.1`, as the engine already did,
  so the emulator could reach a local Forgejo.

Measured on the emulator (arm64, Android 36) against the scratch Forgejo behind a proxy adding 250 ms per request, cold
start then a task's info at once; engine held = what every screen waits for:

| | before | after |
| --- | --- | --- |
| first sync, engine held for the key | 700–810 ms (salt + PBKDF2) | 0–11 ms |
| second (overlapping) sync, engine held | 420–440 ms | 0 ms (waits its turn, reuses the key) |
| apply and push, engine held | 160–190 ms | 0–2 ms (nothing to push) |
| start-up commands waiting on the engine | 116–280 ms | 0–10 ms |
| a task's info, answered | 1–2 ms (opened after the sync) | 10–13 ms |
| key and fetch, engine free | 280 ms | 610–1090 ms the first time, 280 ms after |

The emulator runs PBKDF2 on the Mac's CPU (about 400 ms); a phone is several times slower, which is the seconds
reported. The key now comes from the phone after the first launch, so a launch costs one salt request instead of the
derivation, and it never holds a screen. The first fetch after start still takes 600+ ms with the engine free: the salt
check and the fetch; it delays the new tasks, not the screen.

Checks: `cargo test`, `cargo clippy`, the git tests against the scratch Forgejo (`--ignored`, the `task` CLI one too),
`npm run lint`, web tests, svelte-check, Kotlin unit tests (485), debug APK built.

Left: boards measured on a phone (criterion 5); the first criterion was measured on the emulator, not a phone.
