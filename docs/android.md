# The Android app

A native app in Kotlin and Compose, with the engine as a native library and widgets on the home screen. It keeps your
tasks on the phone and syncs them directly with your store: a branch of a git repository on GitHub, Gitea or Forgejo, an
S3-compatible bucket, or a TaskChampion sync server. No Worker and no Cloudflare Access are involved, and your keys stay
on the phone. It does what the web app does, with a layout made for a
phone, plus two things a browser tab can't: reminders that come with the app closed, and widgets.

## What you get

- **Everything in the web app**: the console, the pages, review, urgency, recurrence, hooks and settings, in a layout
  made for a phone: a menu for the pages and tasks as cards ([On a phone](using/phone.md#on-a-phone)). Review is a
  card with buttons, and `review` in the console opens it too.
- **Works offline.** The engine and tasks are on the phone; changes sync when there is a network.
- **Pull down to sync.** At the top of any page but the console, pulling down syncs and redraws.
- **Any S3-compatible bucket**: Cloudflare R2, AWS S3, MinIO, Backblaze B2. The store must accept conditional writes
  (`If-Match` and `If-None-Match` on upload), which R2, MinIO and current AWS S3 do. Or a git repository, or a
  TaskChampion sync server ([First run](#first-run)).
- **Export and import** use the system's file pickers: `export` offers the file to keep, and `import` reads one and says
  what it would do before it writes anything.

### Tablets, foldables and keyboards

On a tablet, a foldable or a phone held wide (over 760 dp), a report is the web's table instead of cards:

- The report's own columns, with headers that sort (a long press adds a tie-breaker, like the web's Shift-click) and a
  button to group by the column, a tick box per row, and the row's buttons at the end. Columns take the width of what they
  hold and the description takes the rest. A report with more columns than fit scrolls sideways, with the tick box and
  buttons kept in place.
- Summary is a table, the calendar shows its months side by side, and the burndown grows to fill the width.
- A task's details open as a drawer on the right (560 dp, like the web's), the task form and review as a card in the
  middle, and Settings is no wider than the web's 820 dp.
- The menu isn't over the page: the list of reports is an always-open sidebar and the pages are tabs along the top (their
  names drop away below 1100 dp, and those of Review and Settings below 1400 dp). The floating **+** is the same.

With a keyboard attached, `/` focuses the prompt with its line selected (not while you're typing in
a field), Tab completes, ↑ and ↓ go through the last commands, Ctrl+L clears the console and Esc leaves the prompt. In
the Hooks editor Ctrl+S saves and Tab indents. In the task form Enter saves, and Ctrl+Enter saves from a multi-line field.
**Settings → Keybindings** changes the keys of the phone, the web app and the terminal app, as on the web
([Keybindings](using/settings.md#keybindings)).

### Reminders and links

#### Reminders

Reminders have the same options as the web app (Menu → Reminders), including a task's own `remind_in` times and the
button that adds that UDA, and work with the app closed. The phone wakes the
app when something is due or comes off `wait`, and the notification opens the task. Allow notifications when asked. The
once-a-day digest of what is overdue comes at the hour you chose for dates with no time.

#### Task links

The link button in a task's details copies `meghnaad://task/1a2b3c4d` (the first 8 characters of its
uuid, like the web's `/task/1a2b3c4d`). Opening it from a note or another app opens the task, on the phone that has the
task: the link names it and doesn't carry it.

#### Sharing a task

The share button in a task's details, beside the link button, hands the task to another app through Android's share
sheet, as one of three texts (always in this order):

- **Short text**, for a chat: the description, then its project, priority, due date and tags as the quick add reads
  them, so the text shared back into Meghnaad fills the same fields:

  ```
  Buy milk
  project:home priority:H due:2026-11-20 +errand +shop
  ```

- **Task JSON**: the object `task <uuid> export` writes for it (not the array around it), which `task import` takes.
- **Task info**: the text `task <uuid> info` prints, at Taskwarrior's usual width (`defaultwidth`), colour off. It is for
  reading; `task import` does not take it.

Mail apps get the description as the subject.

#### Page links

Every page has a `meghnaad://` link mirroring the web's addresses: `meghnaad://report/<name>`,
`meghnaad://board/<slug>`, `meghnaad://boards`, `meghnaad://settings/<tab>` (and `meghnaad://settings/taskrc/<path>` for
a taskrc file), the pages (`projects`, `tags`, `summary`, `calendar`, `burndown`, `console`) and
`meghnaad://task/<8 characters>`. A board has a button that copies its link. A link to a report or board that isn't
there says so and opens the home page.

### Boards

The boards you make on the web are in the menu under **Boards**, on 4 columns (6 on a wide phone), each card as wide as
its share of the row, so several numbers sit side by side. The same engine code works them out, so the numbers agree:
counts, sums and averages of numbers and durations, per value of an attribute or per day, week, month, quarter or year,
several series on one chart (stacked or side by side), and series that count pending, started and done at the end of
each period as `task burndown` does.

Boards are in `settings.json`, which can sync with the bucket or the git branch (**Settings → Sync**), so a board saved
on one device appears on the others after the next sync. **Settings → General** picks the page the app opens on: any report or board. **ⓘ** on every card shows its
filter (with **Open in Tasks**), the tasks that filter got and the values on its axis.

#### Charts

Bars, lines, areas, pies, donuts and radar are drawn by [Koala Plot](https://github.com/koalaplot/koalaplot-core) (a
polar area chart is drawn as a pie). A heatmap, which Koala Plot has no plot for, is a grid the app lays out itself: one
per series, or one for all when stacked. A tap on a cell says its row, column and value.

#### Editing a board

The pencil changes a board on the phone as the web does: its name, description, filter and address, and for each card its
title, type, filter, measure, grouping, order and series (colours, and which `task burndown` state they count). The board
is drawn as it will look.

- Tap a card to pick it, drag its corner to resize it in the phone's columns, and move it earlier or later with the
  arrows above.
- A card's settings open full screen. The engine says whether the draft is valid, and **Save** waits until it is.

### Widgets

Four, each with its own settings (long-press the widget, then its pencil; the same screen opens when you add one). They
redraw after any change, from the app or the widget, and sync by themselves about every half hour.

#### Tasks

The tasks of any report (built-in or yours), narrowed by a filter (`project:Home +work`), as many as you
choose, with a title of your own (left empty, it is named after the report: `Most urgent tasks` for `next`). Tap a
circle to mark a task done, the text to open **that task**, **+** to add a task typed as in the console (`Buy milk
due:tomorrow +home`), and the sync button to pull now. Under each task it shows the fields you tick: project, due,
priority, tags, urgency, age, scheduled, wait, recurrence, task number, notes.

#### Burndown

Daily, weekly or monthly bars of the tasks a filter picks, with finished at the bottom, then started, then
pending, and how long it would take to clear them at this pace.

#### Board card

One card of a board, chosen in the widget's settings: a number drawn large, or a chart (bars, lines,
areas, pies, donuts, radar, heatmaps) from the same numbers as the board, in the widget's light or dark look. A tap
opens the board. A table card isn't drawn (it says so).

#### Calendar

A month with today circled and a dot on days with something due (red overdue, amber today, blue after),
for the tasks a filter picks. The arrows go to other months, and the month's name goes back to this one. **A tap on a day
selects it** (a ring marks it) and, when the widget has room, shows that day's due or scheduled tasks: beside the month
when wide, under it when tall, and not at all when small, so resizing is how you choose. Two tasks are listed, each with
its time (or "scheduled") and project, and a tap opens the task. A **Show all** line opens the whole day in the app, as
the date above them does.

## Building it

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
version ([Making a release](deploy.md#making-a-release-for-maintainers)): attached to the GitHub release for a tag, or
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

### Updates

The app checks for a newer version itself: when it opens (at most once a day) it asks GitHub for the latest release of the
repository that built it, and says **"Meghnaad 0.6.36 is out"** once per new version. **Settings** has an **Updates**
section at the bottom of every tab:

- **Check for updates** works any time.
- **Download the APK** appears when a newer one is out. It opens the release's APK in the phone's browser, which downloads
  it and offers to install it over this app (Android asks once to let the browser install apps).
- **What's new** opens the release page.

Your tasks and connection stay, as long as the APK is signed with the same key. Only `https` addresses from the release are
opened, and nothing is sent to GitHub but the request for the release.

A build knows where its releases are only if it was told: the Release workflow passes `ANDROID_UPDATE_REPO` (its own
repository, so a fork's APK looks at the fork's releases). A hand-made build leaves it empty and shows no **Updates**
section; set `ANDROID_UPDATE_REPO=owner/name` when building to try it.

## First run

The app asks where your tasks sync, the same store your `task` CLI uses ([Where your tasks live](stores.md)): **S3
bucket**, **Sync server** or **Git**. Each asks for the encryption secret too (the same as `sync.encryption_secret` in
your taskrc), and tries what you entered with a first sync, saying what is wrong if it fails. The keys and tokens are kept
encrypted with a key in the Android Keystore. No screen can read them: the engine takes them to sync and nothing else
does.

### A git repository

Choose **Git** to sync with a branch of a git repository: the same branch `task` 3.5.0 syncs to with `sync.git.*`, and
the one the web app's static site and git Worker use, so they all share the tasks. The phone has no `git`; it uses the
forge's API.

| Field | What to put | In your taskrc |
| --- | --- | --- |
| Repository | `owner/name`, such as `alice/tasks` | the repository of `sync.git.remote` |
| Branch | `taskchampion` unless you chose another | `sync.git.branch` |
| API address | `https://api.github.com` for GitHub, `https://<host>/api/v1` for Gitea or Forgejo, `https://<host>/api/v3` for GitHub Enterprise | |
| Token | a token that may read and write the repository's contents | |
| Encryption secret | the same as `sync.encryption_secret` | `sync.encryption_secret` |

- **The token.** On GitHub, a fine-grained personal access token for that one repository with **Contents: Read and
  write**. On Gitea and Forgejo, an access token with the **repository** scope (read and write). It is kept in the
  Keystore with the other keys and never shown.
- **Keep the repository private.** The tasks are encrypted, but the app's settings and the hooks script go on the same
  branch, under `web/`, as plain text. `task` leaves them alone.
- **On GitHub the repository needs one commit** (a README will do) before the branch can be made; Gitea and Forgejo make
  it from nothing. A branch that isn't there yet is made on the first sync.
- The API address must be `https://` (`http://` only for `localhost` and `127.0.0.1`).

The settings and hooks go to the branch, so no bucket is needed for them: the two switches under **Settings → Sync** keep
them in step with the branch, as they would with a bucket.

### A bucket

Choose **S3 bucket** for any S3-compatible bucket. It asks for the S3 endpoint
(`https://<account>.r2.cloudflarestorage.com` for R2), the bucket, the region (`auto` for R2, the real one for AWS), and
an access key and its secret. For R2, the key is an R2 API token with **Object Read & Write**, scoped to the bucket
([an S3 or R2 bucket](stores.md#an-s3-or-r2-bucket)).

#### An example

| Field | Cloudflare R2 | AWS S3 | MinIO behind HTTPS |
| --- | --- | --- | --- |
| S3 endpoint | `https://0123456789abcdef.r2.cloudflarestorage.com` | `https://s3.eu-west-1.amazonaws.com` | `https://minio.example.com` |
| Bucket | `taskwarrior-sync` | `my-task-sync` | `tasks` |
| Region | `auto` | `eu-west-1` | `us-east-1` |
| Access key ID (the client ID) | `4f1c…` (32 characters) | `AKIA…` | your MinIO user |
| Secret access key (the client secret) | shown once when the token is made | shown once | your MinIO password |
| Encryption secret | the same as `sync.encryption_secret` in your taskrc | same | same |

These are the values your `task` CLI already has: `sync.aws.region`, `sync.aws.bucket`, `sync.aws.access_key_id`,
`sync.aws.secret_access_key`, `sync.encryption_secret` and `sync.aws.endpoint_url` (with `task` 3.5, the endpoint is in
`AWS_ENDPOINT_URL`). The app always puts the bucket in the path, so `sync.aws.force_path_style` has no field. For R2 the account
id in the endpoint is in the dashboard's right-hand column.

The endpoint must be `https://`; plain `http://` is accepted only for `localhost` and `127.0.0.1` (for trying it out).

### A sync server

Choose **Sync server** to use a TaskChampion sync server (`taskchampion-sync-server`, or anything speaking its
protocol). It takes three things, the values your `task` CLI already has:

| Field | In your taskrc |
| --- | --- |
| Server address, such as `https://tasks.example.com` | `sync.server.url` |
| Client ID, a UUID the same on every device that shares the tasks (**Generate** makes one for a new set) | `sync.server.client_id` |
| Encryption secret | `sync.encryption_secret` |

The address must be `https://` (`http://` only for `localhost` and `127.0.0.1`). Tasks are sealed on the phone before they
are sent, so the server holds only what it can't read, and the client ID is kept in the Keystore like the keys.

The web app can't reach a sync server, so tasks that sync to one don't reach the web app. A sync server doesn't hold the
app's settings and hooks either. **A bucket for those files is optional**: tick *Also keep settings and hooks in a
bucket* and give it a bucket (a different one from the web app's tasks is fine), and the two switches under **Settings →
Sync** keep them in step, as below. Without one they stay on the phone.

### Syncing somewhere else

A phone remembers where its tasks were last synced. If you point it at a different place (another repository, bucket or
server, or another kind of store), the new place doesn't have the version the phone stands on, so the app **asks first**:

- **Keep them and add them there**: the phone's tasks are added to what the new place holds (the same tasks aren't
  duplicated), and everything is then in both.
- **Start from what is there**: the phone's tasks are let go, and only what the new place holds is kept.
- **Cancel** changes nothing.

Nothing on the phone changes until the new place has answered, so a wrong address or secret leaves it as it was. A phone
with no tasks moves without asking.

## Different from the web app

### Settings live on the phone

Unless you turn sharing on, the taskrc subset, the app's settings, the hooks script and the hook secrets are in the
app's private storage. Under **Settings → Sync** (when connected) you can keep **the
settings** (the taskrc files under `web/rc/`, `web/config.json`, `web/settings.json`) and/or **the hooks**
(`web/hooks.lua`) in step with the bucket or the git branch. They are the files the web app keeps there, so an edit on
either side reaches the other at the next sync.

- What changed since the last sync wins. When both changed, the newer wins and the other is kept as the web app's restore
  point (`config.prev.json`, `hooks.prev.lua`; for the taskrc files, `rc.prev.json`, all together).
- A taskrc file deleted on one side and untouched on the other is deleted on both.
- A file that won't load, or a settings or taskrc file holding a credential, is not taken from the bucket or the branch.
- **The hook secrets are never synced.** The two switches are this phone's own choice, kept on the phone and not synced.

### Without sharing

Import your taskrc again in **Settings → taskrc**: paste it, or use **+ New** to upload files or a whole folder or
to import from a link (a bottom sheet). The tab is laid out as on the web, with the list of files folded away and opened
over the editor ([Your taskrc](using/settings.md#your-taskrc)). **Download** (in the **⋯** menu by the file's name)
and **Download all (.zip)** keep a file or the whole folder through the save dialog. **Add holidays** (under **+ New**,
in a bottom sheet) fetches a country's holidays from holidata.net or
date.nager.at, or a calendar (`.ics`) from a link, with a plain HTTPS request from the phone (a calendar's link and every
redirect are held to the rules the Worker applies to a taskrc link), and saves them as a file in the taskrc folder.

### Settings → Sync

This shows where the phone syncs (the repository, branch and API address; the endpoint, bucket, region and a hint of the
access key's name; or the sync server and a hint of the client ID; never a secret or a token) and changes it. Blank
fields keep their value, so you can correct the address without typing the keys again. The change is tried with a sync first, and one that doesn't work
isn't kept.

### Hooks run on the phone

They run in the same Lua sandbox with the same `http` and `secret()` (each secret going only to the hosts you saved it
with, so a script that arrives from the bucket or the branch can't send the phone's secrets anywhere else), from the
script you edit in **Settings → Hooks**. The phone makes their network calls, and the secrets are sealed with the Keystore key.

### No Cloudflare Access, no Worker

Anyone with the phone unlocked has the app: lock the phone.

## How it fits together

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
