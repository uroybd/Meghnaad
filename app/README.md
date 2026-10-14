# Meghnaad for Android (Kotlin, Compose)

The next Android app: Kotlin and Jetpack Compose over the same Rust engine as the web app and the Capacitor app, made
for a phone only (one layout: a menu, cards, a prompt at the bottom). It follows the web app screen for screen. The
Capacitor app in `android/` is still what is released until this one replaces it. See `docs/android.md`.

- `shared`: what is common to every screen. `commonMain` has the engine's JSON types (`Types.kt`, mirroring
  `web/src/lib/types.ts`), the engine API (`Engine.kt`, mirroring `web/src/lib/api.ts`), the state (`Store.kt`) and the
  screens (`Shell.kt` and one file per page or panel). `androidMain` has the Keystore vault, the preferences, the one
  engine of the process (`AppEngine`) and the reminders' notifications and alarms (`AndroidReminders`); `jvmCommonMain` has
  the JNI binding (`NativeEngine.kt`) and the engine that keeps its replica in a file (`LocalEngine.kt`).
- `androidApp`: the activity, the reminder alarm's receiver, the file pickers, and the home-screen widgets with their
  background sync (`widgets/`). Its id is `me.utsob.meghnaad.next`, so it installs beside the Capacitor app.
- `desktopApp`: a window for looking at the screens live (see below). A dev tool, not a product: it is not shipped.

The engine is `crates/android`, the one library (`libtc_android`) that the Capacitor app also loads. Its JNI names are
`Java_me_utsob_meghnaad_NativeEngine_*`, so `NativeEngine` has to stay in the package `me.utsob.meghnaad`.

Where `tc-core` already does something, the app calls it and does not copy it: splitting a command line, writing a date in
the taskrc's `dateformat`, and writing a duration the way Taskwarrior does are `crates/android/src/helpers.rs`, reached
through `RustHelpers` (they take no engine, so they never wait for a sync). The rest of what the screens do is the web app's
own view logic (`web/src/lib`), ported with its tests.

## Looking at it live

```bash
cargo build --profile android -p tc-android        # the engine for this machine (again after any Rust change)
cd app
./gradlew :desktopApp:hotRun --auto                # a phone-sized window; saving a file under shared/ updates it in ~2 s
```

It uses Compose Hot Reload (needs a JetBrains Runtime: Android Studio's is one, so point `JAVA_HOME` at it). The preview keeps
its tasks in `desktopApp/build/preview-data` (demo tasks the first time; delete the folder to start again), never your real
ones, and saves no keys. To try a bucket, set `MEGHNAAD_ENDPOINT`, `MEGHNAAD_BUCKET`, `MEGHNAAD_REGION`,
`MEGHNAAD_ACCESS_KEY`, `MEGHNAAD_SECRET_KEY` and `MEGHNAAD_ENCRYPTION_SECRET` in the environment: they are held in memory and
never written, but what you change is pushed to that bucket.

## Build and test

```bash
cargo build --profile android -p tc-android        # the engine for this machine, for the unit tests: target/android
node scripts/build-android.mjs                     # the engine for the phone: android/app/src/main/jniLibs
cd app
./gradlew :shared:testDebugUnitTest                # parsing, dates, cells, sync, and commands through the real engine
./gradlew :shared:desktopTest                      # the same tests without Android (quicker)
./gradlew :androidApp:assembleDebug                # androidApp/build/outputs/apk/debug
```

The engine tests are skipped (and say so) when the host library is not built. Gradle takes the JDK from `JAVA_HOME`; use a
JDK 21 (Android Studio's is one).

## What works

Everything the web app does, on a phone: connecting (or using the phone alone), the pages (Tasks, Projects, Tags, Summary,
Calendar, Burndown, Console), the task detail and editor, bulk actions, the console with completion and history, the engine's
questions as dialogs, tasksh's review (a card), Settings (taskrc with its colour themes, tasksh, commands with their icons,
hooks and their secrets, urgency, sync), file import and export through the system's pickers, and pull down to sync.

On the phone it also has what a browser tab cannot: reminders that work with the app closed (an alarm set for the next
moment something is due, a notification that opens the task), the running-task timer in the top bar, and the three
home-screen widgets with a background sync.

Not done yet: the app has no formatter or linter, it is not released (the Release workflow builds the Capacitor app), and
the desktop preview keeps no secrets.
