<p align="center">
  <img src="web/public/logo.png" alt="Meghnaad" width="140" />
</p>

<h1 align="center">Meghnaad</h1>

<p align="center">
  <a href="https://taskwarrior.org">Taskwarrior 3</a> in your browser. Runs on Cloudflare: a Rust Worker,
  your existing R2 bucket as the sync store, and Cloudflare Access for login.
</p>

---

Meghnaad is another **replica** of your Taskwarrior data. Your `task` CLI keeps syncing to R2 exactly as it does
today; this app reads and writes the same bucket, so a task added in the browser shows up on your laptop after
`task sync`, and the other way round. It is **console-first** (type `task`-style commands) with a normal UI beside
it (tables, forms, a detail view), and both go through the same command engine.

- **Works with the bucket you already have.** Point it at the R2 bucket your `task` already syncs to and give it
  the same encryption secret. No migration, no new server, nothing changes for the CLI.
- **Taskwarrior behaviour, not an imitation of it.** Filters, virtual tags (`+OVERDUE`), urgency, report sorting,
  UDAs and custom reports follow Taskwarrior's own rules, ported from its source.
- **Your `taskrc`, safely.** Import your UDAs, custom reports and contexts. Sync settings and anything that looks
  like a credential are deliberately blocked.
- **Time tracking** (`journal.time`), a sessions table per task, and reminders while the tab is open.

> **Status.** Verified end to end against the released `task` 3.5.0 using a local S3-compatible server (both
> directions: tasks, UDAs, custom reports, timed due dates). It has **not yet been run against a real R2 bucket**;
> see [Known limitations](#known-limitations).

## Contents

[How it works](#how-it-works) · [Quick start (local)](#quick-start-local) · [Deploy to Cloudflare](#deploy-to-cloudflare) ·
[Connect your `task` CLI](#connect-your-task-cli) · [Using Meghnaad](#using-meghnaad) · [Configuration](#configuration-reference) ·
[Security](#security-notes) · [Development](#development) · [Troubleshooting](#troubleshooting) ·
[Known limitations](#known-limitations)

## How it works

```
 task CLI  ──(S3 API + your R2 token)──┐
                                        ▼
 Browser ── Cloudflare Access ──▶  Worker (Rust → WASM) ──(R2 binding)──▶  R2 bucket
   Svelte app ──  /api/cli  ──────▶   • checks the Access token            salt · latest
                                      • decrypts with your secret          v-<parent>-<child>
                                      • runs the command engine            s-<version>
                                      • syncs, then writes                 web/config.json
```

- The bucket holds Taskwarrior's standard **TaskChampion cloud layout** (`salt`, `latest`, `v-…`, `s-…`), encrypted
  client-side. The Worker implements the same protocol, so it and the CLI are interchangeable replicas.
- The Worker keeps an in-memory replica per instance and **syncs before and after every command**, so it only
  downloads versions it hasn't seen.
- One endpoint, `POST /api/cli`, takes `{ "line": "…" }` (the console) or `{ "args": […] }` (the UI), so there is a
  single place where Taskwarrior semantics live.
- Your imported `taskrc` settings are stored in the same bucket (`web/config.json`, plus one backup), so they
  survive restarts, redeploys and moving between devices.

## Quick start (local)

You can run everything on your machine without a Cloudflare account; wrangler simulates R2 locally.

**Prerequisites**

| Tool | Version | Notes |
| --- | --- | --- |
| Rust | 1.91+ | via [rustup](https://rustup.rs); needs the `wasm32-unknown-unknown` target |
| Node.js | 20+ | developed on 24 |
| `worker-build` | 0.8 | compiles the Rust Worker to WASM |

```bash
git clone <this repo> && cd Meghnaad

rustup target add wasm32-unknown-unknown
cargo install worker-build

npm install                      # wrangler
npm --prefix web install         # the Svelte app
cp .dev.vars.example .dev.vars   # local-only secret + Access bypass

npm --prefix web run build       # wrangler needs web/dist to exist
npm run dev                      # builds the Worker, then serves everything
```

The first `npm run dev` compiles the Worker to WASM (about a minute on a recent laptop, longer on a slow machine);
after that it rebuilds incrementally when you edit `crates/`. Open **http://127.0.0.1:8787**.

**Hot-reloading UI** (optional): in a second terminal run `npm --prefix web run dev`. It serves the app with live
reload and proxies `/api` to the Worker on 8787. If Vite's default port 5173 is taken it picks the next free one;
set `API_URL=http://127.0.0.1:<port>` to point it at a different Worker.

**Try it with sample data** (a bucket with nothing in it is fine too):

```bash
./scripts/seed-dev.sh            # demo tasks, UDAs, custom reports, a context, journal.time
```

Local R2 data lives in `.wrangler/state`; delete that directory to start over. `DEV_AUTH_BYPASS=1` in `.dev.vars`
makes the local server skip the Cloudflare Access check, which is why you can use it without logging in.

## Deploy to Cloudflare

You need a Cloudflare account with R2 and Zero Trust (the free plans work).

**1. Log in and choose the bucket.**

```bash
npx wrangler login

# Already syncing Taskwarrior to R2? Use that bucket and skip this line.
npx wrangler r2 bucket create taskwarrior-sync
```

In `wrangler.jsonc`, set `r2_buckets[0].bucket_name` to your bucket's name.

**2. Set the encryption secret** to the *same value* as `sync.encryption_secret` in your Taskwarrior config.

```bash
npx wrangler secret put TC_ENCRYPTION_SECRET
```

If the bucket is new, pick any strong passphrase now and use it for the CLI in the next section. Anyone who can read
this secret can decrypt your tasks, so treat it like a password.

**3. Deploy once** to get a URL (`https://taskwarrior-web.<your-subdomain>.workers.dev`, or attach a custom domain):

```bash
npm run deploy
```

Until step 4 is done every `/api/*` request is answered with `403`: the Worker is closed by default, including
when `TEAM_DOMAIN` and `POLICY_AUD` still hold their placeholders.

**4. Put Cloudflare Access in front.**

1. Zero Trust dashboard → **Access → Applications → Add an application → Self-hosted**.
2. Application domain: the hostname from step 3. Add an **Allow** policy for your own email (or your identity group).
3. Open the application and copy its **Application Audience (AUD) Tag**. Your team name is under **Settings →
   Custom Pages** (`<team>.cloudflareaccess.com`).
4. Put both in `wrangler.jsonc`:

   ```jsonc
   "vars": {
     "TEAM_DOMAIN": "https://<your-team>.cloudflareaccess.com",
     "POLICY_AUD": "<the AUD tag>"
   }
   ```

5. Redeploy: `npm run deploy`.

Cloudflare Access signs people in; the Worker then **re-verifies the signed token on every API call** (signature,
issuer, audience, expiry), so your data stays protected even if someone finds a way around Access, such as the
`workers.dev` address. Make the Access application cover the **whole hostname**: the static app shell (HTML and
JavaScript, nothing private) is served without running the Worker, so Access is what guards it.

**5. Open the URL**, sign in, and import your `taskrc` (the **taskrc** button) to get your UDAs and custom reports.

## Connect your `task` CLI

Skip this if your CLI already syncs to the bucket. Otherwise create an **R2 API token** (R2 → *Manage R2 API Tokens*
→ **Object Read & Write**, scoped to your bucket) and note your account ID, then:

```bash
task config sync.encryption_secret '<the same value as TC_ENCRYPTION_SECRET>'
task config sync.aws.region        auto
task config sync.aws.bucket        <your bucket name>
task config sync.aws.access_key_id     <R2 access key id>
task config sync.aws.secret_access_key <R2 secret access key>

export AWS_ENDPOINT_URL=https://<ACCOUNT_ID>.r2.cloudflarestorage.com   # add to your shell profile
task sync
```

Released versions of Taskwarrior have no config key for a custom S3 endpoint, but the AWS SDK it uses reads
`AWS_ENDPOINT_URL` from the environment, which is what points it at R2. It must be set **everywhere `task sync` runs**
(your shell, cron, hooks, GUI launchers); otherwise `task` quietly talks to real AWS and fails with a
`PermanentRedirect` error. Newer, unreleased Taskwarrior builds add `sync.aws.endpoint_url`.

## Using Meghnaad

Two views over one engine: **Tasks** (a table with filters) and **Console**. A bar at the bottom is the prompt in
both, with the **last command** shown above it. That line follows the UI too: pick a report or click a table header
and you can see the exact command it ran, then press **↑** to edit and re-run it.

### Console

Commands follow Taskwarrior: `[filter] command [modifications]`, and the leading `task` is optional.

```
next                                   # a report (built-in, or defined in your taskrc)
project:Home +errand due.before:eow list
add Pay rent project:Home due:eom +bills
3 modify due:2026-12-25T08:30 priority:H
3 start        3 stop        3 done        3 delete        undo
3 annotate called the landlord
estimate:big count                     # UDAs work like any attribute
rc.report.next.sort:due-,project+ next # per-command overrides, as in Taskwarrior
help
```

| | |
| --- | --- |
| **Write** | `add` `modify` `done` `delete` `start` `stop` `annotate` `denotate` `append` `prepend` `undo` |
| **Read** | `info` `count` `projects` `tags` `udas` `columns` `reports` `contexts` `show` `export` `ids` `uuids` |
| **Filters** | `attr:value`, with modifiers `.is .not .has .startswith .before .after .by .none .any …`; `+tag` / `-tag`; virtual tags (`+OVERDUE +DUETODAY +READY +ACTIVE +BLOCKED …`); `/text/`; ids (`3`, `1-4,7`) and uuid prefixes; `and` `or` `not` and parentheses |
| **Dates** | `today tomorrow eow som eoy monday 3d 2w`, `2026-12-25`, `2026-12-25T08:30`, `now+2h` |

Notes that differ from Taskwarrior on a desktop:

- **Numeric ids are specific to this app** (pending tasks numbered by creation time). They won't match your laptop's.
  Use uuid prefixes where it matters.
- A command that would change several tasks asks you to confirm first.
- `undo` works on the last few commands made in this session of the Worker; it is forgotten when the Worker is
  recycled.

**Keys:** `/` focuses the prompt · `Tab` completes (and cycles a menu; `Shift+Tab` goes back) · `↑`/`↓` the last 20
commands · `Esc` closes a menu · `Ctrl+L` clears.

### Tasks view

- Pick any report (including your own) and add filters by typing, with Tab completion, or from the helpers. Filters
  show as removable chips.
- **Click a column header to sort** (again to reverse, a third time to reset, Shift-click to add a tie-breaker). The
  sort is applied as `rc.report.<name>.sort:…`, so it matches what you would type in the console.
- **Click a row** for the detail view: all fields, annotations, dependencies you can follow, and (with
  `journal.time`) a table of work sessions. Row buttons: done, start/stop, edit, delete.
- **Add tasks** from the sidebar. *More fields…* opens the full form: project, priority, due/wait/scheduled/until,
  tags, dependencies, UDAs, "start now", and a first note.
- Every **date field takes a date and an optional time**. Leave the time empty for a whole day.

### Your taskrc

UDAs, custom reports and contexts live in `~/.taskrc`, not in your synced data, so the app keeps its own copy. Open
**taskrc**, paste or choose your file, and save. Only these are kept: `uda.*`, `report.*`, `context*`, `urgency.*`,
`journal.*`, `default.command`, `due`, `weekstart`, `dateformat*`.

- **Sync settings and anything credential-like are blocked** (`sync.*`, `taskd.*`, names containing `secret`,
  `password`, `token`, …). Only the *names* of what was dropped are shown. The raw text is never stored.
- What is saved is shown back to you, so each import edits it rather than replacing it with a blank page. **Restore
  previous** undoes the last save.
- `include` lines can't be followed; paste the included files' contents too.
- A UDA that exists on a task but isn't defined in your taskrc is shown but **read-only**.

### Time tracking and reminders

- Add `journal.time=on` to your taskrc. `start` and `stop` then record "Started task" / "Stopped task" (the texts are
  configurable), and the detail view shows a sessions table with a running total. A running task shows a timer in
  the header and tab title.
- The **bell** turns on reminders: a heads-up before a timed due date, a morning reminder for dates without a time,
  tasks coming off `wait`, and one digest per day for what's already overdue. Reminders need the **tab to be open**
  (the app can't wake up in the background), and system notifications need HTTPS or `localhost` plus your permission.

## Configuration reference

| Name | Kind | Purpose |
| --- | --- | --- |
| `TASKS` | R2 binding (`wrangler.jsonc`) | The bucket your `task` CLI syncs to |
| `TC_ENCRYPTION_SECRET` | Worker secret | Same value as the CLI's `sync.encryption_secret` |
| `TEAM_DOMAIN` | var | `https://<team>.cloudflareaccess.com` |
| `POLICY_AUD` | var | The Access application's AUD tag |
| `DEV_AUTH_BYPASS` | `.dev.vars` only | Skips the Access check for local development. It is only honoured for loopback hosts (`localhost`, `127.x.x.x`, `::1`); on any other host it is ignored and logged. Still: never set it on a deployed Worker |

## Security notes

- **Login** is Cloudflare Access. The Worker independently validates the `Cf-Access-Jwt-Assertion` token on every
  `/api/*` request: RS256 signature against your team's published keys, issuer, audience, expiry, and not-before.
  It refuses `alg: none`, algorithm-confusion tokens, unknown keys, and a token supplied only in a cookie.
  `npm run test:auth` proves this by minting good and forged tokens against a mock Access (see
  [Development](#development)).
- **Closed by default:** misconfiguration (placeholder team/audience, unreachable keys) means `403`, never open.
- **Cross-site requests:** writes whose `Origin` isn't this site are refused, API responses are `no-store`, and the app
  shell is served with a strict Content-Security-Policy (`web/public/_headers`).
- **Encryption:** the CLI and the Worker use TaskChampion's scheme (PBKDF2-HMAC-SHA256 → ChaCha20-Poly1305). The
  Worker holds the secret to decrypt on your behalf, which is a deliberate trade-off: whoever can read Worker
  secrets in your Cloudflare account can read your tasks. It is not end-to-end encrypted from Cloudflare.
- The R2 token on your machines (for the CLI) is separate from the Worker, which uses a binding and holds no token.
- The taskrc importer never stores, logs or returns sync/credential values; there are tests that feed it a file full
  of secrets and assert none appear anywhere.

## Development

```
crates/tc-core/   protocol, crypto, filter/report/urgency engine, command parser, taskrc  (pure Rust)
crates/worker/    the Cloudflare Worker: routes, Access check, R2 store, session cache
web/              the Svelte 5 app (Vite)
scripts/          seed-dev.sh (demo data) · interop-local.sh (real-CLI regression)
assets/           the logo master
```

```bash
npm test                      # cargo test + the web unit tests
npm run check                 # svelte-check (types, accessibility)
npm run test:auth             # Cloudflare Access: forged, expired, wrong-audience tokens... all refused
./scripts/interop-local.sh    # real `task` ⇄ the Worker, both directions
```

`test:auth` needs only Node and a built Worker (`cd crates/worker && worker-build --release`). It starts a mock
Access, runs the Worker with the dev bypass off, and checks a matrix of ~30 requests.

`interop-local.sh` needs `task`, the `aws` CLI, `sqlite3`, a built Worker, and a local S3-compatible server that
enforces conditional writes, for example
`docker run -d --name tw-s3 -p 18333:8333 chrislusf/seaweedfs server -s3 -dir=/data`. It is self-contained (own port,
own storage) and does not touch a dev server you have running.

Taskwarrior's behaviour is ported from its source (`sort.cpp`, `Task.cpp`, `CLI2.cpp`) and checked by tests, so when
changing filters, sorting, urgency or journalling, read the C++ first.

## Troubleshooting

| Symptom | Likely cause |
| --- | --- |
| `403` from `/api/*` after deploying | `TEAM_DOMAIN` / `POLICY_AUD` not set or wrong (the `iss`/`aud` of the token must match them exactly), or you're not signed in through Access. The Worker log says why (`auth rejected: …`) |
| "task storage error" (502) | Wrong `TC_ENCRYPTION_SECRET` or the wrong bucket. The cached state resets on the next request |
| `wrangler dev` says the assets directory is missing | Run `npm --prefix web run build` once |
| `task sync` fails with `PermanentRedirect` | `AWS_ENDPOINT_URL` isn't set in that environment, so it's talking to AWS |
| Banner: "saved taskrc settings couldn't be read" | The stored settings are corrupt; nothing was deleted. Open **taskrc** and save again (the unreadable copy is kept aside) |
| Reminders never appear | The tab must be open; the page needs HTTPS or `localhost`; check the browser's notification permission |
| Vite starts on another port | 5173 is in use; that's fine, use the port it prints |
| "the server is busy; try again" | Many commands arrived at once and waited too long for the shared replica; retry |

## Known limitations

- **Not yet tested against real R2.** The CLI's own use of R2's conditional writes is a good sign, but run a
  `task sync` and a web edit against a scratch bucket before trusting it with data you can't lose.
- **Recurring tasks** aren't supported yet (`recur:` is rejected with a clear message); existing ones display.
- **Snapshots and cleanup** of old versions are left to the CLI.
- `dateformat*` settings are stored but the UI shows ISO dates.
- Single user: one set of settings and one shared replica per Worker instance.
