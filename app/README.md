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

The engine is `crates/android`, the one library (`libtc_android`) that the Capacitor app also loads. Its JNI names are
`Java_me_utsob_meghnaad_NativeEngine_*`, so `NativeEngine` has to stay in the package `me.utsob.meghnaad`.

## Build and test

```bash
cargo build --profile android -p tc-android        # the engine for this machine, for the unit tests: target/android
node scripts/build-android.mjs                     # the engine for the phone: android/app/src/main/jniLibs
cd app
./gradlew :shared:testDebugUnitTest                # parsing, dates, and commands through the real engine
./gradlew :androidApp:assembleDebug                # androidApp/build/outputs/apk/debug
```

The engine tests are skipped (and say so) when the host library is not built. Gradle takes the JDK from `JAVA_HOME`; use a
JDK 21 (Android Studio's is one).

## What works

Local use only: no bucket connection or sync yet. A report picker, the console line, the rows of a report (with the date in
the phone's time zone), text, table, changed and error results, and the engine's questions as a dialog. Other result
kinds say they have no screen yet.
