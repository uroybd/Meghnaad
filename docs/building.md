# Building and releasing

For maintainers and contributors: how a release is made, how the Android app is built and signed, and how the
Android and terminal apps fit around the engine. Running the apps is in [Getting started](stores.md); the engine and
the stores are in [Architecture](architecture.md).

## Making a release

The version is the one in the root `package.json`. The web app shows it at the bottom of **Settings**, the Android app
takes it as its version name (and a code that rises with it), and the prebuilt package carries it. To release, with
[`just`](https://just.systems) and everything committed on `main`:

```bash
just release          # bump the patch version, commit "chore: bump to <version>", tag v<version>, push both
just release minor    # or major, or an exact version: just release 0.7.0
just release patch tidy   # lint and test first (`just tidy`), and release only if they pass
just ship             # the same, then start the Deploy workflow (`just ship minor tidy` and so on)
```

`release` refuses to run off `main`, with changes not committed, or behind `origin/main`. It runs no checks unless you
pass `tidy` after the level; run `just check` (format, lint, tests, type checks) before committing. By hand it is the
same steps: `npm version <x> --no-git-tag-version` (which writes the version into `Cargo.toml` and `Cargo.lock` too),
commit, then `git tag v<version> && git push origin main v<version>`.

Pushing the tag runs the **Release** workflow, which refuses a tag that isn't `v` plus the version in `package.json`. It
builds once and publishes:

- a new commit on the `release` branch holding the built Worker (`worker/`), the web app and engine (`app/`), the scripts
  and a `wrangler.jsonc` without a build step (the branch only gains commits; nothing is force-pushed),
- a GitHub release titled with the version, holding the same files as `meghnaad-<version>.tar.gz`, whose notes list
  what changed since the release before: each commit's subject, grouped by its conventional type (Features for `feat:`,
  Fixes for `fix:`, then `perf:`, `refactor:` and `doc:`; chores, CI and the bumps are left out). So write the subject
  for someone reading the notes. `just notes` shows them for what is on `main` now (`scripts/release-notes.mjs`),
- the Android APK, added to that release when its build finishes ([signing it](#releases-and-signing)).
- `meghnaad-hook` and `meghnaad` (the terminal app) for Linux and macOS (x86_64 and arm64), as `<program>-<version>-<platform>.tar.gz`,
  added to that release when their builds finish.

**Build times.** A tag's run can read the caches saved on `main` but never another tag's, so the Release workflow
saves none of its own and reads `main`'s: CI's web job builds what the release builds for the web (both Workers and
the static app too) under the shared Rust cache key `web`, and keeps `worker-build` in a cache of its own (keyed on its
version) that the release and the deploy restore in place of building it; the APK reads CI's android job's Rust and Gradle caches.
The terminal programs build without a cache. The Deploy workflow that `just ship` starts deploys the release's own
build once it is on the `release` branch, without building it again.

**A Homebrew tap.** The release can also keep the `meghnaad-hook` and `meghnaad` formulas up to date in a tap, so
people install them with `brew install <owner>/tap/meghnaad-hook` (or `meghnaad`, which brings `task` with it). It is off until you set the repository **variable** `HOMEBREW_TAP` to the tap
(`owner/homebrew-tap`) and add the **secret** `HOMEBREW_TAP_TOKEN`: a fine-grained token with **Contents: read and write**
on that one repository (the workflow's own token can't push to another). For a release version (not `1.2.3-rc.1`) the
`tap` job waits for the four platforms' builds, writes `Formula/meghnaad-hook.rb` and `Formula/meghnaad.rb`
(`scripts/homebrew-formula.mjs`, with the URL and sha256 of each platform's archive) and pushes them as "Update
meghnaad-hook and meghnaad to v`<version>`". The terminal app's formula used to be `meghnaad-tui`: the job also writes the
tap's `formula_renames.json` (`meghnaad-tui` → `meghnaad`) and removes `Formula/meghnaad-tui.rb`, so `brew update` moves an
installed `meghnaad-tui` to `meghnaad`, and the formula keeps a `meghnaad-tui` link to the program for now. If a platform's build fails, no formula is written; re-run the failed jobs and it follows. Without the token it only warns.

**Trying a build first.** **Actions → Release → Run workflow** builds the same things as `<version>-dev.<commit>` and only
updates the `release` branch (the APK stays as the run's artifact). `node scripts/build.mjs && node
scripts/package-release.mjs` produces the same package in `release/` on your machine. The workflow doesn't run the tests,
so release a commit CI has passed.

**The documentation site.** These docs are an mdBook (`docs/book.toml`) published to GitHub Pages by the **Docs**
workflow on a push to `main` that touches `docs/`. Set the custom domain in the repository's Pages settings, and the
repository variable `DOCS_DOMAIN` so the sitemap uses it.

## Building the Android app

You need the Android SDK and NDK (Android Studio installs both), Node.js 20+, Rust and `cargo install cargo-ndk`. Then:

```bash
npm run build:android          # the engine for the phone, then the APK
adb install -r app/androidApp/build/outputs/apk/debug/androidApp-debug.apk
```

The script finds the SDK in `ANDROID_HOME` or `~/Library/Android/sdk`, uses the JDK 21 that comes with Android Studio, adds
the Rust targets it needs, and builds for `arm64-v8a` (phones). A debug build also has `x86_64` (the emulator on an Intel
machine), and `ANDROID_ABIS` chooses. It doesn't build for 32-bit ARM.

**Size.** The library has its symbols stripped (`profile.android` in `Cargo.toml`) and is packed compressed, a release is
arm64 only, and R8 cuts unused code and resources (`app/androidApp/proguard-rules.pro` keeps what is looked up by name:
the engine's functions). If you add a class that something finds by name, add it there, and try the release build, not
only the debug one. Open `app/` in Android Studio to run it from there; `app/README.md` says how to see the screens live
on a computer. The launcher icon is the web app's logo: `node scripts/android-icons.mjs` makes it again from
`web/public/logo.png` (it needs ImageMagick).

### Releases and signing

The **Release** workflow (a `v*` tag, or Actions → Release → Run workflow) also builds the APK, with the web app's
version ([Making a release](#making-a-release)): attached to the GitHub release for a tag, or
kept as the run's artifact (`android-apk`) otherwise. An installed app can only be updated by an APK signed with the
**same key**, so make one key and keep it:

```bash
keytool -genkeypair -keystore meghnaad.keystore -alias meghnaad -keyalg RSA -keysize 2048 -validity 10000
base64 < meghnaad.keystore | tr -d '\n'        # the value of the first secret below
```

Add these repository secrets: `ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` (`meghnaad`)
and `ANDROID_KEY_PASSWORD`. Without them the workflow still builds, but the APK is debug-signed: it installs, and the next
one can't update it in place. Don't commit the keystore. Locally, `npm run build:android -- --release` signs with the same
variables in the environment (`ANDROID_KEYSTORE_FILE` is the path).

## How the Android app fits together

```
screens (Compose)                                      app/shared: Store, one file per page
  │  JSON in and out                                   Engine.kt (mirrors web/src/lib/api.ts)
  ▼
LocalEngine (Kotlin)  ── AppEngine: one engine for the screens, the widgets, the reminder alarm and the sync worker
  │  JNI                                               NativeEngine.kt (package me.utsob.meghnaad)
  ▼
crates/android (Rust): tc-core + the stores + the settings routes
  │  the forge's API, S3 signed with SigV4, or the sync server's protocol; all over HTTPS
  ▼
your repository, bucket or sync server
```

- `crates/android/src/engine.rs` is the engine and `jni.rs` the thin layer Kotlin calls. Every call answers in JSON.
- `crates/tc-core/src/request.rs` turns a request into a command and its answer for this engine and the browser's, so the
  two can't differ. `webapi.rs` answers the settings routes (`/api/config`, `/api/settings`, …) for any store: the Worker
  keeps them in the bucket, the app in a folder, asked of the engine's `api` call.
- `s3.rs` is the bucket as tc-core's `ObjectStore`, and `sigv4.rs` signs requests (tested against Amazon's own examples).
- `syncserver.rs` is a TaskChampion sync server as a `taskchampion::Server` (`/v1/client/…`, `X-Client-Id`, the key
  derived with the client id as the salt). `tests/syncserver.rs` runs the engine against a server kept in the test and,
  with `--ignored` and `TC_SYNC_SERVER`, against the real `taskchampion-sync-server` and the `task` CLI.
- `gitrepo.rs` carries `tc_core::forge`'s requests over `ureq`: the tasks are `tc_core::gitsync::GitServer` and the
  shared files `GitObjects`, both over the same branch. `tests/gitsync.rs` (`--ignored`, with `TC_GIT_API`,
  `TC_GIT_REPO`, `TC_GIT_TOKEN` and `TC_GIT_REMOTE`) runs two phones and the real `task` on one branch of a real forge.
- `engine.rs` chooses between the sync server, the git branch and the bucket for the tasks, keeps a bucket of its own
  for the shared files (the branch keeps them itself), and remembers where the tasks were last synced. Another place
  makes `sync` fail with `backend-changed` until it is asked again with `switch` (`merge` or `replace`), and the new place's replica is made aside and swapped in only once
  it has synced.
- The replica is saved to a file in the app's storage after every change. The screens, widgets and reminder alarm share one
  engine, so what a widget changes shows in the app and the other way round.
- Reminders: `Notify.kt` holds the web's rules (`notify.ts`). After every change the next moment worth a reminder is worked
  out and `AlarmManager` is set for it; its receiver shows what is due and sets the next one (also after the phone starts).
  The background sync (WorkManager, every half hour) runs while a widget is placed or reminders are on.

Tests: `cargo test -p tc-android` (an in-process bucket stands in for S3), and in `app/`
`./gradlew :shared:desktopTest :shared:testDebugUnitTest :androidApp:testDebugUnitTest` (build the engine for this
machine first with `cargo build --profile android -p tc-android`; the tests that run the real engine skip without it).

## How the terminal app fits together

It is a front end like the web app and the phone (layer 2 in `AGENTS.md`), with the real `task` as its backend instead
of `tc-core`'s engine. `tc-core` comes in only for what `task` has no command for, the same helpers the page and the
phone call: completion (`complete`), the add preview (`preview`), the cells of a report (`cells`), dates in
`dateformat`, colours in the taskrc's words, and what a board means and measures (`board`), all fed with what `task`
said, so a report's columns, dates and indicators read as in the web app. The settings those need come from `task
_show`, which is what `task` itself resolved, parsed by the engine's taskrc reader, which keeps nothing sync or
credential-like. The app never reads or edits your taskrc file itself; review's setup goes through `task config`.
Chains and task commands are the web app's logic, ported to Rust with its tests as the phone has them in Kotlin, and so
are the Projects and Tags lists, review (tasksh's, from the web app's port), the reminders, the timer and the glue of
boards (Android's `Boards.kt`). Nothing of it is compiled into the browser engine or the
Worker.
