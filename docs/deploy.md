# Deploying to Cloudflare

## What you need

| | Plan | Why |
| --- | --- | --- |
| **Workers** | **Paid** ($5/month) | The first request after an idle spell derives the encryption key, which takes tens of milliseconds of CPU. Workers Free allows 10 ms per request and ends longer ones with error 1102. See [Limits of the platform](architecture.md#limits-of-the-platform) |
| **R2** | Free is fine | The bucket your `task` CLI syncs to |
| **Access** | Free is fine | Sign-in (one-time PIN works with no setup) |

On your machine: Node.js 20+ and [Rust](https://rustup.rs) for routes A and C (the build compiles the Worker to WASM; the
`wasm32-unknown-unknown` target and `worker-build` are installed for you if missing). Route B builds on Cloudflare.

Pick one route.

## A. Guided: `npm run setup` (recommended)

```bash
npm install
npm run setup
```

It checks your build tools (offering to install what's missing), logs you in with `wrangler login`, then asks for the
things that differ between people:

| Question | Notes |
| --- | --- |
| **R2 bucket** | Your **existing** sync bucket is used as is; a name that doesn't exist yet is created. Nothing in the bucket is changed |
| **Custom domain** | Optional, e.g. `tasks.example.com`. The domain must already be a zone on your Cloudflare account. Empty = the free `*.workers.dev` address. With a custom domain the `workers.dev` address is switched off |
| **Encryption secret** | The same value as `sync.encryption_secret` in your taskrc (new bucket? invent a strong passphrase and use it for the CLI too). Stored as a Worker secret |
| **Cloudflare Access** | It tells you exactly what to click (see below) and asks for the team domain and AUD tag, then deploys again |

Your answers are saved in `.deploy.vars` (git-ignored). Run **`npm run deploy`** whenever you update the code, or
`npm run setup` again to change something; both are safe to repeat. This is also how a change to your [hooks](using.md#hooks) (Rust in `crates/tc-core/src/my_hooks.rs`; [keep them as a patch](using.md#keeping-your-hooks-across-updates) so updates stay easy) reaches the Worker. `npm run deploy -- --check` prints the exact config
that would be deployed. Any of the keys can be given as environment variables instead (handy in CI).

**You can stop after the first deploy.** Until Access is connected, the app refuses every request and shows a **setup
page** explaining what's left, with your hostname filled in. Nothing about your tasks is reachable.

### Connecting Access

The one part that needs the dashboard, because the AUD tag only exists once the application does:

1. Cloudflare dashboard → **Access controls → Applications → Add an application → Self-hosted**.
2. Destination: your hostname (the custom domain if you set one, otherwise the `workers.dev` address).
3. Add an **Allow** policy with the selector **Emails** and your address. *One-time PIN* login works without setup.
4. Copy the application's **Application Audience (AUD) tag**. Your team domain, `https://<team>.cloudflareaccess.com`,
   is under Access controls → **Settings**.

Access signs people in; the Worker then **re-verifies the signed token on every API call** (signature, issuer,
audience, expiry), so your data stays protected even if someone finds a way around Access. Make the Access application
cover the **whole hostname**: the static app shell (HTML and JavaScript, nothing private) is served without running the
Worker, so Access is what guards it.

## B. From the dashboard (no terminal)

Fork this repo to your GitHub account, then in the Cloudflare dashboard: **Workers & Pages → Create → Import a
repository**. Set the build command to `node scripts/build.mjs` (the Rust and web parts) and leave the deploy command as
`npx wrangler deploy`. Before the first build, edit `wrangler.jsonc` in your fork if you need a different bucket
(`bucket_name`) or a custom domain (`routes`). Then open the deployed address: the setup page walks you through Access
and the two **Text variables** (`TEAM_DOMAIN`, `POLICY_AUD`) to add under *Settings → Variables and Secrets*; add
`TC_ENCRYPTION_SECRET` there as a **Secret**. (`keep_vars` is on, so redeploys don't remove what you added.)

A **Deploy to Cloudflare** button (`https://deploy.workers.cloudflare.com/?url=<your public repo URL>`) should also
work, and `package.json` carries the descriptions it shows, but it hasn't been tried with this repo, mainly because the
build needs the Rust toolchain. Treat route B as the supported dashboard path.

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

`.deploy.vars` takes `R2_BUCKET`, `CUSTOM_DOMAIN`, `TEAM_DOMAIN`, `POLICY_AUD` and `WORKER_NAME` (to run a second
copy), one `KEY=value` per line. The tracked `wrangler.jsonc` never needs editing: `npm run deploy` overlays these
values into a git-ignored `wrangler.deploy.jsonc`.

> **Heads-up:** after `wrangler r2 bucket create`, wrangler may offer to add the bucket to `wrangler.jsonc` as a *new*
> binding (named after the bucket, with `"remote": true`). Answer **no**, or delete it afterwards: the app uses exactly
> one binding, `TASKS`, and `wrangler dev` refuses to start with an extra remote binding unless you've registered a
> workers.dev subdomain. (`npm run dev` runs in local mode, so it ignores remote bindings either way.)

**Last step, whichever way:** open the URL, sign in, and import your `taskrc` (the **taskrc** button) to get your UDAs
and custom reports.

## Connect your `task` CLI

Skip this if your CLI already syncs to the bucket. Otherwise create an **R2 API token** (R2 → *Manage R2 API Tokens* →
**Object Read & Write**, scoped to your bucket) and note your account ID, then:

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

## Configuration reference

| Name | Kind | Purpose |
| --- | --- | --- |
| `TASKS` | R2 binding (`wrangler.jsonc`) | The bucket your `task` CLI syncs to |
| `TC_ENCRYPTION_SECRET` | Worker secret | Same value as the CLI's `sync.encryption_secret` |
| `TEAM_DOMAIN` | var (not in `wrangler.jsonc`) | `https://<team>.cloudflareaccess.com`. Set by `npm run setup` (via `.deploy.vars`) or as a Text variable in the dashboard |
| `POLICY_AUD` | var (not in `wrangler.jsonc`) | The Access application's AUD tag; same two ways |
| `R2_BUCKET`, `CUSTOM_DOMAIN`, `WORKER_NAME` | `.deploy.vars` | Optional deploy choices |
| `DEV_AUTH_BYPASS` | `.dev.vars` only | Skips the Access check for local development. Only honoured for loopback hosts (`localhost`, `127.x.x.x`, `::1`); anywhere else it is ignored and logged. Never set it on a deployed Worker |

## Troubleshooting a deployment

| Symptom | Likely cause |
| --- | --- |
| The app shows an "Almost there" page | Access isn't connected yet: follow the page, or `npm run setup` |
| `403` from `/api/*` after deploying | `TEAM_DOMAIN` / `POLICY_AUD` not set or wrong (the `iss`/`aud` of the token must match them exactly), or you're not signed in through Access. The Worker log says why (`auth rejected: …`) |
| **Cloudflare error 1102** ("Worker exceeded resource limits"), usually on the first request after a while | The CPU limit. On Workers Free it is 10 ms, less than a cold start needs: use Workers Paid. The app retries read-only requests once on its own, and the retry meets a warm instance. In the dashboard, "Exceeded CPU Time Limits" vs "Exceeded Memory" says which limit; `npx wrangler tail` shows it live |
| "task storage error" (502) | Wrong `TC_ENCRYPTION_SECRET` or the wrong bucket. The cached state resets on the next request |
| `wrangler dev` says the assets directory is missing | Run `npm --prefix web run build` once |
| `wrangler dev` exits with "register a workers.dev subdomain" | `wrangler.jsonc` has an extra `"remote": true` R2 binding (added when you ran `wrangler r2 bucket create`). Delete it, keeping only `TASKS` |
| `task sync` fails with `PermanentRedirect` | `AWS_ENDPOINT_URL` isn't set in that environment, so it's talking to AWS |
| "the server is busy; try again" | Many commands arrived at once and waited too long for the shared replica; retry |
