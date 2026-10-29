# Deploying to Cloudflare

## What you need

| | Plan | Why |
| --- | --- | --- |
| **Workers** | Free is fine | Workers Free allows 10 ms of CPU per request. The Worker only checks Access and opens and seals versions (the engine runs in your browser), but the first request after an idle spell derives the encryption key, which can be cut off with error 1102. Cloudflare tolerates an occasional overrun and the app retries. Workers Paid ($5/month) lifts the limit. See [Limits of the platform](architecture.md#limits-of-the-platform) |
| **R2** | Free is fine | The bucket your `task` CLI syncs to |
| **Access** | Free is fine | Sign-in (one-time PIN works with no setup) |

On your machine you need Node.js 20+ for every route. Routes A and C also need [Rust](https://rustup.rs) when you deploy
from a clone of the source: the build compiles the Worker and the in-browser engine to WASM, and installs the
`wasm32-unknown-unknown` target, `worker-build` and `wasm-bindgen` (at the version in `Cargo.lock`) if missing. The first
build takes a few minutes. **Route 0 needs no Rust**: it deploys a prebuilt release. Route B builds on Cloudflare, and
route D on GitHub.

Pick one route to start. Route D is for updating after A, C or 0 has done the first deploy.

## 0. Prebuilt: no Rust, no build

Every release is built once by CI and published ready to deploy: to the `release` branch and, for a tagged version, as a
`.tar.gz` on the [releases page](https://github.com/uroybd/Meghnaad/releases). Nothing about your hooks, secrets or
settings is in the build (they live in your bucket), so everyone deploys the same files.

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

## A. Guided: `npm run setup` (recommended)

```bash
npm install
npm run setup
```

It checks your build tools (offering to install what's missing), logs you in with `wrangler login`, then asks:

| Question | Notes |
| --- | --- |
| **R2 bucket** | Your **existing** sync bucket is used as is; a name that doesn't exist yet is created. Nothing in the bucket is changed |
| **Custom domain** | Optional, e.g. `tasks.example.com`. The domain must already be a zone on your Cloudflare account. Empty means the free `*.workers.dev` address. With a custom domain the `workers.dev` address is switched off |
| **Encryption secret** | The same value as `sync.encryption_secret` in your taskrc (for a new bucket, invent a strong passphrase and use it for the CLI too). Stored as a Worker secret |
| **Cloudflare Access** | It tells you exactly what to click (below), asks for the team domain and AUD tag, then deploys again |

Your answers are saved in `.deploy.vars` (git-ignored). Run **`npm run deploy`** to update, or `npm run setup` again to
change something; both are safe to repeat. `npm run deploy -- --check` prints the exact config that would be deployed.
Any key can be an environment variable instead (handy in CI).

**You can stop after the first deploy.** Until Access is connected, the app refuses every request and shows a **setup
page** with your hostname filled in, explaining what's left. Nothing about your tasks is reachable.

### Connecting Access

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

## B. From the dashboard (no terminal)

Fork this repo, then in the Cloudflare dashboard go to **Workers & Pages → Create → Import a repository**. Set the build
command to `node scripts/build.mjs` and leave the deploy command as `npx wrangler deploy`. Before the first build, edit
`wrangler.jsonc` in your fork if you need a different bucket (`bucket_name`) or a custom domain (`routes`).

Then open the deployed address. The setup page walks you through Access and the two **Text variables** (`TEAM_DOMAIN`,
`POLICY_AUD`) to add under *Settings → Variables and Secrets*; add `TC_ENCRYPTION_SECRET` there as a **Secret**.
(`keep_vars` is on, so redeploys don't remove what you added.)

The **Deploy to Cloudflare** button is route 0, which uses the prebuilt `release` branch and needs no Rust. This route is
for building on Cloudflare from your own fork, for example to carry your own changes.

## C. By hand

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

## D. From GitHub Actions (manual)

The repository has a workflow, [`.github/workflows/deploy.yml`](https://github.com/uroybd/Meghnaad/blob/main/.github/workflows/deploy.yml),
that builds and deploys the Worker when you start it: **Actions → Deploy → Run workflow**. It never runs on a push. Do
route 0, A or C **once** first, since the bucket and Access have to exist and the first deploy is where you find out they
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
pull request, so deploy a commit whose CI is green. Run it from the branch you want to deploy; for your own hooks that is
your `my-hooks` branch (see [Hooks](using/hooks.md)). Runs are serialised, so two clicks never deploy at once.

**Last step, whichever way:** open the URL, sign in, and import your `taskrc` (**Settings → taskrc**) to get your UDAs and
custom reports.

## Making a release (for maintainers)

The version is the one in the root `package.json`. The web app shows it at the bottom of **Settings**, the Android app
takes it as its version name (and a code that rises with it), and the prebuilt package carries it. To release:

```bash
# 1. set "version" in package.json (for example 0.2.0), commit it, push, and let CI pass
git tag v0.2.0 && git push origin v0.2.0
```

Pushing the tag runs the **Release** workflow, which refuses a tag that isn't `v` plus the version in `package.json`. It
builds once and publishes:

- a new commit on the `release` branch holding the built Worker (`worker/`), the web app and engine (`app/`), the scripts
  and a `wrangler.jsonc` without a build step (the branch only gains commits; nothing is force-pushed),
- a GitHub release titled with the version, holding the same files as `meghnaad-<version>.tar.gz`,
- the Android APK, added to that release when its build finishes ([signing it](android.md#releases-and-signing)).

**A Homebrew tap.** The release can also keep the `meghnaad-hook` formula up to date in a tap, so people install it with
`brew install <owner>/tap/meghnaad-hook`. It is off until you set the repository **variable** `HOMEBREW_TAP` to the tap
(`owner/homebrew-tap`) and add the **secret** `HOMEBREW_TAP_TOKEN`: a fine-grained token with **Contents: read and write**
on that one repository (the workflow's own token can't push to another). For a release version (not `1.2.3-rc.1`) the
`tap` job waits for the four hook builds, writes `Formula/meghnaad-hook.rb` (`scripts/homebrew-formula.mjs`, with the URL
and sha256 of each platform's archive) and pushes it as "Update meghnaad-hook to v`<version>`". If a platform's build
fails, no formula is written; re-run the failed jobs and it follows. Without the token it only warns.

**Trying a build first.** **Actions → Release → Run workflow** builds the same things as `<version>-dev.<commit>` and only
updates the `release` branch (the APK stays as the run's artifact). `node scripts/build.mjs && node
scripts/package-release.mjs` produces the same package in `release/` on your machine. The workflow doesn't run the tests,
so release a commit CI has passed.

**The documentation site.** These docs are an mdBook (`docs/book.toml`) published to GitHub Pages by the **Docs**
workflow on a push to `main` that touches `docs/`. Set the custom domain in the repository's Pages settings, and the
repository variable `DOCS_DOMAIN` so the sitemap uses it.

## The Android app

It doesn't use the Worker or Access: it syncs straight with the bucket, so deploying the web app isn't a step toward it.
Install the APK from the [latest release](https://github.com/uroybd/Meghnaad/releases/latest) and give it the same bucket
and encryption secret ([the Android app](android.md)). With the web app deployed, the two can also share settings and
hooks through the bucket (**Settings → Sync** on the phone).

## Connect your `task` CLI

Skip this if your CLI already syncs to the bucket. Otherwise create an **R2 API token** (R2 → *Manage R2 API Tokens* →
**Object Read & Write**, scoped to your bucket), note your account ID, then:

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
`AWS_ENDPOINT_URL` from the environment, which is what points it at R2. Set it **everywhere `task sync` runs** (your
shell, cron, hooks, GUI launchers), or `task` quietly talks to real AWS and fails with a `PermanentRedirect` error. Newer,
unreleased Taskwarrior builds add `sync.aws.endpoint_url`.

## Configuration reference

| Name | Kind | Purpose |
| --- | --- | --- |
| `TASKS` | R2 binding (`wrangler.jsonc`) | The bucket your `task` CLI syncs to |
| `TC_ENCRYPTION_SECRET` | Worker secret | Same value as the CLI's `sync.encryption_secret` |
| `TEAM_DOMAIN` | var (not in `wrangler.jsonc`) | `https://<team>.cloudflareaccess.com`. Set by `npm run setup` (via `.deploy.vars`) or as a Text variable in the dashboard |
| `POLICY_AUD` | var (not in `wrangler.jsonc`) | The Access application's AUD tag; set the same two ways |
| `R2_BUCKET`, `CUSTOM_DOMAIN`, `WORKER_NAME` | `.deploy.vars` | Optional deploy choices |
| `DEV_AUTH_BYPASS` | `.dev.vars` only | Skips the Access check for local development. Honoured only for loopback hosts (`localhost`, `127.x.x.x`, `::1`); anywhere else it is ignored and logged. Never set it on a deployed Worker |

## Troubleshooting a deployment

| Symptom | Likely cause |
| --- | --- |
| The app shows an "Almost there" page | Access isn't connected yet: follow the page, or run `npm run setup` |
| `403` from `/api/*` after deploying | `TEAM_DOMAIN` or `POLICY_AUD` is unset or wrong (the token's `iss` and `aud` must match exactly), or you're not signed in through Access. The Worker log says why (`auth rejected: …`) |
| **Cloudflare error 1102** ("Worker exceeded resource limits"), usually on the first request after a while | The CPU limit (10 ms on Workers Free), passed by the one request that does the most work. The app retries read-only requests once, and the retry meets a warm instance. If it keeps happening, say with a very large task list, Workers Paid lifts the limit. In the dashboard, "Exceeded CPU Time Limits" vs "Exceeded Memory" says which; `npx wrangler tail` shows it live |
| A stack trace in `wrangler tail` shows only `wasm-function[1234]` | The build leaves out debug function names to save about 70 KB. Build and deploy once with `KEEP_WASM_NAMES=1 npm run deploy` when you need to read one |
| "task storage error" (502) | Wrong `TC_ENCRYPTION_SECRET` or the wrong bucket. The cached state resets on the next request |
| Task command icons show as a terminal, and the icon picker says the list could not be fetched | The browser can't reach `cdn.jsdelivr.net`: it is offline or blocked, or a proxy in front of the app sets a Content-Security-Policy without it (`web/public/_headers` allows it with `connect-src 'self' https://cdn.jsdelivr.net https://holidata.net https://date.nager.at`). Typing an icon's name in the picker still works |
| Every command fails with "The engine could not start in this browser" | The in-page WebAssembly engine was blocked or failed: a proxy sets a Content-Security-Policy without `'wasm-unsafe-eval'` (`web/public/_headers` has `script-src 'self' 'wasm-unsafe-eval'`), or the browser has no Web Workers or WebAssembly. The reason follows the message; nothing on the Worker can run commands instead |
| A banner says "Couldn't sync with the server" | The page couldn't reach `/api/tc/*`: you're offline, Cloudflare Access signed you out (reload to sign in again), or the Worker failed. Your changes are kept in the browser and go up with the next sync that works. **Start over from the server** discards the browser's copy, for a replaced bucket or a changed secret |
| `wrangler dev` says the assets directory is missing | Run `npm --prefix web run build` once |
| `wrangler dev` exits with "register a workers.dev subdomain" | `wrangler.jsonc` has an extra `"remote": true` R2 binding (added when you ran `wrangler r2 bucket create`). Delete it, keeping only `TASKS` |
| `task sync` fails with `PermanentRedirect` | `AWS_ENDPOINT_URL` isn't set in that environment, so it's talking to AWS |
| "the server is busy; try again" | Many commands arrived at once and waited too long for the shared replica; retry |
