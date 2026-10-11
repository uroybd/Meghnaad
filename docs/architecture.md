# Architecture

How Meghnaad is put together, and why. For what it does, see [Using Meghnaad](using.md); for the
settings it reads, [taskrc support](taskrc-support.md).

## The idea

Taskwarrior 3 keeps its data in **TaskChampion**, which syncs through a *server*: a place where every client
pushes its changes (as numbered "versions") and pulls everyone else's. With `sync.server.type=aws` that place is a
bucket, and the clients agree on its layout. Meghnaad is one more client of that bucket, written for the browser:

- It is a **replica** like your laptop's, not a copy of the database. It reads and writes the same bucket, speaking
  TaskChampion's own format and encryption, so `task sync` and the web app see each other's changes.
- The browser holds no tasks. A Cloudflare **Worker** holds an in-memory replica, and the browser asks it to run
  Taskwarrior commands and shows what comes back.
- Everything that is *Taskwarrior behaviour* (filters, reports, urgency, recurrence, confirmations, dates) lives in
  one Rust library, `tc-core`. The browser does layout, not logic.

```mermaid
flowchart TB
  subgraph desktop["Desktop: Taskwarrior 3"]
    direction TB
    sh["shell aliases<br/>alias fail='task done outcome:fail'"]
    tsh["tasksh<br/>review"]
    cli["task CLI (C++)"]
    tcd["TaskChampion (Rust)"]
    db[("SQLite replica<br/>~/.task")]
    rc["~/.taskrc"]
    sh -.-> cli
    tsh -.-> cli
    cli --> tcd --> db
    rc -.-> cli
  end

  subgraph web["Cloudflare: Meghnaad"]
    direction TB
    cmds["task commands<br/>(settings.json)"]
    rev["tasksh review"]
    spa["Svelte app<br/>(draws results)"]
    access{{"Cloudflare Access<br/>token check"}}
    wk["Worker (Rust → WASM)<br/>command engine in tc-core<br/>+ your hooks"]
    tcw["TaskChampion (Rust)"]
    mem[("In-memory replica<br/>per Worker instance")]
    cmds -.-> spa
    rev -.-> spa
    spa -- "POST /api/cli" --> access --> wk --> tcw --> mem
  end

  bucket[("R2 bucket<br/>salt · latest<br/>v-parent-child · s-version<br/>web/config.json · web/settings.json")]

  tcd <-- "task sync (when you run it)<br/>S3 API, key on your machine" --> bucket
  tcw <-- "before and after every command<br/>R2 binding, key in a Worker secret" --> bucket
  wk -. "taskrc settings, settings.json" .-> bucket

  sh <-. "counterpart" .-> cmds
  tsh <-. "counterpart" .-> rev

  classDef ext fill:transparent,stroke:#999,color:#999,stroke-dasharray:4 3
  class sh,tsh,cmds,rev ext
```

The dashed grey boxes are not Taskwarrior itself. They are what people add around it: shell aliases and `tasksh` on the
desktop, and their counterparts here, **task commands** (`settings.json`, expanded in the browser) and **tasksh's
`review`** (see below; the Settings page sets up the `reviewed` attribute and `_reviewed` report it needs). Both
only ever send ordinary commands to the one endpoint, so the engine knows nothing about them.

## The request path

Every action goes through one endpoint, `POST /api/cli`, with `{ "line": "…" }` (what you typed in the console) or
`{ "args": […] }` (what a button sends). One endpoint means one place where Taskwarrior semantics live. The one
exception is `POST /api/import`, which takes the *text of a file* (a command line can't carry one): the browser reads the
file and sends it as it is, and `tc-core`'s `import` module parses and checks it, so the rules are still only in Rust.
`?apply=1` writes it; without that it only reports what it would do. The settings are not commands, so they have routes
of their own: `/api/config/*` for the taskrc and urgency, and `GET`/`PUT /api/settings` for `settings.json` (`config` and
`show` still edit the taskrc through the one endpoint).

1. **Static files** (the app shell) are served by Cloudflare without running the Worker. Only `/api/*` runs it.
2. **`auth.rs`** checks the Cloudflare Access token on every `/api/*` request: RS256 signature against your team's
   published keys, issuer, audience, expiry. Misconfiguration means `403`, never open. A cold instance would wait about
   400 ms for the team's endpoint before it could check its first token, so the key list is fetched through
   Cloudflare's cache (`cacheEverything`, an hour, good answers only), which every instance in a data centre shares. A
   token signed with a key that is not in the list fetches it again past the cache (at most once a minute), so a
   rotation is picked up at once.
3. **`session.rs`** holds what a Worker instance ("isolate") keeps between requests: the derived key, a long-lived
   replica, the undo stack and the imported taskrc. A cold instance builds these; a warm one reuses them.
4. **Sync, run, sync.** The Worker pulls versions it hasn't seen, runs the command against the replica, and if
   anything changed pushes it before answering. With nothing new, a sync is one read of `latest`.
5. **`tc-core::cli::execute`** parses and runs the command and returns a `CliResult` (a report, a table, a calendar,
   a question, an error…), which is serialised as JSON.

## `tc-core`: the engine

Pure Rust, no I/O of its own, tested natively. The Worker and the tests are two hosts for it.

| Area | Modules | What they do |
| --- | --- | --- |
| **Protocol** | `cloud`, `crypto`, `names`, `store` | TaskChampion's cloud server over an object store; PBKDF2 → ChaCha20-Poly1305; object names; the storage trait the Worker implements over R2 |
| **Replica storage** | `live` | The replica's in-memory database. TaskChampion's own copies every task on the first write of each transaction, so every sync and every write cost a full copy (and twice the memory for a moment); this one changes its data in place and keeps a short journal to undo a transaction that is dropped without commit. A test runs random transactions against both and compares them |
| **Tasks** | `model`, `filter`, `modify`, `dates`, `rx`, `recur` | A plain view of a task; filter expressions; planning `modify`/`add`; Taskwarrior's date and duration parsing; Taskwarrior's regular expressions; recurring tasks |
| **Commands** | `cli` | Parsing a command line (aliases, abbreviations, contexts, `rc.` overrides), the write path with its confirmations (`add`, `log`, `duplicate`, `modify`, …), undo, the `commands` listing, and dispatch to everything below |
| **Reports** | `report`, `run`, `urgency`, `history` | Built-in and custom reports, running one (filter, sort, limit, columns), urgency, and the change history of a task |
| **Colour** | `color` | Taskwarrior's colour specifications and how they blend, the rules that colour a task (with precedence and merge), the `colors` command and the history graph's colours; checked against the escape codes of the real `task` |
| **Views** | `summary`, `calendar`, `burndown`, `activity`, `stats`, `calc`, `export`, `import` | The `summary`, `calendar`, `burndown.*`, `history.*`/`ghistory.*`/`timesheet` (`activity`), `stats`, `calc` (and `_get`, which reads the same DOM) and `export` commands, each a port of its Taskwarrior counterpart. `export` writes Taskwarrior's own JSON from the stored properties; `import` reads it back (a port of `CmdImport`, checking the whole file before it writes anything); the app's pages use a separate, hidden `_rows`, and the detail view's History a hidden `_history <uuid>` that reads one task's operations without loading the rest |
| **Hooks** | `hooks`, `my_hooks` | `hooks` runs your own Rust at Taskwarrior's four hook points (`on_launch`, `on_add`, `on_modify`, `on_exit`): compiled into the Worker, fed a task and handing one back, with what they print returned to the Console; `my_hooks` is the one file you edit, kept apart so upstream updates rarely touch it |
| **Settings** | `taskrc`, `rc_url`, `settings`, `appsettings` | The allowlisted subset of a taskrc: what is accepted, what is refused, and the typed `Config` the rest reads; `rc_url` is which links the Worker may fetch a taskrc from (HTTPS to a public host name only, redirects included) and the GitHub/GitLab page-to-raw rewrite; and the `show` / `config` / `context` commands that list and edit it under the same rules (`context` is a few `context.<name>.*` settings, edited through `config`); `appsettings` checks that the app's own `settings.json` is a JSON object of a sensible size before the Worker stores it |

### How a command runs

`execute` does the same things in the same order every time:

1. Apply the command line's `rc.<key>:<value>` overrides to a copy of the config, then the active context's own
   settings (which win, as in Taskwarrior).
2. Read the first day of the week, `date.iso` and `dateformat` into the clock, and expand aliases (typed lines only).
3. Parse into a filter, a command and modifications. A word shorter than `abbreviation.minimum` is just a word.
4. Run the `on_launch` hook, which can refuse the command (`show`, `config` and `context` skip it: they are about settings).
5. Do Taskwarrior's housekeeping first: create the recurring instances that are due, expire tasks past `until`.
6. Load the tasks as plain "facts", number the pending ones, and run the command. `on_add` and `on_modify` fire inside
   it, just before a write is committed.
7. Run the `on_exit` hook, and return what the hooks printed with the result (`feedback`), for the Console to show.

Writes go through one path (`write_selected`) so the safety rules are in one place:

- **Questions are data, not prompts.** A write that needs an answer returns a `Confirm` result and changes
  nothing. There are three kinds: `plain` (yes/no: undo, a command with no filter), `permission` (one row per task
  the command would change: Taskwarrior's yes/no/all/quit becomes ticks in a table) and `extras` (follow-ups that
  only arise once those are answered: repair a dependency chain, carry a change to a recurring series). The client
  re-sends the command with the answers (`Options { confirmed, approved, extras }`).
- **Hooks** run inside this path, before the commit: `on_add` and `on_modify` see the task as the command leaves it, and what they hand back is diffed and applied into the same batch, or the command is refused and nothing is saved.
- Everything the command changes is built as one batch of operations and committed once, so it is one undo step.
- Undo can't use TaskChampion's own (it only covers unsynced changes, and the Worker syncs after every write), so
  the inverse operations are committed as new changes. The stack lives in the isolate's memory.

### Sync

`CloudServer` is TaskChampion's server interface over the bucket, wire-compatible with the CLI's own AWS/GCP
backends (`salt`, `latest`, `v-<parent>-<child>`, `s-<version>`; encrypted with a key from your secret and the
bucket's salt). Because the Worker is a *replica*, it never needs the CLI's cooperation.

- **Cheap when idle.** One `GET latest`. Catching up lists the versions once and fetches them in batches of 32,
  rather than one at a time.
- **Conditional writes.** `latest` is swapped with its ETag, so two clients pushing at once can't both win; the loser
  re-syncs and retries.
- **Snapshots, like the CLI.** After a push the Worker occasionally (about one push in ten) writes a snapshot and
  removes the ones it supersedes, so a cold start restores one snapshot plus a few versions instead of replaying
  history. A failed snapshot is ignored on purpose: failing the sync would make the replica send an already-sent
  version again.
- **Tidying up, like the CLI.** About one push in twenty is followed by a cleanup, as in TaskChampion's own server:
  it deletes versions that lost a race (but not one a writer may be about to make `latest`), every snapshot but the
  newest, and versions older than about 180 days that the newest snapshot covers. It uses each object's upload time from
  R2's listing and deletes up to 1,000 objects per cleanup in a single request (R2 takes that many at once), so a
  cleanup costs a handful of requests however much it removes; the next one carries on. The oldest versions go first,
  so what is left is always one unbroken chain up to `latest` (a replica that is behind finds the next version or none,
  never a gap). A failure is ignored, since the push it follows has already happened. A replica that was last synced
  before the oldest surviving version can no longer catch up from the versions alone, which is the same for the CLI's
  own server.

## Limits of the platform

A Worker request has hard limits, and this app is shaped by them. Cloudflare's numbers (checked at their limits page):

| | Workers Free | Workers Paid |
| --- | --- | --- |
| CPU time per request | **10 ms** | 30 s (up to 5 min) |
| Memory per isolate | 128 MB | 128 MB |
| Subrequests per request (R2 calls count) | 50 | 10,000 |

What this app costs, measured (`crates/tc-core/tests/memory.rs`, and a local `workerd`):

- **The key.** Deriving it (PBKDF2, 600,000 rounds) is the heaviest thing a *cold* instance does. The Worker asks the
  runtime's native Web Crypto for it, which takes about 40 ms; the same in WebAssembly took about 250 ms. A warm
  instance reuses it.
- **A warm request** is a few milliseconds of CPU for a hundred tasks and, before the cache below, tens for thousands
  (filtering, sorting, urgency, serialising).
- **Memory** is small: restoring a snapshot of 3,000 tasks peaks near 6 MB, replaying 1,200 versions from nothing near
  6 MB, and a request over 3,000 tasks 8 MB (19 MB for a full `export`). An `import` of the most one takes (5,000
  tasks, a 2 MB file) peaks near 51 MB, and its `undo` step keeps about 8 MB while the instance lives, which is why the
  request is capped. The test fails if a change makes any of these much worse.

**It runs on Workers Free**, as does its author's own deployment, with R2 and Access on their free plans too. The first
request after an idle spell does the most work (the key, then the snapshot): one on a real deployment used about 550 ms
of CPU, far past the free plan's 10 ms, and still completed, because Cloudflare tolerates a Worker that only
*infrequently* runs over and ends requests with error 1102 only when it keeps doing so. So cold starts must stay rare
and cheap: a very large task list, or many cold starts in a row, can tip it. The app retries a read-only request once,
because the retry meets a warm instance, and Workers Paid ($5/month) lifts the limit if that ever bothers you. R2 calls
count toward the 50 subrequests per request, so a cold start that has to replay many versions (the Worker writes
snapshots to keep that number small) is the other thing to watch.

**What is done about CPU.** Turning every stored task into the engine's form (`Facts`) took about 2 µs a task, and every
command began with it, which at 3,000 tasks was most of a read. The Worker now keeps that list between
requests (`tc_core::warm`, next to the replica) and rebuilds it only when the storage's change counter
(`LiveStorage::generation`, bumped when a transaction that changed anything commits or is taken back) has moved. Most
requests change nothing, so they skip it. `tests/warm.rs` checks, after every step of a long random run of commands,
direct writes and pulls from the bucket, that the cached list equals a fresh read. The page's own requests were trimmed
to match: `_names` is `_tags` and `_projects` in one request, and the reminder poll asks `_remind` (seven fields) instead
of `_rows` (a full row, with every virtual tag, for each task). Measured natively at 3,000 tasks (WebAssembly is somewhat
slower, but the proportions hold):

| Request | Before | After |
| --- | --- | --- |
| `next`, `list`, `count` | about 8 ms | about 2 ms |
| the name lists (`_tags` and `_projects`) | 2 × 6.2 ms | one request, 0.5 ms |
| the reminder poll | 13.3 ms (`_rows`) | 0.9 ms (`_remind`) |
| a write (`modify` on one task) | about 8 ms | about 1.7 ms |

A write does not rebuild the list either. The storage keeps which tasks each of its last 32 transactions touched
(`Generation::touched_since`), and a list that is a few changes behind is brought up to date by reading just those tasks
and moving them into place (`warm::patch`). That is only done when no touched task depends on another, or has another
depending on it, since a dependency changes whether tasks are blocked or blocking, which is worked out for the whole
list; then the list is built again, as it is after a long pull from the bucket or an import. The same random run checks
the patched list against a fresh read after every step, including the steps that add and remove dependencies.

For scale, one personal deployment over its first nine days of a billing cycle used about 1,100 requests a day (1% of the
free plan's 100,000), about 460 R2 writes and listings and 1,100 R2 reads a day (projecting to roughly 1.4% and 0.35% of
R2's free 1 million and 10 million a month) and next to no storage, and was billed $0.00.

## The web app

A Svelte 5 single-page app (`web/`), built with Vite and served as static assets.

- `store.svelte.ts` is the one store: the current page, the config, the console's entries, the live report, the
  answers to questions in flight, and the toast.
- **Pages** (`TasksView`, `ProjectsView`, `TagsView`, `SummaryPage`, `CalendarPage`, `BurndownPage`, `ConsoleView`, `SettingsPage`) are thin. Each
  asks the engine for a result and hands it to a shared component, so the console and the pages draw the same thing.
- **Settings** is one page, `SettingsPage`, with a tab per panel: `TaskrcPanel` (the taskrc text, themes, import),
  `TaskshPanel`, `CommandsPanel` and `UrgencyPanel`. Each has its own Apply and tells the page when it has edits, and all three stay
  mounted so edits survive a change of tab. `tasksh.ts` decides what the `reviewed` UDA and the `_reviewed` report need
  and which `config` lines write them; the panel sends those through `POST /api/cli` like any other command, so nothing
  about tasksh is in the Worker.
- **Review** (tasksh's `review`) is the browser's too, and only a client of the one endpoint. `review.svelte.ts` is the
  session: it asks for the `_reviewed` report (the uuids), shows each task with `info`, and carries out the menu with
  ordinary commands (`modify reviewed:now`, `done`, `delete`, with `rc.confirmation:no`), so undo, hooks and the settings
  work as for any other command. A question Taskwarrior asks mid-step (the recurring series) comes back as a `confirm`
  result and is put to the person (`asking`), then the same command is sent again with the answer. Two surfaces drive
  it, with one `ReviewBar` of buttons between them: `ConsoleInput` (tasksh's menu is keybindings, so while
  `review.captures` a key is the answer, the prompt is off, and only what to change is typed; `store.addEntry` prints the
  banner, the `info` and the messages into the scrollback) and `ReviewCard` (buttons and the same keys, opened from the
  top bar or by **Open as card**, which moves a console review there). `tasksh.ts` has what is pure: reading `review N`,
  the menu keys, tasksh's wording and the setup's questions (the setup is the tasksh tab's `config` lines, asked one part
  at a time). The editor the `e` choice opens reports its closing through `App.svelte` (`review.editClosed`).
- **Task commands** are the browser's. `appsettings.ts` reads and writes `settings.json` (`{ "taskCommands": [{ name,
  command, icon? }] }`, other keys kept), which the Worker only stores (`GET`/`PUT /api/settings`; `tc-core`'s
  `appsettings` checks that it is a JSON object under 64 KB and nothing more). `taskCommands.ts` writes a command out on
  the line you typed: the first word that is not part of the filter, as the engine reads a line, is the command word,
  and only that word can be one, so `add fail the test` is untouched. A name is written out once per line, so a command
  can extend the Taskwarrior command of the same name. `ConsoleInput` expands before it decides where the result goes and
  hands `store.run` what was typed for the history. The same commands are the **⋯** menu (`MoreMenu`) in a table row,
  the task drawer and the bulk bar, which send `<tasks> <the command's words>` like any other button. Icons are
  Lucide's, picked in `IconPicker`. The app's own are bundled and found by name (`bundledIcons` in `lucide.ts`); any other
  is fetched as one SVG file from jsdelivr at the Lucide version pinned there (`LUCIDE_VERSION`), then read into a short
  list of plain shapes (`svgToShapes`: only `path`, `circle`, `rect`, `line`, `polyline`, `polygon` and `ellipse`, with
  numeric or path attributes; anything else refuses the whole file) and drawn inline by `NodeIcon` in the page's colour.
  So the CDN supplies data and never code, and the CSP needs only `connect-src` for that one host. `lucideLib.svelte.ts`
  keeps what was fetched (in memory, and the last 150 in `localStorage`) and the picker's list of names and search words
  (`tags.json`, about 50 KB compressed, fetched when the picker opens).
- Colours: the engine sends each row's colour as palette indexes (`Row.style`) and the chart colours with the config;
  `colors.ts` draws them (the soft basic sixteen per light/dark page, exact xterm colours otherwise, a contrast guard),
  `scheme.svelte.ts` says which page it is, and `themes.ts` and the taskrc tab's picker put a bundled theme's lines
  into the taskrc (the theme files are static assets).
- `ResultView` turns any `CliResult` into UI: `ReportTable`, `SummaryView`, `CalendarView`, `BurndownView`,
  `TaskInfo`, `ConfirmView`, and the lines hooks printed under the result (a toast when a button, not the console, ran the command). A report's cells are formatted in `format.ts`, the one place that knows `dateformat`,
  indicators and urgency colours.
- **Bulk actions** (`BulkBar`, `BulkModify`, `selection.svelte.ts`, `bulk.ts`): the table's checkboxes feed one selection; the bar sends ordinary commands (the whole uuids of the ticked tasks side by side, which Taskwarrior reads as one group, then `done`, `delete` or `modify` with the words the editor made), so the engine needs nothing of its own. **Command** hands the prompt a line through `store.requestPrompt`.
- **Task addresses**: `route.ts` maps a task to `/task/<8 characters of its uuid>` and back; `App.svelte` keeps the address bar and the drawer in step (opening pushes an address, closing or Back leaves it), and `store.openFromAddress` finds the task with `uuid.startswith:<prefix> uuids`. `DepLink` is the id of another task (a dependency, a parent) with a hover, focus or tap card; in the drawer `DepItem` is a line that expands to the same `TaskCard`, and the tasks that wait on this one are looked up with `depends.has:<uuid>` only when asked. A project part or a tag chip jumps to its entry on the Projects or Tags page (`store.showProject` / `showTag`).
- The filter box (`FilterInput`, `FilterChips`) and the prompt share `completion.ts`; both feed the same engine.
- The browser stores nothing but small conveniences (command history, reminder settings). Settings live in the bucket
  (`web/config.json`, with one backup; the app's own `web/settings.json`, which has none).

## How correctness is kept

The goal is Taskwarrior's behaviour, not a lookalike, so the method is to **read the C++, port it, and check it
against the real `task` 3.5.0**: run both on a scratch data directory and compare. Examples: calendar layouts and week
numbers, burndown charts bar by bar, `summary`/`projects`/`tags` output, `calc` results, confirmation prompts, the
dates each `date.iso`/`dateformat` combination accepts.

| Layer | What it proves |
| --- | --- |
| `cargo test` (unit) | Each port against fixtures taken from the real `task` |
| `tests/cli_exec.rs` | Real command lines against a real replica: writes, reports, confirmations, recurrence, calc, `show`/`config`, hooks (injected through `Options.hooks`) |
| `tests/sync_cost.rs`, `replica_sync.rs` | R2 call counts per sync; two replicas converging |
| `tests/memory.rs` | Peak memory of a cold start and of requests |
| `npm --prefix web test` | The browser-side logic: formatting, completion, the store, the API client |
| `scripts/interop-local.sh` | The real `task` CLI and the Worker against one local S3 bucket, both directions: tasks, UDAs, recurring series, snapshots |
| `scripts/auth-check.mjs` | Forged, expired and wrong-audience Access tokens are all refused; a rotated key is picked up |
