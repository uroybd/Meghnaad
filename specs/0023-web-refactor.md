# 0023: The web app's settings reads in one place, its largest files split, and pages loaded on demand

- **Status:** in progress <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-19
- **Layer:** 2 (UI)
- **Front ends:** web. Android is left out: its settings are read from local files, which cost nothing to read again
  (see Open questions). The terminal has no settings screens of this kind.
- **Supersedes / related:** 0013 (static web app), 0021 (the taskrc tab as a workspace); the forge request fixes of
  v0.8.16–0.8.18 (`fix(static): …`, `fix(git): …`)

## Why

The settings page of the static web app sent the same request to GitHub many times over, enough to worry about the
forge's rate limits. v0.8.16–0.8.18 made each request cheaper in the engine: files are kept by blob id, a folder is read
in one GraphQL query, and a read repeated within five seconds is answered again. The cause is in the page, though. Each
screen fetches what it shows for itself, in an `$effect` that runs again whenever `store.config` changes. One save, or
one `config` typed in the console, then makes every panel on screen fetch again on its own, and the settings route each
calls reads the whole taskrc folder. The Worker flavour has the same pattern; there it costs R2 reads instead of API
quota.

The app has grown to 87 components (about 18,000 lines) and 83 TypeScript modules (about 11,000). That is not too much
for Svelte. But three files carry a lot: `ReportTable.svelte` (1,183 lines), `TaskrcPanel.svelte` (935) and
`store.svelte.ts` (976, 46 members across settings, console, task actions and navigation). They are hard to change
safely, and each change risks the next fetch cascade.

The app also loads more than it shows. Its main bundle (`App-*.js`, 144 KB gzip, measured 2026-11-19) holds every page
(boards, burndown, calendar, summary, projects, tags, console) and every settings tab, though a visit usually opens one
or two of them. Only the editors (CodeMirror, about 104 KB), the charts (Chart.js, 66 KB) and the static app's lock
screen are loaded on demand today.

## What

Nothing changes on screen: every page, address, key and command works as it does now. What changes:

- The settings page asks for each thing once. Opening Settings, switching tabs, opening a file in the taskrc tab, or
  saving makes at most one request per settings route. After a save, only what the save changed is read again.
- Something saved elsewhere is still followed: `config` or `context` in the console, another tab, or a change pulled
  from the branch. The page then reads again only what changed.
- The three largest files are split into smaller ones by what each part does, with no change in behaviour.
- The app opens faster: the first visit downloads the page you land on, not every page and every settings tab. The
  other pages load when first opened, or quietly in the background once the app is idle, so they open without a wait.

## Not doing

- No other framework, and no new runtime dependency (no query library): the shared cache is a small module of our own.
- No change to the engine, the Worker, the routes (`tc_core::webapi`) or the sync. The engine-side reuse from v0.8.17
  stays (see Open questions).
- No change to task sync or its 30-second pull (decided 2026-11-19).
- No restyling and no new features while splitting. A bug found while splitting is fixed in its own commit, not folded in.
- Android and the terminal are not ported or changed.

## Design

Proposal, in three steps. Each step is its own commit (or a few) and can ship alone; the tests that pass before it pass
after it.

### 1. One shared cache for settings reads

A new module, `web/src/lib/settingsData.svelte.ts`, holds one entry per settings route the page reads:

- `/api/config`: the settings the engine runs on (today `store.loadConfig`);
- `/api/config/taskrc`, `/api/config/files` and `/api/config/files/<path>`: the taskrc folder;
- `/api/settings`: `settings.json`;
- the hooks and the hook secrets' names.

Each entry is reactive state (`value`, `error`, `loading`). Its `get()` starts a fetch only when there is no value and
none is in flight; callers at the same moment share one promise. `invalidate(...keys)` marks entries stale, and an
entry being shown is fetched again once. Its value stays on screen meanwhile, so nothing flickers.

- **Writes go through the module.** Each `put…`/`import…` in `api.ts` that the panels call gets a wrapper there. The
  wrapper writes, puts the answer in the entry where the route gives one back, and invalidates only what the write can
  change. Saving one rc file, for example, makes the folder list and `/api/config` stale; the other files are only made
  stale when the import replaced them.
- **"Settings changed elsewhere" becomes an invalidation.** `engine.onSettingsChanged`, the console's `config` and
  `context`, and `store.settingsChanged` mark the affected entries stale instead of reloading them. `store.config`
  becomes a read of the `/api/config` entry, so the many components reading `store.config?.config…` don't change.
- **The panels read entries instead of fetching.** `TaskColours`, `TaskrcPanel` and `HooksPanel` read the entries and
  lose their `$effect` fetches on `store.config`. `TaskrcPanel` keeps its drafts: an entry going stale while a file has
  unapplied edits doesn't touch the draft, as today.

### 2. `store.svelte.ts` split by area

The single `store` stays as the one object components import, so no component changes. Its parts move into modules
it puts together:

| module | holds |
|---|---|
| `store/settings.svelte.ts` | the config (through `settingsData`), app settings, keys, boards, homepage, completion names |
| `store/console.svelte.ts` | entries, `run`, chains, `#exec`, prompts, redaction |
| `store/tasks.svelte.ts` | `refresh`, `act`, `actSteps`, `confirm`, `tickBox`, `answerItems`, `written`, live panels |
| `store/navigation.svelte.ts` | `openSettings`, `goHome`, `openDetail`, `openFromAddress`, `focusReport`, the view |

`store.svelte.ts` keeps the class's public members as forwarding getters and methods (or plain delegation where Svelte
5's reactivity allows it), so `store.x` reads as before.

### 3. The two largest components split

- **`ReportTable.svelte`:**
  - `ReportRows.svelte` (the table and the cards);
  - `RowActions.svelte` (a row's buttons, the ⋯ menu, the folding of the Actions column);
  - `lazyRows.svelte.ts` (drawing the top first and more as the end nears);
  - `ReportTable` keeps the headers, sorting, grouping, select-all and export.
- **`TaskrcPanel.svelte`:**
  - `RcFileTree.svelte` (the list, folders, folding);
  - `RcEditor.svelte` (the CodeMirror box, markers, the live check);
  - `RcActions.svelte` (new file or folder, upload, download, holidays, include-in);
  - `TaskrcPanel` keeps the drafts, Apply and Restore, and the address.

Where a split would pass more than a handful of props, the part reads a small shared state object
(`taskrcState.svelte.ts`) instead of having props threaded through.

The parts are made to be used again where the same thing is drawn twice today (a row's ⋯ menu, a file list, a drawer's
form rows), not split for size alone.

### 4. Pages and settings tabs loaded on demand

- **Pages.** `App.svelte` imports every page up front. Instead, a small `lazy(() => import('./lib/BoardsPage.svelte'))`
  helper (`web/src/lib/lazy.svelte.ts`) gives each page a component that loads on first use. Tasks and the console stay
  in the main bundle: they are what most visits open, and the console bar is on every page.
- **Settings tabs.** `SettingsPage.svelte` does the same for its tabs, so only the tab opened is loaded.
- **No wait on first click.** Once the app is idle after start (`requestIdleCallback`, or a timeout where there is
  none), the lazy pages are fetched in the background. Hovering or focusing a link to a page starts its load at once.
  While a page loads, the shell stays and the page area shows the existing loading line (`WaitBar`), never a blank page.
- **Offline.** A page that fails to load (offline, a deploy replaced the files) says so, with Reload. The static app's
  service worker, if it has one, caches the chunks like any asset.

Drawers and dialogs used on many pages (the detail drawer, the task form) stay in the main bundle; rarely opened ones
(export, import, notify, holidays) load on demand the same way.

## Acceptance criteria

- [ ] Step 1:
  - opening Settings and moving through every tab makes at most one request per settings route;
  - opening a file in the taskrc tab makes at most one request;
  - a save reads again only the entries it changed.

  Checked by a test that counts the calls of a mocked `api.ts`, and by the network panel of the static app on a real
  branch (by a person).
- [ ] Step 1: a change from elsewhere still shows on the settings page without a reload: the console's `config`,
  another tab, a change pulled from the branch. Unapplied edits in the taskrc tab are never overwritten. Covered by tests.
- [ ] Step 1: no component fetches settings in an `$effect` that watches `store.config` (`grep` finds none).
- [ ] Step 2: `store.svelte.ts` is under 300 lines; every component still imports `store` alone; no component changed
  for the split.
- [ ] Step 3: no component in `web/src/lib` is over 600 lines.
- [ ] No behaviour change:
  - the web tests that pass before pass after, changed only where they import a moved module;
  - `npm run check` is clean;
  - the pages are clicked through in the dev server, on a scratch stack: Tasks, the console, the detail drawer, boards,
    and every Settings tab.
- [ ] Step 4: the main bundle (`App-*.js`, gzip) is at least a third smaller than the 144 KB measured here. The pages and
  settings tabs moved out are their own chunks (checked in the build output).
- [ ] Step 4: after the app has been idle a few seconds, opening any page shows it with no loading line (by a person,
  in the dev server, with the network throttled). Offline, a page not yet loaded says so and offers Reload (test).
- [ ] Docs: `docs/architecture.md`'s web section names the new modules; the AGENTS.md architecture map's `web/` line is
  updated.
- [ ] Size: the web bundle (`web/dist/assets`, gzip) measured before and after each step; it does not grow by more than
  a few KB.

## Open questions

Resolved 2026-11-19, with the person:

- The engine's five-second reuse (v0.8.17) stays.
- The phone is left as it is.
- Three releases: step 1; then step 4; then steps 2 and 3 together.
- The main bundle keeps Tasks and the console; a homepage that is another page has its chunk fetched with the app.

## Implementation notes

2026-11-19, step 1:

- `web/src/lib/settingsData.svelte.ts`: an `Entry` per read. Its `get()` joins a read in flight and uses an answer
  for `FRESH_MS` (30 s); `set` takes a save's answer; `stale` marks it to read again and keeps the value shown.
  `settingsData` holds the folder list, each file (the main taskrc read through `getTaskrcText`), the hooks and the
  secrets. Its write wrappers set or mark what they change. `changed(fresh)` marks all but `fresh` stale and bumps `rev`.
- Departure from the design: `/api/config` stays where it was (`getConfig` in `api.ts`, which already joins a read in
  flight, and `store.loadConfig`). It is read once per change, which is what step 1 asks; moving it is part of step 2.
- Departure from the design: an answer is used again for 30 seconds rather than kept until marked stale. A change made
  on another device reaches a page only through a sync of the settings it started from (kept routes), so without the
  limit a tab left open would never notice one.
- `store.settingsChanged(fresh)` calls `settingsData.changed(fresh)`; the taskrc tab and Task colours pass the files they
  just saved. A hooks change from the branch calls `settingsData.hooksChanged()`.
- `TaskrcPanel`, `TaskColours` and `HooksPanel` read and write through it. The taskrc tab's watcher on `store.config` is
  now a watcher on `settingsData.rev`.
- Measured on a scratch dev stack (Worker flavour, Chrome, `/api/*` requests other than `/api/tc`):
  - opening Settings read the main taskrc 4 times and the file list twice before; now once each;
  - an Apply in the taskrc tab was the write plus `/api/config`, the main taskrc and the list; now the write,
    `/api/config` and the list;
  - moving between tabs made no requests before or after (the panels stay mounted).
- Tests: `settingsData.test.ts` (joining, the 30 s, saves, imports, `changed`, a read made stale on its way), and two in
  `TaskrcPanel.test.ts` that count the reads of a start, an Apply, moving between files and a change from elsewhere.

2026-11-19, step 4:

- `lazy.svelte.ts` (`lazy`, `loadWhenIdle`), `LazyView.svelte` (draws it, counts the wait in `WaitBar`, says so with
  Reload and Try again when the code can't be fetched), `pages.ts` (`PAGES`, `TABS`, `DIALOGS`, `preloadPage`,
  `preloadPages`). Pages other than Tasks and the console, the settings tabs (each drawn from the first time it is
  opened), and the setup guide, review card, reminders, keys' help, export and import load on demand. Everything is
  fetched once the app is idle; pointing at a top-bar button starts its page.
- Measured (gzip -9): a first visit downloaded 182.9 KB of JavaScript before and 123.4 KB after; `App-*.js` went from
  144.8 KB to 55.3 KB.
- Clicked through in Chrome on a scratch dev stack: every page, every settings tab and three lazy pages opened by address,
  with no errors.
