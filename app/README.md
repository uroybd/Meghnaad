# Meghnaad for Android (Kotlin, Compose)

The next Android app: Kotlin and Jetpack Compose over the same Rust engine as the web app and the Capacitor app. It is
early: the web app and `android/` (Capacitor) remain the apps to use until this one reaches parity. See `docs/android.md`
for the app that ships today.

- `shared`: what is common to every screen. `commonMain` has the engine's JSON types (`Types.kt`, mirroring
  `web/src/lib/types.ts`), the engine API (`Engine.kt`, mirroring `web/src/lib/api.ts`), the state (`Model.kt`) and the
  screens (`App.kt`). `androidMain` has the JNI binding (`NativeEngine.kt`) and the engine that keeps its replica in a file
  (`LocalEngine.kt`).
- `androidApp`: the activity and the Keystore-backed engine key. Its id is `me.utsob.meghnaad.next`, so it installs beside
  the Capacitor app.
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

A bucket connection and sync (the web client's rules), a report picker, the console line, the rows of a report, text, table,
changed and error results, and the engine's questions as a dialog. Other result kinds say they have no screen yet.
