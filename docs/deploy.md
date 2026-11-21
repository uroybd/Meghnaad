# Running the web app

The web app is the same page however you host it; what differs is where your tasks live and who holds the encryption
secret. There are three setups, and none is the default:

| | Static site | Cloudflare Worker with R2 | Cloudflare Worker with git |
| --- | --- | --- | --- |
| **Your tasks** | a git repository (GitHub, Gitea, Forgejo) | an R2 bucket | a git repository (GitHub, Gitea, Forgejo) |
| **You need** | any static host (GitHub Pages, Cloudflare Pages, Netlify) and a repository token | Cloudflare Workers, R2 and Access | Cloudflare Workers and Access, and a repository token |
| **Sign-in** | the encryption secret unlocks the page | Cloudflare Access | Cloudflare Access |
| **The secret and token are held by** | your browser: sealed at rest, in memory while unlocked | the Worker, as Cloudflare secrets | the Worker, as Cloudflare secrets |
| **You trust** | the site's code, your browser and its extensions; the forge sees your settings, never your tasks | Cloudflare, and whoever can read your account's Worker secrets | the same; the forge sees your settings, never your tasks |
| **Hooks run** | in the page | on the Worker | on the Worker |
| **Costs** | a free static host | Cloudflare's free plans | Cloudflare's free plans |

Each setup syncs with the same store your `task` CLI uses; pointing `task` at it is in
[Where your tasks live](stores.md). The Android app needs none of them: it syncs with the store directly
([The Android app](#the-android-app)).

## The static app

The web app as a static site, with no server of its own: no Worker, no bucket, no Access. The page reaches your tasks on
a branch of a git repository through GitHub's (or Gitea's, or Forgejo's) API, the same branch your `task` CLI syncs to
with `sync.git.*` ([a git repository](stores.md#a-git-repository)), and decrypts them in the browser. Any static host
serves it: GitHub Pages, Cloudflare Pages, Netlify.

Each release publishes it as `meghnaad-static-<version>.tar.gz` (with a `.sha256`), and a deploy template as
`meghnaad-static-template.tar.gz`. Unpack the template into a new repository: its `fetch-release.mjs` downloads the
release named in `meghnaad-version` (`latest` by default), checks it against its checksum and puts it in `site/`, setting
the `<base>` for the folder the site is served from. Its README has the steps for each host:

- **GitHub Pages:** the template's workflow builds and deploys on every push, by hand, and weekly. Pages for a private
  repository needs a paid plan, so keep the site in its own (public) repository and the tasks in a private one.
- **Cloudflare Pages or Netlify:** build command `node fetch-release.mjs`, output directory `site`. They serve a private
  repository for free, so one private repository can hold the site on `main` and the tasks on another branch.

To build it from source instead: `npm run build:static` writes it to `web/dist-static`.

**Or use the public one.** If your tasks sync with a git repository and you'd rather not host anything,
[mn.utsob.me](https://mn.utsob.me) is the author's own copy of the static site, the release as published, and anyone may
use it with their own repository. The page talks to your repository from your browser, and your token, secret and
tasks stay in your browser and your repository: nothing of yours reaches the site's owner, and nothing of the owner's
reaches you. Its form starts filled with the author's repository, so put yours in its place. As with any web app, you
trust the code it serves; host your own copy to choose that code yourself. For Gitea or Forgejo, allow
`https://mn.utsob.me` in `[cors]` (below).

**Connecting.** The first visit on a device asks for the repository, the branch, the API address (empty for GitHub), a
token that can read and write the repository's contents, and the encryption secret. The token is kept on the device
sealed with the secret (in OpenPGP's symmetric format, so `gpg --decrypt` opens it too); the key is derived from the
secret and the salt in the branch's `meta`. Later visits ask only for the secret, and an unlocked tab unlocks the
others. The device's copy of the tasks is sealed too. **Lock** and an idle time lock every tab; **Settings → General →
Forget this device** removes the token and the copy. A locked tab keeps reminding (below).

**Trusting a device.** *Trust this device*, on the unlock and connect forms and in **Settings → This device**,
keeps the device unlocked: a reload, a new tab or a browser restart opens without the password, and the idle lock is off.
The access and the key are kept sealed with a key the browser makes and page code cannot read out, but the browser may
keep that key in its profile as it is, so anyone who can use this browser (or a copy of its profile) can open your tasks.
Only this browser knows of it: nothing is synced. **Lock now** still locks, and a trusted device then stays locked, a
reload too, until it is unlocked. Turning the switch off, or forgetting the device, removes what was kept.

**A PIN on a trusted device.** **Settings → This device → Set a PIN** takes four keys (the arrows, letters and
digits; a capital counts as its lowercase letter), pressed twice; it is saved as the second four match, with no button.
After the idle minutes you choose there (5 to begin with; 0 for only when asked), or at once with **Lock with PIN now**
or `:lock` (`:lock pin` to ask for it; `:lock key` and the lock button in the top bar lock fully), every tab shows a frosted screen (a reload opens on it too) over the app, and the four keys take it away again (Backspace takes one
back; there is no OK). The app runs on underneath: sync, reminders, a half-typed edit. It is a visual block, not a lock:
it seals nothing, and anyone who can open the browser's developer tools gets past it. **Forgot the PIN?** locks fully,
as **Lock now** does. Only this browser knows of the PIN; turning trust off removes it.

**Unlocking with a passkey.** **Settings → This device → Unlock with a passkey on this device** makes a passkey
for the site (in the browser, the phone, or a password manager such as Bitwarden), and the unlock form then offers it
before the password. The password always still works. On a device that is not trusted the passkey must give a secret
of its own (WebAuthn's PRF extension), which seals the access and the key; a browser or password manager that does not
give it is told so, and nothing is kept. On a trusted device any passkey works: its signature is checked, and it then
opens the trusted copy, so it guards the screen after **Lock now**, not the data (turning trust off removes it). Each
device sets up its own: a passkey synced to another device unlocks nothing there until it is set up there too. **Stop
unlocking with a passkey** removes what was kept; the passkey itself stays in your password manager. `config.json` beside the site fills in the form. A password
manager can keep the token and the secret as one login (the token as the username, the secret as the password): the
form marks them so, Chrome and Edge offer to save them once connected, and a new device fills both from the saved login.

**Gitea and Forgejo** must allow the page to call their API: `[cors]` with `ENABLED = true`, `ALLOW_DOMAIN` the site's
host and `ALLOW_HEADERS = Content-Type,Authorization`. GitHub allows it already.

**How it differs from the Worker.**

- The token and the encryption secret are in the browser (sealed at rest, in memory while unlocked), not on a server.
  What an unlocked page runs can read them: the site's own code, the browser and its extensions. A trusted device keeps
  them openable by its browser profile.
- Hooks run in the page (a WebAssembly Lua runner loaded on first use). Their network requests and an importer's or a
  calendar's URL are fetched by the browser, so those hosts must allow CORS.
- The settings, the hooks script and the sealed hook secrets are kept on the branch under `web/`, as the git Worker
  keeps them, so the repository must be private.

## The Cloudflare Worker

A small Worker on Cloudflare serves the page and sits between it and your store: it checks Cloudflare Access (the
login), holds the encryption secret, and opens and seals the versions that pass through; it never parses a task. It
comes in two builds, one for an R2 bucket and one for a git repository
([below](#a-git-repository-instead-of-r2)); every route here deploys either, except the button, which deploys the R2 one.

### What you need

| | Plan | Why |
| --- | --- | --- |
| **Workers** | Free is fine | Workers Free allows 10 ms of CPU per request. The Worker only checks Access and opens and seals versions (the engine runs in your browser), but the first request after an idle spell derives the encryption key, which can be cut off with error 1102. Cloudflare tolerates an occasional overrun and the app retries. Workers Paid ($5/month) lifts the limit. See [Limits of the platform](architecture.md#limits-of-the-platform) |
| **R2** | Free is fine | The bucket your `task` CLI syncs to (not needed with [a git repository](#a-git-repository-instead-of-r2)) |
| **Access** | Free is fine | Sign-in (one-time PIN works with no setup) |

On your machine you need Node.js 20+ for every route. Routes A and C also need [Rust](https://rustup.rs) when you deploy
from a clone of the source: the build compiles the Worker and the in-browser engine to WASM, and installs the
`wasm32-unknown-unknown` target, `worker-build` and `wasm-bindgen` (at the version in `Cargo.lock`) if missing. The first
build takes a few minutes. **Route 0 needs no Rust**: it deploys a prebuilt release. Route B builds on Cloudflare, and
route D on GitHub.

Pick one route to start. Route D is for updating after A, C or 0 has done the first deploy.

### 0. Prebuilt: no Rust, no build

Every release is built once by CI and published ready to deploy: to the `release` branch and, for a tagged version, as a
`.tar.gz` on the [releases page](https://github.com/uroybd/Meghnaad/releases). Nothing about your hooks, secrets or
settings is in the build (they live in your store), so everyone deploys the same files.

**With the button** (nothing to install):

[![Deploy to Cloudflare](https://deploy.workers.cloudflare.com/button)](https://deploy.workers.cloudflare.com/?url=https://github.com/uroybd/Meghnaad/tree/release)

Choose your R2 bucket (your existing sync bucket, or a new one) and give the encryption secret (the same value as
`sync.encryption_secret`). Then open the address it gives you and follow the setup page to connect Access
([Connecting Access](#connecting-access)).

**From a terminal** (Node.js 20+ only):

```bash
git clone --branch release --depth 1 https://github.com/uroybd/Meghnaad.git meghnaad
cd meghnaad
npm install
npm run setup        # the same guided questions as route A, without the build
```

To update, pull the `release` branch (or download the new release) and run `npm run deploy`. Your answers are kept in
`.deploy.vars`, which `git pull` never touches because it isn't tracked.

### A. Guided: `npm run setup`

```bash
npm install
npm run setup
```

It checks your build tools (offering to install what's missing), logs you in with `wrangler login`, then asks:

| Question | Notes |
| --- | --- |
| **Where the tasks are kept** | `r2` (a bucket) or `git` (a branch of a git repository: [below](#a-git-repository-instead-of-r2)). With `git` it asks for the repository, branch, API address and token instead of a bucket |
| **R2 bucket** (`r2`) | Your **existing** sync bucket is used as is; a name that doesn't exist yet is created. Nothing in the bucket is changed |
| **Custom domain** | Optional, e.g. `tasks.example.com`. The domain must already be a zone on your Cloudflare account. Empty means the free `*.workers.dev` address. With a custom domain the `workers.dev` address is switched off |
| **Encryption secret** | The same value as `sync.encryption_secret` in your taskrc (for a new store, invent a strong passphrase and use it for the CLI too). Stored as a Worker secret |
| **Cloudflare Access** | It tells you exactly what to click (below), asks for the team domain and AUD tag, then deploys again |

Your answers are saved in `.deploy.vars` (git-ignored). Run **`npm run deploy`** to update, or `npm run setup` again to
change something; both are safe to repeat. `npm run deploy -- --check` prints the exact config that would be deployed.
Any key can be an environment variable instead (handy in CI).

**You can stop after the first deploy.** Until Access is connected, the app refuses every request and shows a **setup
page** with your hostname filled in, explaining what's left. Nothing about your tasks is reachable.

#### Connecting Access

This is the one part that needs the dashboard, because the AUD tag only exists once the application does:

1. Cloudflare dashboard → **Access controls → Applications → Add an application → Self-hosted**.
2. Destination: your hostname (the custom domain if you set one, otherwise the `workers.dev` address).
3. Add an **Allow** policy with the selector **Emails** and your address. *One-time PIN* login works without setup.
4. Copy the application's **Application Audience (AUD) tag**. Your team domain, `https://<team>.cloudflareaccess.com`, is
   under Access controls → **Settings**.

Access signs people in; the Worker then **re-verifies the signed token on every API call** (signature, issuer, audience,
expiry), so your data stays protected even if someone finds a way around Access. Make the Access application cover the
**whole hostname**: the static app shell (HTML and JavaScript, nothing private) is served without running the Worker, so
Access is what guards it.

### B. From the dashboard (no terminal)

For the R2 Worker. Fork this repo, then in the Cloudflare dashboard go to **Workers & Pages → Create → Import a repository**. Set the build
command to `node scripts/build.mjs` and leave the deploy command as `npx wrangler deploy`. Before the first build, edit
`wrangler.jsonc` in your fork if you need a different bucket (`bucket_name`) or a custom domain (`routes`).

Then open the deployed address. The setup page walks you through Access and the two **Text variables** (`TEAM_DOMAIN`,
`POLICY_AUD`) to add under *Settings → Variables and Secrets*; add `TC_ENCRYPTION_SECRET` there as a **Secret**.
(`keep_vars` is on, so redeploys don't remove what you added.)

The **Deploy to Cloudflare** button is route 0, which uses the prebuilt `release` branch and needs no Rust. This route is
for building on Cloudflare from your own fork, for example to carry your own changes.

### C. By hand

For the R2 Worker (the git Worker's steps are [below](#a-git-repository-instead-of-r2)):

```bash
npx wrangler login
npx wrangler r2 bucket create <name>             # skip if the bucket exists
echo "R2_BUCKET=<name>" > .deploy.vars           # skip if it is the default, taskwarrior-sync
npm run deploy                                   # builds, then prints the URL (Access can't be connected before this)
npx wrangler secret put TC_ENCRYPTION_SECRET     # the same value as sync.encryption_secret
# create the Access application, add TEAM_DOMAIN=... and POLICY_AUD=... to .deploy.vars, then:
npm run deploy
```

`.deploy.vars` takes `R2_BUCKET`, `CUSTOM_DOMAIN`, `TEAM_DOMAIN`, `POLICY_AUD` and `WORKER_NAME` (to run a second copy),
one `KEY=value` per line. The tracked `wrangler.jsonc` never needs editing: `npm run deploy` overlays these values into a
git-ignored `wrangler.deploy.jsonc`.

> **Heads-up:** after `wrangler r2 bucket create`, wrangler may offer to add the bucket to `wrangler.jsonc` as a *new*
> binding (named after the bucket, with `"remote": true`). Answer **no**, or delete it afterwards. The app uses exactly
> one binding, `TASKS`, and `wrangler dev` refuses to start with an extra remote binding unless you've registered a
> workers.dev subdomain. (`npm run dev` runs locally, so it ignores remote bindings either way.)

### D. From GitHub Actions (manual)

The repository has a workflow, [`.github/workflows/deploy.yml`](https://github.com/uroybd/Meghnaad/blob/main/.github/workflows/deploy.yml),
that deploys the Worker when you start it: **Actions → Deploy → Run workflow**. When the [Release](#making-a-release-for-maintainers) workflow
has built that commit (a released version), it deploys that build from the `release` branch as it is, waiting for the
release while it is still building; any other commit it builds from source first. It never runs on a push. Do
route 0, A or C **once** first, since the store and Access have to exist and the first deploy is where you find out they
work. After that, updating is one click. In your fork:

1. **Create an API token** at Cloudflare → My Profile → API Tokens, from the *Edit Cloudflare Workers* template (it needs
   Workers Scripts: Edit and, for a custom domain, Workers Routes / Zone access). Note your **Account ID** (Workers &
   Pages overview, right-hand side).
2. **Settings → Environments → New environment → `production`.** Add *required reviewers* if you want an approval click
   before each deploy. Put the secrets and variables below on this environment (or on the repository).
3. Add these:

| Where | Name | Value |
| --- | --- | --- |
| Secret | `CLOUDFLARE_API_TOKEN` | the token from step 1 |
| Secret | `CLOUDFLARE_ACCOUNT_ID` | your account id |
| Secret, *optional* | `TC_ENCRYPTION_SECRET` | the same value as `sync.encryption_secret`. If unset, the Worker keeps the secret it already has from route 0, A or C. Set it only if you want the workflow to be the single source of truth |
| Secret, *optional* | `GIT_TOKEN` | for the git Worker (`BACKEND=git`): the repository token. If unset, the Worker keeps the one it has |
| Variable | `DEPLOY_VARS` | your deployment choices: **the contents of your `.deploy.vars`**, one `KEY=value` per line. Optional; without it the defaults apply |

`DEPLOY_VARS` is a multi-line variable. Paste what `.deploy.vars` holds, for example:

```
# Same choices as `npm run setup` saved on your machine.
R2_BUCKET=taskwarrior-sync
CUSTOM_DOMAIN=tasks.example.com
TEAM_DOMAIN=https://myteam.cloudflareaccess.com
POLICY_AUD=<the 64-character Application Audience tag>
WORKER_NAME=taskwarrior-web
```

All five are optional, `#` comments and blank lines are fine, and each value is checked as `npm run deploy` checks it (a
typo fails the run before anything is built). The workflow writes the variable out as `.deploy.vars`, so it works exactly
like a deploy from your machine. **Use the same values you deployed with**: leaving out `TEAM_DOMAIN` or `POLICY_AUD`
later removes nothing (`keep_vars`), but a different `R2_BUCKET` or `WORKER_NAME` points at a different bucket or Worker.

It is a variable, not a secret, because none of it is secret (a team domain and an AUD tag are in every request's token).
The encryption secret and the API token are, and only the steps that need them receive them.

The workflow's one option is **Dry run**: it only prints the Worker config your settings would produce, the quickest way
to check your variables, and builds and deploys nothing. It doesn't run the tests, because the
[CI workflow](https://github.com/uroybd/Meghnaad/blob/main/.github/workflows/ci.yml) already runs them on every push and
pull request, so deploy a commit whose CI is green. Run it from the branch you want to deploy (your hooks are not part of
the build: they live in your store, [Hooks](using/hooks.md)). Runs are serialised, so two clicks never deploy at once.

**Last step, whichever route:** open the URL, sign in, and import your `taskrc` (**Settings → taskrc**) to get your UDAs and
custom reports.

### A git repository instead of R2

`task` 3.5.0 can sync to a git repository (`man task-sync`, `sync.git.*`), and so can Meghnaad: every release carries a
second Worker, built to keep everything on a branch of a git repository on **GitHub**, **Gitea** or **Forgejo** instead of
in R2. The page is the same. The Worker reaches the repository through the forge's API with a token, so it needs no
bucket and no `git`. `task`, the git Worker and the phone can all share one branch.

| Setting | Where | Default | What |
| --- | --- | --- | --- |
| `BACKEND` | `.deploy.vars` | `r2` | `git` deploys the git Worker |
| `GIT_REPO` | `.deploy.vars` | (needed) | `owner/name`, such as `alice/tasks` |
| `GIT_BRANCH` | `.deploy.vars` | `taskchampion` | The branch, the same as `sync.git.branch` |
| `GIT_API` | `.deploy.vars` | `https://api.github.com` | `https://<host>/api/v1` for Gitea or Forgejo, `https://<host>/api/v3` for GitHub Enterprise |
| `GIT_TOKEN` | Worker secret | (needed) | A token that may read and write the repository's contents |

`npm run setup` asks for all of them (answer `git` to the first question) and sets the token as a secret. By hand:

```bash
printf 'BACKEND=git\nGIT_REPO=alice/tasks\n' > .deploy.vars
npm run deploy
npx wrangler secret put GIT_TOKEN
npx wrangler secret put TC_ENCRYPTION_SECRET
```

- **The token.** On GitHub, a fine-grained personal access token for that one repository, with **Contents: Read and
  write** (nothing else). On Gitea and Forgejo, an access token with the **repository** scope, read and write.
- **Keep the repository private.** Tasks and hook secrets are encrypted, but the settings, the taskrc files and the hooks
  script are kept under `web/` on the same branch as plain text.
- **On GitHub the repository needs at least one commit** (create it with a README) so the branch can be made from it;
  Gitea and Forgejo make the branch from nothing. The branch is made on the first sync if it isn't there.
- **The Deploy to Cloudflare button deploys the R2 Worker only.** For git, use route 0 from a terminal, A or C.
- From GitHub Actions (route D), set the repository variables as `DEPLOY_VARS` and the token as the `GIT_TOKEN` secret.
- History grows with every sync, as it does with the CLI: removed versions stay in older commits.
- GitHub allows 5,000 API requests an hour per token. An idle sync is one request and a change three or four, so the
  page's sync every 30 seconds while it is visible stays far below it.

To point `task` at the same branch, see [a git repository](stores.md#a-git-repository).

## The Android app

It uses no Worker, no Access and no static site: it syncs directly with your store, so hosting the web app isn't a step
toward it. Install the APK from the [latest release](https://github.com/uroybd/Meghnaad/releases/latest) and give it the
same repository, bucket or sync server and encryption secret ([the Android app](android.md)). The two can also share
settings and hooks through the bucket or the git branch (**Settings → Sync** on the phone).

## Connect your `task` CLI

Skip this if your CLI already syncs to the store. Otherwise [Where your tasks live](stores.md) has the settings for each:
[a git repository](stores.md#a-git-repository), [an S3 or R2 bucket](stores.md#an-s3-or-r2-bucket) (with
`AWS_ENDPOINT_URL` for R2) or [a sync server](stores.md#a-taskchampion-sync-server).

## Configuration reference

| Name | Kind | Purpose |
| --- | --- | --- |
| `TASKS` | R2 binding (`wrangler.jsonc`) | The R2 Worker's bucket, the one your `task` CLI syncs to |
| `TC_ENCRYPTION_SECRET` | Worker secret | Same value as the CLI's `sync.encryption_secret` |
| `TEAM_DOMAIN` | var (not in `wrangler.jsonc`) | `https://<team>.cloudflareaccess.com`. Set by `npm run setup` (via `.deploy.vars`) or as a Text variable in the dashboard |
| `POLICY_AUD` | var (not in `wrangler.jsonc`) | The Access application's AUD tag; set the same two ways |
| `R2_BUCKET`, `CUSTOM_DOMAIN`, `WORKER_NAME` | `.deploy.vars` | Optional deploy choices |
| `BACKEND`, `GIT_REPO`, `GIT_BRANCH`, `GIT_API` | `.deploy.vars` | The git Worker's repository ([A git repository instead of R2](#a-git-repository-instead-of-r2)) |
| `GIT_TOKEN` | Worker secret | The git Worker's token for the repository |
| `DEV_AUTH_BYPASS` | `.dev.vars` only | Skips the Access check for local development. Honoured only for loopback hosts (`localhost`, `127.x.x.x`, `::1`); anywhere else it is ignored and logged. Never set it on a deployed Worker |

## Troubleshooting a deployment

| Symptom | Likely cause |
| --- | --- |
| The app shows an "Almost there" page | Access isn't connected yet: follow the page, or run `npm run setup` |
| `403` from `/api/*` after deploying | `TEAM_DOMAIN` or `POLICY_AUD` is unset or wrong (the token's `iss` and `aud` must match exactly), or you're not signed in through Access. The Worker log says why (`auth rejected: …`) |
| **Cloudflare error 1102** ("Worker exceeded resource limits"), usually on the first request after a while | The CPU limit (10 ms on Workers Free), passed by the one request that does the most work. The app retries read-only requests once, and the retry meets a warm instance. If it keeps happening, say with a very large task list, Workers Paid lifts the limit. In the dashboard, "Exceeded CPU Time Limits" vs "Exceeded Memory" says which; `npx wrangler tail` shows it live |
| A stack trace in `wrangler tail` shows only `wasm-function[1234]` | The build leaves out debug function names to save about 70 KB. Build and deploy once with `KEEP_WASM_NAMES=1 npm run deploy` when you need to read one |
| "task storage error" (502) | Wrong `TC_ENCRYPTION_SECRET`, or the wrong bucket or repository. The cached state resets on the next request |
| Task command icons show as a terminal, and the icon picker says the list could not be fetched | The browser can't reach `cdn.jsdelivr.net`: it is offline or blocked, or a proxy in front of the app sets a Content-Security-Policy without it (`web/public/_headers` allows it with `connect-src 'self' https://cdn.jsdelivr.net https://holidata.net https://date.nager.at`). Typing an icon's name in the picker still works |
| Every command fails with "The engine could not start in this browser" | The in-page WebAssembly engine was blocked or failed: a proxy sets a Content-Security-Policy without `'wasm-unsafe-eval'` (`web/public/_headers` has `script-src 'self' 'wasm-unsafe-eval'`), or the browser has no Web Workers or WebAssembly. The reason follows the message; nothing on the Worker can run commands instead |
| A banner says "Couldn't sync with the server" | The page couldn't reach `/api/tc/*`: you're offline, Cloudflare Access signed you out (reload to sign in again), or the Worker failed. Your changes are kept in the browser and go up with the next sync that works. **Start over from the server** discards the browser's copy, for a replaced store or a changed secret |
| `wrangler dev` says the assets directory is missing | Run `npm --prefix web run build` once |
| `wrangler dev` exits with "register a workers.dev subdomain" | `wrangler.jsonc` has an extra `"remote": true` R2 binding (added when you ran `wrangler r2 bucket create`). Delete it, keeping only `TASKS` |
| `task sync` fails with `PermanentRedirect` | `AWS_ENDPOINT_URL` isn't set in that environment, so it's talking to AWS |

## Making a release (for maintainers)

The version is the one in the root `package.json`. The web app shows it at the bottom of **Settings**, the Android app
takes it as its version name (and a code that rises with it), and the prebuilt package carries it. To release, with
[`just`](https://just.systems) and everything committed on `main`:

```bash
just release          # bump the patch version, commit "chore: bump to <version>", tag v<version>, push both
just release minor    # or major, or an exact version: just release 0.7.0
just release patch tidy   # lint and test first (`just tidy`), and release only if they pass
just ship             # the same, then start the Deploy workflow (`just ship minor tidy` and so on)
```

`release` refuses to run off `main`, with changes not committed, or behind `origin/main`. It runs no checks unless you
pass `tidy` after the level; run `just check` (format, lint, tests, type checks) before committing. By hand it is the
same steps: `npm version <x> --no-git-tag-version` (which writes the version into `Cargo.toml` and `Cargo.lock` too),
commit, then `git tag v<version> && git push origin main v<version>`.

Pushing the tag runs the **Release** workflow, which refuses a tag that isn't `v` plus the version in `package.json`. It
builds once and publishes:

- a new commit on the `release` branch holding the built Worker (`worker/`), the web app and engine (`app/`), the scripts
  and a `wrangler.jsonc` without a build step (the branch only gains commits; nothing is force-pushed),
- a GitHub release titled with the version, holding the same files as `meghnaad-<version>.tar.gz`, whose notes list
  what changed since the release before: each commit's subject, grouped by its conventional type (Features for `feat:`,
  Fixes for `fix:`, then `perf:`, `refactor:` and `doc:`; chores, CI and the bumps are left out). So write the subject
  for someone reading the notes. `just notes` shows them for what is on `main` now (`scripts/release-notes.mjs`),
- the Android APK, added to that release when its build finishes ([signing it](android.md#releases-and-signing)).
- `meghnaad-hook` and `meghnaad` (the terminal app) for Linux and macOS (x86_64 and arm64), as `<program>-<version>-<platform>.tar.gz`,
  added to that release when their builds finish.

**Build times.** A tag's run can read the caches saved on `main` but never another tag's, so the Release workflow
saves none of its own and reads `main`'s: CI's web job builds what the release builds for the web (both Workers and
the static app too) under the shared Rust cache key `web`, and keeps `worker-build` in a cache of its own (keyed on its
version) that the release and the deploy restore in place of building it; the APK reads CI's android job's Rust and Gradle caches.
The terminal programs build without a cache. The Deploy workflow that `just ship` starts deploys the release's own
build once it is on the `release` branch, without building it again.

**A Homebrew tap.** The release can also keep the `meghnaad-hook` and `meghnaad` formulas up to date in a tap, so
people install them with `brew install <owner>/tap/meghnaad-hook` (or `meghnaad`, which brings `task` with it). It is off until you set the repository **variable** `HOMEBREW_TAP` to the tap
(`owner/homebrew-tap`) and add the **secret** `HOMEBREW_TAP_TOKEN`: a fine-grained token with **Contents: read and write**
on that one repository (the workflow's own token can't push to another). For a release version (not `1.2.3-rc.1`) the
`tap` job waits for the four platforms' builds, writes `Formula/meghnaad-hook.rb` and `Formula/meghnaad.rb`
(`scripts/homebrew-formula.mjs`, with the URL and sha256 of each platform's archive) and pushes them as "Update
meghnaad-hook and meghnaad to v`<version>`". The terminal app's formula used to be `meghnaad-tui`: the job also writes the
tap's `formula_renames.json` (`meghnaad-tui` → `meghnaad`) and removes `Formula/meghnaad-tui.rb`, so `brew update` moves an
installed `meghnaad-tui` to `meghnaad`, and the formula keeps a `meghnaad-tui` link to the program for now. If a platform's build fails, no formula is written; re-run the failed jobs and it follows. Without the token it only warns.

**Trying a build first.** **Actions → Release → Run workflow** builds the same things as `<version>-dev.<commit>` and only
updates the `release` branch (the APK stays as the run's artifact). `node scripts/build.mjs && node
scripts/package-release.mjs` produces the same package in `release/` on your machine. The workflow doesn't run the tests,
so release a commit CI has passed.

**The documentation site.** These docs are an mdBook (`docs/book.toml`) published to GitHub Pages by the **Docs**
workflow on a push to `main` that touches `docs/`. Set the custom domain in the repository's Pages settings, and the
repository variable `DOCS_DOMAIN` so the sitemap uses it.
