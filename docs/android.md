# The Android app

The same app, in a shell, with the engine as a native library and a widget on the home screen. It keeps your tasks on
the phone and syncs them straight with your bucket over S3: no Worker and no Cloudflare Access are involved, and your
keys stay on the phone.

> **Status:** it builds, its Rust half is tested (signing, sync against a bucket that enforces conditional writes,
> settings, hooks), and it has been tried on an Android 16 emulator: connecting, adding and syncing tasks, and the widget
> (list, tick to finish, quick add, sync). It has not been run on a physical phone yet, so expect rough edges.

## What you get

- Everything in the web app: the console, the pages, review, urgency, recurrence, hooks, settings.
- **Works offline.** The engine and the tasks are on the phone; changes sync when there is a network.
- **Home-screen widgets**, three of them, each with its own settings (long-press the widget, then its pencil; the same
  screen opens when you add it):
  - **Tasks**: the tasks of any report, the built-in ones and your own, narrowed by a filter (`project:Home +work`), as
    many as you choose, with a title of your own. Tap a circle to mark a task done, the text to open the app, **+** to
    add a task by typing it as in the console (`Buy milk due:tomorrow +home`), and the sync button to pull now. Under
    each task it shows the fields you tick: project, due date, priority, tags, urgency, age, scheduled, wait, recurrence,
    task number, notes.
  - **Burndown**: daily, weekly or monthly bars of the tasks a filter picks: finished at the bottom, then started, then
    pending, with how long it would take to clear them at this pace.
  - **Calendar**: a month with today circled and a dot on the days with something due (red when overdue, amber today,
    blue after), for the tasks a filter picks. The arrows go to other months, and the month's name back to this one.

  They redraw after any change, from the app or the widget, and sync by themselves about every half hour.
- **Any S3-compatible store**: Cloudflare R2, AWS S3, MinIO, Backblaze B2. The store has to accept conditional writes
  (`If-Match` and `If-None-Match` on upload), which R2, MinIO and current AWS S3 do.

## Building it

You need the Android SDK and NDK (Android Studio installs both), Node.js 20+, Rust and
`cargo install cargo-ndk`. Then:

```bash
npm install
npm run build:android          # web app, the engine for the phone, the APK
adb install -r android/app/build/outputs/apk/debug/app-debug.apk
```

The script finds the SDK in `ANDROID_HOME` or `~/Library/Android/sdk`, uses the JDK 21 that comes with Android Studio,
adds the Rust targets it needs, and builds for `arm64-v8a` (phones) and `x86_64` (the emulator). It does not build for
32-bit ARM. Open `android/` in Android Studio to run it from there. The launcher icon and the splash screens are the web app's logo:
`node scripts/android-icons.mjs` makes them again from `web/public/logo.png` (it needs ImageMagick).

### Releases and signing

The **Release** workflow (a `v*` tag, or Actions → Release → Run workflow) also builds the APK, with the same version as
the web app ([Making a release](deploy.md#making-a-release-for-maintainers)): attached to the GitHub release for a tag, or
kept as the run's artifact (`android-apk`) otherwise. An installed app can only be updated by an
APK signed with the **same key**, so make one key and keep it:

```bash
keytool -genkeypair -keystore meghnaad.keystore -alias meghnaad -keyalg RSA -keysize 2048 -validity 10000
base64 < meghnaad.keystore | tr -d '\n'        # the value of the first secret below
```

Add these as repository secrets: `ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` (`meghnaad`)
and `ANDROID_KEY_PASSWORD`. Without them the workflow still builds, but a debug-signed APK that installs and cannot be
updated in place by the next one. Do not commit the keystore. Locally, `npm run build:android -- --release` signs with
the same variables set in the environment (`ANDROID_KEYSTORE_FILE` is the path).

## First run

The app asks for the bucket: the S3 endpoint (`https://<account>.r2.cloudflarestorage.com` for R2), the bucket, the
region (`auto` for R2, the real one for AWS), an access key and its secret, and the encryption secret (the same as
`sync.encryption_secret` in your taskrc). It tries them with a first sync and says what is wrong if they do not work.
Create the access key as in [Connect your `task` CLI](deploy.md#connect-your-task-cli): an R2 token with **Object Read &
Write**, scoped to the bucket.

The keys are kept encrypted with a key that lives in the Android Keystore. The page never sees them: it asks the app to
sync, and the app does.

### An example

| Field | Cloudflare R2 | AWS S3 | MinIO behind HTTPS |
| --- | --- | --- | --- |
| S3 endpoint | `https://0123456789abcdef.r2.cloudflarestorage.com` | `https://s3.eu-west-1.amazonaws.com` | `https://minio.example.com` |
| Bucket | `taskwarrior-sync` | `my-task-sync` | `tasks` |
| Region | `auto` | `eu-west-1` | `us-east-1` |
| Access key ID (the client ID) | `4f1c…` (32 characters) | `AKIA…` | your MinIO user |
| Secret access key (the client secret) | shown once when the token is made | shown once | your MinIO password |
| Encryption secret | the same as `sync.encryption_secret` in your taskrc | same | same |

These are the values your `task` CLI already has: `sync.aws.region`, `sync.aws.bucket`, `sync.aws.access_key_id`,
`sync.aws.secret_access_key` and `sync.encryption_secret`, plus the endpoint in `AWS_ENDPOINT_URL`. For R2 the endpoint
is `https://<account id>.r2.cloudflarestorage.com` (the account id is in the dashboard's right-hand column), and the key
pair is an R2 API token with **Object Read & Write** on the bucket.

The endpoint has to be `https://`; plain `http://` is only accepted for `localhost` and `127.0.0.1` (for trying it out).

## Different from the web app

- **Settings are on the phone.** The taskrc subset, the app's settings, the hooks script and the secrets for hooks live in
  the app's private storage, not in the bucket, so they are not shared with the web app. Import your taskrc again in
  **Settings** (pasting it works; importing from a link does not yet).
- **Hooks run on the phone**, in the same Lua sandbox, with the same `http` and `secret()`, from the script you edit in
  **Settings → Hooks**. Their network calls are made by the phone, and the secrets are sealed with the Keystore key.
- **No Cloudflare Access, no Worker.** Anyone who has the phone unlocked has the app: lock the phone.

## How it fits together

```
page (Svelte, in a WebView)
  │  the engine's messages, and /api/… requests      web/src/engine/native.ts
  ▼
TcEnginePlugin (Kotlin)  ── Engines: one engine for the page, the widget and the sync worker
  │  JNI                                              android/…/NativeEngine.kt
  ▼
crates/android (Rust): tc-core + the S3 store + the settings routes
  │  S3 over HTTPS, signed (SigV4)
  ▼
your bucket
```

- `crates/android/src/engine.rs` is the engine; `jni.rs` is the thin layer Kotlin calls. Every call answers in JSON.
- `crates/tc-core/src/request.rs` turns a request into a command and its answer for this engine and the browser's, so
  the two cannot differ. `webapi.rs` answers the settings routes (`/api/config`, `/api/settings`, …) for any store: the
  Worker keeps them in the bucket, the app in a folder.
- `s3.rs` is the bucket as tc-core's `ObjectStore`; `sigv4.rs` signs requests (tested against Amazon's own examples).
- In the app, `fetch('/api/…')` is answered by the phone (`installNativeApi`), so the page's code is the web app's.
- The replica is kept in a file in the app's storage after every change; the widget and the page share one engine, so
  what the widget changes shows in the app and the other way round.

Tests: `cargo test -p tc-android` (an in-process bucket stands in for S3), `web/src/engine/native.test.ts`, and the
widgets' field formatting and settings with `cd android && ./gradlew testDebugUnitTest`.
