# The Android app

A native app, in Kotlin and Compose, with the engine as a native library and widgets on the home screen. It keeps your
tasks on the phone and syncs them straight with your bucket over S3, or with a TaskChampion sync server: no Worker and no
Cloudflare Access are involved, and your keys stay on the phone. It does what the web app does, with a layout made for a phone, and a few things a browser tab
cannot: reminders that come with the app closed, and the widgets.

> **Status:** it builds, its Rust half is tested (signing, sync against a bucket that enforces conditional writes,
> settings, hooks), its Kotlin half has unit tests (the screens' logic, and commands, review and import through the real
> engine), and it has been tried on an Android 16 emulator: connecting, the pages, editing, review, reminders, the widgets,
> importing and exporting. It has not been run on a physical phone yet, so expect rough edges.

## What you get

- Everything in the web app: the console, the pages, review, urgency, recurrence, hooks, settings, with a layout made
  for a phone: a menu for the pages, and tasks as cards ([On a phone](using.md#on-a-phone)). Review is a card with
  buttons, which `review` in the console opens too.
- **A table where there is room.** On a tablet, a foldable or a phone held wide (more than 760 dp across), a report is
  drawn as the web's table, not as cards: the report's own columns, headers that sort (a long press adds a tie-breaker,
  the web's Shift-click) with a button to group by the column, a tick box on each row and the row's buttons at the end.
  Columns take the width of what they hold and the description the rest; a report with more columns than fit scrolls
  sideways, with the tick box and the buttons kept in place. Narrower than that you get the cards. Summary is a table too, the calendar shows its months side by side and the
  burndown chart grows to fill the width. A task's details open as a drawer on the right (560 dp, the web's), the task form
  and the review as a card in the middle, and Settings is no wider than the web's 820 dp. The menu is not over the page
  there: the list of reports and the quick add are a sidebar that is always open, the pages are tabs along the top (their
  names drop away below 1100 dp, those of Review and Settings below 1400 dp), and there is no floating +.
- **With a keyboard attached**, the web's keys work: `/` focuses the prompt with its line selected (not while you are typing
  in a field), Tab completes, ↑ and ↓ go through the last commands, Ctrl+L clears the console and Esc leaves the prompt;
  in the Hooks editor Ctrl+S saves and Tab indents; in the task form Enter saves and Ctrl+Enter saves from a field that
  takes several lines.
- **Pull down to sync.** At the top of any page but the console, a pull down syncs with the bucket and redraws the page.
- **Reminders that work with the app closed.** The same options as the web app's (Menu → Reminders); the phone wakes the
  app at the moment something is due or comes off `wait`, and the notification opens the task. Allow notifications when
  asked. The one-a-day digest of what is overdue comes at the hour you chose for dates with no time.
- **Task links.** The link button in a task's details copies `meghnaad://task/1a2b3c4d` (the first 8 characters of its
  uuid, as the web app's `/task/1a2b3c4d`); opening that link, from a note or another app, opens the task. It works on
  the phone that has the task: the link names the task, and does not carry it.
- **Export and import** use the system's file pickers: `export` offers the file to keep, `import` reads one and says what
  it would do before it writes anything.
- **Works offline.** The engine and the tasks are on the phone; changes sync when there is a network.
- **Home-screen widgets**, three of them, each with its own settings (long-press the widget, then its pencil; the same
  screen opens when you add it):
  - **Tasks**: the tasks of any report, the built-in ones and your own, narrowed by a filter (`project:Home +work`), as
    many as you choose, with a title of your own (left empty, the widget is called what its report is described as: `Most urgent tasks` for `next`). Tap a circle to mark a task done, the text to open **that task** in
    the app (open or not), **+** to add a task by typing it as in the console (`Buy milk due:tomorrow +home`), and the
    sync button to pull now. Under each task it shows the fields you tick: project, due date, priority, tags, urgency,
    age, scheduled, wait, recurrence, task number, notes.
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
npm run build:android          # the engine for the phone, then the APK
adb install -r app/androidApp/build/outputs/apk/debug/androidApp-debug.apk
```

The script finds the SDK in `ANDROID_HOME` or `~/Library/Android/sdk`, uses the JDK 21 that comes with Android Studio,
adds the Rust targets it needs, and builds for `arm64-v8a` (phones); a debug build also has `x86_64` (the emulator on an
Intel machine), and `ANDROID_ABIS` chooses. It does not build for 32-bit ARM.

**Size.** The library has its symbols taken out (`profile.android` in `Cargo.toml`) and is packed compressed, a release is
for arm64 only, and R8 cuts the unused code and resources (`app/androidApp/proguard-rules.pro` keeps what is looked up by
name: the engine's functions). If you add a class that something finds by name, add it there, and try the release build,
not only the debug one. Open `app/` in Android Studio to run it from there; `app/README.md` says how to look at the
screens live on a computer. The launcher icon is the web app's logo: `node scripts/android-icons.mjs` makes it again from
`web/public/logo.png` (it needs ImageMagick).

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

The app asks where your tasks sync: **an S3 bucket** or **a sync server**.

For a bucket, it asks for the bucket: the S3 endpoint (`https://<account>.r2.cloudflarestorage.com` for R2), the bucket, the
region (`auto` for R2, the real one for AWS), an access key and its secret, and the encryption secret (the same as
`sync.encryption_secret` in your taskrc). It tries them with a first sync and says what is wrong if they do not work.
Create the access key as in [Connect your `task` CLI](deploy.md#connect-your-task-cli): an R2 token with **Object Read &
Write**, scoped to the bucket.

The keys are kept encrypted with a key that lives in the Android Keystore. No screen can read them: the engine takes them
to sync, and nothing else does.

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

### A sync server

Choose **Sync server** to use a TaskChampion sync server (`taskchampion-sync-server` or anything that speaks its protocol)
instead of a bucket. It takes three things, the values your `task` CLI already has:

| Field | In your taskrc |
| --- | --- |
| Server address, such as `https://tasks.example.com` | `sync.server.url` |
| Client ID, a UUID the same on every device that shares the tasks (**Generate** makes one for a new set of tasks) | `sync.server.client_id` |
| Encryption secret | `sync.encryption_secret` |

The address has to be `https://` (`http://` only for `localhost` and `127.0.0.1`). The tasks are sealed on the phone
before they are sent, so the server only holds what it cannot read; the client ID is kept in the Keystore like the keys.

The web app and the Worker sync with the bucket only, so tasks that sync to a sync server do not reach the web app. A
sync server does not hold the app's settings and hooks either. **A bucket for those files is optional**: tick *Also keep
settings and hooks in a bucket* and give it a bucket (a different one from the web app's tasks is fine), and the two
switches under **Settings → Sync** keep the settings and the hooks in step with it, as below. Without one, they stay on
the phone.

### Syncing somewhere else

A phone remembers where its tasks were last synced. If you set it to sync to a different place (another server, another
bucket, or from a bucket to a server) the new place does not have the version the phone stands on, so the app **asks first**:

- **Keep them and add them there**: the tasks on the phone are added to what the new place holds (the same tasks
  are not duplicated), and everything is then in both.
- **Start from what is there**: the tasks on the phone are let go, and only what the new place holds is kept.
- Cancel changes nothing.

Nothing on the phone changes until the new place has answered, so a wrong address or secret leaves the phone as it was. A
phone with no tasks goes to another place without asking.

## Different from the web app

- **Settings are on the phone**, unless you turn sharing on. The taskrc subset, the app's settings, the hooks script and
  the secrets for hooks live in the app's private storage. Under **Settings → Sync** (when connected) you can keep **the
  settings** (`web/config.json`, `web/settings.json`) and/or **the hooks** (`web/hooks.lua`) in step with the bucket: they
  are the files the web app keeps there, so an edit on either side reaches the other at the next sync. What changed since
  the last sync wins; when both did, the newer one does and the other is kept as the web app's restore point
  (`config.prev.json`, `hooks.prev.lua`). A file that would not load, or a settings file holding a credential, is not taken
  from the bucket. **The secrets for hooks are never synced.** The two switches are this phone's own choice, kept on the
  phone, and are not synced themselves. Without sharing, import your taskrc again in **Settings** (paste it, or import it from a link
  or a file).
- **Settings → Sync** shows where the phone syncs to (the endpoint, the bucket, the region and a hint of the access key's
  name, or the sync server and a hint of the client ID; never a secret) and changes it: blank fields keep their value, so the address can be corrected without typing the
  keys again, and the change is tried with a sync first. A change that does not work is not kept.
- **Hooks run on the phone**, in the same Lua sandbox, with the same `http` and `secret()`, from the script you edit in
  **Settings → Hooks**. Their network calls are made by the phone, and the secrets are sealed with the Keystore key.
- **No Cloudflare Access, no Worker.** Anyone who has the phone unlocked has the app: lock the phone.

## How it fits together

```
screens (Compose)                                      app/shared: Store, one file per page
  │  JSON in and out                                   Engine.kt (mirrors web/src/lib/api.ts)
  ▼
LocalEngine (Kotlin)  ── AppEngine: one engine for the screens, the widgets, the reminder alarm and the sync worker
  │  JNI                                               NativeEngine.kt (package me.utsob.meghnaad)
  ▼
crates/android (Rust): tc-core + the S3 store + the settings routes
  │  S3 over HTTPS, signed (SigV4)
  ▼
your bucket
```

- `crates/android/src/engine.rs` is the engine; `jni.rs` is the thin layer Kotlin calls. Every call answers in JSON.
- `crates/tc-core/src/request.rs` turns a request into a command and its answer for this engine and the browser's, so
  the two cannot differ. `webapi.rs` answers the settings routes (`/api/config`, `/api/settings`, …) for any store: the
  Worker keeps them in the bucket, the app in a folder; the app asks them of the engine's `api` call.
- `s3.rs` is the bucket as tc-core's `ObjectStore`; `sigv4.rs` signs requests (tested against Amazon's own examples).
- `syncserver.rs` is a TaskChampion sync server as a `taskchampion::Server` (`/v1/client/…`, `X-Client-Id`, the key derived
  with the client id as the salt). `engine.rs` chooses between it and the bucket for the tasks, keeps a bucket of its own
  for the shared files, and remembers where the tasks were last synced: another place makes `sync` fail with
  `backend-changed` until it is asked again with `switch` (`merge` or `replace`), and the replica of the new place is made
  aside and swapped in only when it has synced. `tests/syncserver.rs` runs the engine against a server kept in the test, and,
  with `--ignored` and `TC_SYNC_SERVER`, against the real `taskchampion-sync-server` and the `task` CLI.
- The replica is kept in a file in the app's storage after every change; the screens, the widgets and the reminder alarm
  share one engine, so what a widget changes shows in the app and the other way round.
- Reminders: `Notify.kt` is the web's rules (`notify.ts`); after every change the next moment something is worth saying is
  worked out, `AlarmManager` is set for it, and its receiver shows what is due and sets the next one (also after the phone
  starts). The background sync (WorkManager, every half hour) runs while a widget is placed or reminders are on.

Tests: `cargo test -p tc-android` (an in-process bucket stands in for S3), and in `app/`
`./gradlew :shared:desktopTest :shared:testDebugUnitTest :androidApp:testDebugUnitTest` (build the engine for this machine
first: `cargo build --profile android -p tc-android`; the tests that run the real engine skip without it).
