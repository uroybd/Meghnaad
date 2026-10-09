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

```
 task CLI ──(S3 API, your R2 token)──────┐
                                          ▼
 Browser ── Cloudflare Access ──▶ Worker (Rust → WASM) ──(R2 binding)──▶ R2 bucket
   Svelte SPA ── POST /api/cli ──▶   auth → session → tc-core engine        salt · latest
                                                                            v-<parent>-<child>
                                                                            s-<version>
                                                                            web/config.json
```

## The request path

Every action goes through one endpoint, `POST /api/cli`, with `{ "line": "…" }` (what you typed in the console) or
`{ "args": […] }` (what a button sends). One endpoint means one place where Taskwarrior semantics live.

1. **Static files** (the app shell) are served by Cloudflare without running the Worker. Only `/api/*` runs it.
2. **`auth.rs`** checks the Cloudflare Access token on every `/api/*` request: RS256 signature against your team's
   published keys, issuer, audience, expiry. Misconfiguration means `403`, never open.
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
| **Tasks** | `model`, `filter`, `modify`, `dates`, `rx`, `recur` | A plain view of a task; filter expressions; planning `modify`/`add`; Taskwarrior's date and duration parsing; Taskwarrior's regular expressions; recurring tasks |
| **Commands** | `cli` | Parsing a command line (aliases, abbreviations, contexts, `rc.` overrides), the write path with its confirmations, undo, and dispatch to everything below |
| **Reports** | `report`, `run`, `urgency`, `history` | Built-in and custom reports, running one (filter, sort, limit, columns), urgency, and the change history of a task |
| **Views** | `summary`, `calendar`, `burndown`, `calc` | The `summary`, `calendar`, `burndown.*` and `calc` commands, each a port of its Taskwarrior counterpart |
| **Settings** | `taskrc` | The allowlisted subset of a taskrc: what is accepted, what is refused, and the typed `Config` the rest reads |

### How a command runs

`execute` does the same things in the same order every time:

1. Apply the command line's `rc.<key>:<value>` overrides to a copy of the config, then the active context's own
   settings (which win, as in Taskwarrior).
2. Read the first day of the week, `date.iso` and `dateformat` into the clock, and expand aliases (typed lines only).
3. Parse into a filter, a command and modifications. A word shorter than `abbreviation.minimum` is just a word.
4. Do Taskwarrior's housekeeping first: create the recurring instances that are due, expire tasks past `until`.
5. Load the tasks as plain "facts", number the pending ones, and run the command.

Writes go through one path (`write_selected`) so the safety rules are in one place:

- **Questions are data, not prompts.** A write that needs an answer returns a `Confirm` result and changes
  nothing. There are three kinds: `plain` (yes/no: undo, a command with no filter), `permission` (one row per task
  the command would change: Taskwarrior's yes/no/all/quit becomes ticks in a table) and `extras` (follow-ups that
  only arise once those are answered: repair a dependency chain, carry a change to a recurring series). The client
  re-sends the command with the answers (`Options { confirmed, approved, extras }`).
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
- **Left to the CLI:** deleting old versions (those a snapshot covers, older than about 180 days).

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
- **A warm request** is a few milliseconds of CPU for a hundred tasks and tens for thousands (filtering, sorting,
  urgency, serialising).
- **Memory** is small: restoring a snapshot of 3,000 tasks peaks near 6 MB, replaying 1,200 versions from nothing near
  6 MB, and a request over 3,000 tasks 8 MB (19 MB for a full `export`). The test fails if a change makes any of
  these much worse.

So the free plan's 10 ms CPU limit is below what the first request after an idle spell needs, and **the Worker needs
the Workers Paid plan** (R2 and Access are fine on free). The app retries a read-only request once if Cloudflare cuts
it off with error 1102, because the retry meets a warm instance.

## The web app

A Svelte 5 single-page app (`web/`), built with Vite and served as static assets.

- `store.svelte.ts` is the one store: the current page, the config, the console's entries, the live report, the
  answers to questions in flight, and the toast.
- **Pages** (`TasksView`, `ProjectsView`, `SummaryPage`, `CalendarPage`, `BurndownPage`, `ConsoleView`) are thin. Each
  asks the engine for a result and hands it to a shared component, so the console and the pages draw the same thing.
- `ResultView` turns any `CliResult` into UI: `ReportTable`, `SummaryView`, `CalendarView`, `BurndownView`,
  `TaskInfo`, `ConfirmView`. A report's cells are formatted in `format.ts`, the one place that knows `dateformat`,
  indicators and urgency colours.
- The filter box (`FilterInput`, `FilterChips`) and the prompt share `completion.ts`; both feed the same engine.
- The browser stores nothing but small conveniences (command history, reminder settings). Settings live in the bucket
  (`web/config.json`, with one backup).

## How correctness is kept

The goal is Taskwarrior's behaviour, not a lookalike, so the method is to **read the C++, port it, and check it
against the real `task` 3.5.0**: run both on a scratch data directory and compare. Examples: calendar layouts and week
numbers, burndown charts bar by bar, `summary`/`projects`/`tags` output, `calc` results, confirmation prompts, the
dates each `date.iso`/`dateformat` combination accepts.

| Layer | What it proves |
| --- | --- |
| `cargo test` (unit) | Each port against fixtures taken from the real `task` |
| `tests/cli_exec.rs` | Real command lines against a real replica: writes, reports, confirmations, recurrence, calc |
| `tests/sync_cost.rs`, `replica_sync.rs` | R2 call counts per sync; two replicas converging |
| `tests/memory.rs` | Peak memory of a cold start and of requests |
| `npm --prefix web test` | The browser-side logic: formatting, completion, the store, the API client |
| `scripts/interop-local.sh` | The real `task` CLI and the Worker against one local S3 bucket, both directions: tasks, UDAs, recurring series, snapshots |
| `scripts/auth-check.mjs` | Forged, expired and wrong-audience Access tokens are all refused |
