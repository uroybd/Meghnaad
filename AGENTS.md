# Working on Meghnaad

Taskwarrior 3.5.0 in the browser: a pure engine (`crates/tc-core`) that runs **in the page** as WebAssembly
(`crates/browser`, in a Web Worker, with its replica saved in IndexedDB), a Svelte 5 app (`web/`) that draws what it
returns, a small Rust → WASM Cloudflare Worker (`crates/worker`) that checks Access, holds the secret and opens and seals
versions, and an R2 bucket as the TaskChampion store. The README and `docs/architecture.md` explain the design; this
file is how to change it.

## Where a feature goes

Two layers. Ask which one the feature is about.

1. **Taskwarrior and TaskChampion** (`crates/tc-core`, carried by `crates/browser`, `crates/worker`, `crates/android` and
   the bucket). It does what the real `task` 3.5.0 CLI does, command for command: what a filter, report, date, urgency,
   recurrence, confirmation, `calc`, setting or hook means, and the sync format. Check against the real `task`; where it
   is odd, so are we. The Worker, the browser engine, the phone library and the bucket only host this layer (storage,
   sync, the secret, running hooks) and add no behaviour.
2. **UI** (`web/`, `app/`). Everything a TUI or a shell adds around `task`, and everything custom: screens, the console,
   task commands, `&&` `||` `;` chains, bulk actions, review, reminders, themes, routes. It uses the task CLI as its
   backend to our instance: it sends command lines (and `_` hidden commands, as Taskwarrior has) and draws what comes
   back. It never reaches into the replica and never re-implements layer 1.

Test: would the real `task` do this with the same command line? Layer 1. Would a person do it with a shell alias, tasksh
or a TUI around `task`? Layer 2.

- A UI feature never changes how the engine runs a command. If it needs structured data, use `<filter> export`, which
  gives Taskwarrior's own JSON, and work from that in the UI; `_rows` is fine too when it already has everything the
  screen needs. Don't add another command of our own for a screen. The existing app-only ones (`_rows`, `_history`,
  `_remind`, `_names`) stay as they are; nothing needs migrating.
- `tc_core::helpers` (a date in `dateformat`, a line split as the engine reads it, completion, a report's cells) is
  layer 1 knowledge handed to both UIs, so they never copy it. Shell-level logic does not go there, with the exceptions
  below.
- Layer 2 is per front end: the web app's logic is ported to Kotlin with its tests, not shared through Rust. Two
  exceptions, because they are pure functions both UIs would otherwise write twice and drift apart on: the header-sort
  helpers (`sortspec`) and boards (`tc_core::board`: what a saved board means, the numbers a section shows from an
  `export`, and the grid layout), both reached through `helpers`. Sorting and trimming a chart's groups, and all drawing,
  stay in each UI.
- Charts are drawn by a charting library on each front end (Chart.js on the web), never by hand; the table is the one thing
  that stays ours, and the one exception is the heatmap on Android, a grid of cells (`Heatmap` in `BoardCharts.kt`), because Koala
  Plot has no plot for it. The web draws it with Chart.js's matrix plugin. The engine's `Chart` (labels and a dataset for each series, `tc_core::board::aggregate::chart`) is the
  shared contract, shaped like Chart.js's `data`: a front end maps it to its library and never computes chart data itself.

## Before you say a change is done

Format, lint and test, in that order. CI runs the same checks and fails on any of them.

```bash
npm run format                # clippy --fix (the fixes it can make itself), cargo fmt (rustfmt.toml, 120 columns) and
                              # Prettier (.prettierrc.json): fixes in place. `fix:rust` is the clippy half
npm run lint                  # Rust: rustfmt check + clippy -D warnings (engine, and the Worker and browser engine for wasm32)
                              # JS/TS/Svelte: Prettier check + ESLint (eslint.config.js)
cargo test                    # engine, end-to-end command tests, sync cost, memory budgets
npm --prefix web test         # web unit tests
npm run check                 # svelte-check (types, accessibility)
node --test scripts/*.test.mjs  # the build and deploy scripts
cargo check -p tw-worker --target wasm32-unknown-unknown
cargo check -p tc-browser --target wasm32-unknown-unknown
```

`npm run lint:rust` and `npm run lint:js` run each half alone. Everything is formatted and linted, Rust and web alike:
run `npm run format` after editing any code, not just at the end.

- Clippy is clean; keep it so. Fix a warning rather than allowing it. An `#[allow]` needs a reason, written as
  `reason = "..."` (the existing `too_many_arguments` ones are for the async command functions that thread the replica
  and its context).
- The lints are set once for the workspace (`[workspace.lints]` in `Cargo.toml`, inherited by each crate with
  `[lints] workspace = true`; `clippy.toml` keeps `#[test]` code out of the panic-family lints) and CI makes every one an
  error. Beyond clippy's defaults they forbid `unwrap`, `expect`, `panic!`, `todo!`, `dbg!`, printing, and `let _ =` on a
  `#[must_use]` value in code that ships: return the error, or fall back as the function already does, and where a
  value truly cannot be missing say so with a pattern (`let Some(x) = .. else`) rather than a panic. An integration test
  file under `tests/` opens with one `#![allow(.., reason = "test code: a failure is the point")]`. To ignore a result
  on purpose use `.ok();` with a comment saying why. Add to the list when a new lint earns its place; don't remove one to
  get a change through.
- ESLint is clean too. Fix a finding rather than disabling it; a `// eslint-disable-next-line` needs a reason after `--`.
  Two Svelte rules are off in `eslint.config.js`, each with the reason beside it (`prefer-svelte-reactivity`,
  `no-unused-svelte-ignore`); don't add to that list to get a change through.
- A Svelte `{#each}` needs a key. Use something unique (an id, a name); for display-only lists whose values can repeat
  (table headers, cells, lines), key by index, because a duplicate key breaks the list.
- A bare `store.x;` inside `$effect` to subscribe to it is written `void store.x;`.
- Don't hand-format around the formatters. Markdown in `docs/` and the README is hand-wrapped and not run through Prettier.
- If a change touches sync, the Worker or crypto, also run `./scripts/interop-local.sh` (needs the `tw-s3` Docker
  container and a built Worker; see the README).

## Conventions

- **Follow Taskwarrior 3.5.0, not a lookalike.** For behaviour (filters, dates, reports, confirmations, `calc`, settings)
  read the C++ first and check against the real `task` run on a scratch `TASKRC` and data directory. Don't guess.
- **Every page has an address** (`web/src/lib/route.ts`, `Place`): a new page or view adds its place, a round-trip test and a
  row in the address table of `docs/using.md`, so it can be linked and the back button works.
- **Docs move with the code.** A new or changed feature updates `docs/using.md`, and `docs/taskrc-support.md` for a
  setting, `docs/architecture.md` for a module, and the README if it is user-visible. Keep the Mermaid diagram identical
  in the README and `docs/architecture.md`.
- **Taskwarrior logic lives in `tc-core`**; the page only draws. The engine runs in the page (`crates/browser`); the
  Worker runs no commands and holds no tasks. The page talks to the bucket only through the Worker's `/api/tc/*`
  (`tc_core::wire`). To run commands from a shell (seeding, interop, poking a deployment) use
  `node scripts/engine-cli.mjs --api URL '<command>'`, which runs the same engine under Node.
- **Secrets never leave the Worker.** The encryption secret is only ever in the Worker (it derives the key and opens
  and seals versions; the page gets plain versions over HTTPS). The taskrc importer, `show` and `config` refuse sync and
  credential-like names and never echo their values, and `PUT /api/config/engine` refuses them too; keep tests that
  prove it passing, and add one for any new path that handles settings.
- **Mind the sizes.** Both are compressed WebAssembly. The engine is downloaded by every visitor (about 669 KB; measure
  with `gzip -9 -c web/src/engine/pkg/tc_browser_bg.wasm | wc -c` after `npm run build:engine`). The Worker (about 660 KB with the Lua and its network;
  `gzip -9 -c crates/worker/build/index_bg.wasm | wc -c`) has Cloudflare's 1 MB limit. Every feature spends some: measure a release build before and after, and prefer reusing what
  is already compiled in (an existing `BTreeSet<String>`, `serde_json::from_slice`, the `config` line editing) over a
  new instantiation. `twiggy` on a build made in a scratch directory shows where bytes go. The builds drop the debug
  names (`KEEP_WASM_NAMES=1` keeps them); measure with them left out, as the deploy does.
- **Hooks** are user code, and not Rust: a Lua script the user edits in Settings, kept in the bucket and run on the Worker
  (`tc_core::lua_hooks`, behind the `lua` feature the Worker turns on; the browser engine does not carry it). The engine
  calls hooks through the async `HookHost`; `RemoteHooks` (`crates/browser`) is the one that asks the Worker. A hook is a
  task in and a task out as JSON, so change `task_to_json` / `task_from_json` and `lua/prelude.lua` together, and keep the
  tests in `crates/tc-core/tests/lua_hooks.rs` passing (run them with `cargo test -p tc-core --features lua`). The Lua
  must stay a sandbox: nothing in the prelude or the host functions may reach a file, or a real clock. The network and secrets
  exist only through the `Net` a host passes in (the Worker's `hooknet.rs`); the runner, not the host, enforces the
  allowlist, https, the request limits and the redaction of a secret's value, and the tests for that are `tests/lua_net.rs`.

## Releases

The Release workflow builds once and publishes a prebuilt package (`scripts/package-release.mjs`) to the `release`
branch and, for a tag, a GitHub release: that is what the Deploy to Cloudflare button and `npm run setup` in a release
clone use, with no Rust. Nothing user-specific may be compiled into the build (hooks, secrets and config live in the
bucket), and `setup.mjs`, `deploy.mjs` and `deploy-lib.mjs` must keep working in a release, which has no `crates/`
(`isSourceCheckout`). A new file the deploy needs at run time has to be added to `assemble`, with a test.

## Versions

One version, the root `package.json`'s: the web app's label in Settings (`__APP_VERSION__`), the Android versionName and
versionCode (`scripts/version.mjs`), and the prebuilt package. Release by changing it, committing, and pushing the tag
`v<version>` (the Release workflow refuses a tag that does not match). Don't hard-code a version anywhere else.

## The Android app

`crates/android` is `tc-core` natively (S3 sync with its own SigV4, a TaskChampion sync-server client, the settings routes over a folder, hooks on the
phone) behind a JNI layer; `app/` is the app that loads it (below); see `docs/android.md`. `node scripts/build-android.mjs`
builds the APK. Anything the screens need from a server must also be answered by the app (the engine's `api` call answers
`/api/…` on the phone), so a new settings route goes in `tc_core::webapi` where the Worker and the app share it. A request's meaning is in `tc_core::request`, shared with the browser engine: change it there, not in
one host. Nothing may put the bucket keys or the encryption secret anywhere a screen can read them. The few files a phone may keep
in step with the bucket (settings, hooks; never secrets) are listed in `crates/android/src/filesync.rs`, which checks what it
takes from the bucket the way the app checks what a person enters.

## The Kotlin app

`app/` is the Android app, in Kotlin and Compose (see `app/README.md`): `shared` has the engine's types and API, the
state and the screens; `androidApp` is the activity, the reminder alarm and the widgets. It loads the `crates/android`
library, so `NativeEngine` keeps its package (`me.utsob.meghnaad`) for the JNI names. The page-side rules hold here too: Taskwarrior
behaviour is the engine's, and the screens only draw what it returns; mirror `web/src/lib/types.ts` and `api.ts` when the
engine's answers change. **Never port Rust to Kotlin:** where `tc-core` already does something (splitting a command line,
writing a date in `dateformat`, `format_vague`, completion, highlighting, urgency terms, a report's cells, Markdown) call it through `tc_core::helpers` (`crates/android/src/helpers.rs` re-exports it for
JNI, and the page calls the same functions through `web/src/engine/helpers.ts`), and add a function there when a screen
needs another; only the web app's own view logic (`web/src/lib`: task commands, bulk, reminders, the screens' state) is ported,
with its tests. `./gradlew :desktopApp:hotRun --auto` (see `app/README.md`) opens a live preview of the screens. Colours the user sets in the taskrc are drawn as a subtle tint, never as full colour. Check it with
`cd app && ./gradlew :shared:testDebugUnitTest :androidApp:assembleDebug` (build the engine for the host first:
`cargo build --profile android -p tc-android`); CI runs the same on the `kotlin-app` job. It has no formatter or linter in CI yet.

## Running it

`npm run dev` is the dev server: it builds the engine (`scripts/build-engine.mjs`, which needs the `wasm-bindgen` tool at
the version in `Cargo.lock` and installs it once), rebuilds it when Rust under `crates/tc-core` or `crates/browser`
changes, runs `wrangler dev --local` (8787) and Vite (5273, hot reload, `/api` proxied). `--api-port`, `--web-port` and
`--persist-to` move it. The generated engine package (`web/src/engine/pkg`) is git-ignored, formatted and linted by
nobody; `web/src/engine/tc-engine.d.ts` types it by hand, so keep it in step with `crates/browser/src/lib.rs`.
`npm run build:web` and `npm run build` build the engine first. The engine in the page does not exist under Vitest
(jsdom has no Web Worker), so `api.ts` takes the server path there unless a test mocks `../engine/client.svelte`. The page's
helpers are different: they are the engine's WebAssembly called at once from the page's thread (`web/src/engine/helpers.ts`:
dates in the user's `dateformat`), so the web tests load the built engine (`web/src/test-setup.ts`), and
`npm --prefix web test` builds it first when it is out of date (CI installs the Rust toolchain before them for this).

## Running things without disturbing the person using the app

The user may have dev servers running and real tasks in them (`wrangler dev` on 8787, Vite on 5273; 5173 is an
unrelated app). Never kill servers broadly, never write test tasks into their data.

- Test anything that writes on an isolated stack: build, copy `crates/worker/build` and `web/dist` somewhere scratch,
  write a small `wrangler.jsonc` there (no `build` command), and run
  `wrangler dev --local --config <that> --port <unused> --persist-to <scratch>/state`. Stop only your own processes.
- Keep scratch files (copies, screenshots, throwaway scripts) out of the repo.
- Reads against the running server (`count`, `next`) are fine.

## Git

Don't commit or push unless asked. When asked, use a conventional message (`feat:`, `fix:`, `doc:`, `ci:`) that says what
and why, and keep a formatting-only change in its own commit.
