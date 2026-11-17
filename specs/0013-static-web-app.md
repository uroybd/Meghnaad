# 0013: The web app as static files, on a git branch, unlocked with a password

- **Status:** draft
- **Created:** 2026-11-17
- **Layer:** both (hosts for `tc-core` in the browser: sync, settings routes and hooks in the page; the unlock screen)
- **Front ends:** web (a third flavour; Android and the terminal app are unchanged)
- **Supersedes / related:** needs 0011 (`tc_core::gitsync`, `tc_core::forge`) and 0012 (hook secrets bound to hosts)

## Why

The Worker flavours put a server between the page and the tasks. That server is the one place that holds the encryption
secret and sees every task in the clear. That is a good default: it keeps the secret out of the browser, and Access
gives a real sign-in. But it means trusting Cloudflare and the Cloudflare account. Some people would rather no server
ever saw their tasks, and would rather host nothing but files: GitHub Pages, any static host, or a folder.

With the git backend the browser can do everything itself. It talks to the forge's API directly (GitHub's allows
CORS), holds the TaskChampion replica as it does now, and seals and opens versions with a key derived from a password
typed for each session. That is the phone's model (everything on the device), in a browser.

## What

- **A third flavour, `static`:** the built page and nothing else. No Worker, no Access, no bucket. It is deployed by
  copying `web/dist` (or the release package's `static/`) to any static host. A GitHub Pages workflow example is in the
  docs.
- **First visit on a device:** a **Connect** screen asks for:
  - the repository (`owner/name`), the branch (`taskchampion`) and the API address (GitHub's, or a Gitea or Forgejo
    `…/api/v1`);
  - a fine-grained token for that repository with Contents read and write;
  - the encryption secret (`sync.encryption_secret`).

  It tries them with a first sync. The token is then kept in IndexedDB, sealed with a key derived from the encryption
  secret.
- **Every later visit:** an **Unlock** screen asks for the encryption secret only. A wrong one says so and nothing
  changes. **Forget this device** removes the sealed token and the saved replica.
- **Locked again** when the tab closes, after an idle time (default 30 minutes, set in Settings), or with **Lock** in
  the menu. Locking drops the key and the token from memory; the saved replica stays, sealed.
- **Everything else is the web app as it is:** pages, Console, boards, review, settings and taskrc files (kept under
  `web/` on the branch, as the git Worker keeps them), and hooks.
- **Hooks run in the page.** The Lua runner (`omnilua`, pure Rust) is a second WebAssembly module, downloaded only when
  the saved script defines hooks. Secrets are `web/hook-secrets.enc`, opened in the page, and go only to their hosts
  (0012). A hook's `http` is the page's `fetch`, so a host that doesn't allow CORS can't be called. The hooks editor
  says so.
- **A template to deploy from.** `templates/static/` in this repository, published as a GitHub template repository
  ("Use this template"), holds no code of ours, only:
  - a Pages workflow that downloads the static build of a chosen release (`MEGHNAAD_VERSION`, or the latest), checks its
    checksum, and publishes it to GitHub Pages;
  - a `config.json` with the defaults the Connect screen fills in (the tasks repository, branch and API address; never a
    token);
  - a README with the steps.

  Updating means changing the version, or a scheduled run that takes the latest release. The same folder deploys to
  Cloudflare Pages or Netlify (their build command runs the same download script).
- **One branch for everything:** the same branch serves `task` (`sync.git.*`), the phone and the git Worker, so a person
  can use any mix.

## Not doing

- **A bucket from the page** (S3 with keys in the browser, given CORS on the bucket): possible later, not now.
- **GitHub OAuth sign-in in v1.** GitHub's token exchange doesn't allow CORS, so it needs a server. See the open
  questions.
- **GitHub repository (Actions) secrets:** they can't be read back by any API.
- **Protection from code running in an unlocked page** (extensions, an injected script, a tampered build of the static
  files). Like any web app, it trusts the code it serves and the browser it runs in. The docs say so, and the measures
  below only make a leak worth less.
- **Changes to the Worker flavours, Android or the terminal app.**

## Design

**Where the code goes.**

- **`crates/browser`** is still the one browser engine, with a second sync path chosen at connect time:
  - `RemoteServer` (wire, through a Worker) for the Worker flavours;
  - for `static`, `GitServer` over a `FetchHttp`, a `tc_core::forge::Http` over the page's `fetch`, the same as the
    Worker's `gitrepo.rs` but in a Web Worker.

  The page's settings routes are answered in the page, as the phone answers them: `tc_core::webapi::WebSettings` over
  `GitObjects`, through the engine's `api` call. `web/src/lib/api.ts` already has the engine path.
- **Key and token.**
  - The key is `Cryptor::new(salt, secret)`, with the salt from `meta` on the branch. The token is sealed with it under
    an AAD of its own and kept in IndexedDB beside the saved replica, and the replica is sealed with the same key.
  - In memory, the key is a non-extractable WebCrypto key where WebCrypto can do the job. TaskChampion's ChaCha20 runs
    in the engine, so the raw key does exist in the engine's memory while unlocked.
  - The derivation is the same PBKDF2 (600,000 rounds) that TaskChampion uses, so a stolen IndexedDB costs a guesser as
    much as a stolen bucket does.
- **Hooks.** A second wasm (`crates/browser-hooks`, or a feature build of `crates/browser`) with `lua_hooks` and a
  `Net` over `fetch`. It is loaded the first time a hook is to run, which keeps the first download the size it is.
  `HookHost` in the engine calls it in place of `RemoteHooks`.
- **Addresses.** Every `route.ts` `Place` works under a base path (GitHub Pages serves `/<repo>/`). The page falls back
  from `404.html` for deep links on hosts without single-page routing.
- **Security measures.**
  - A Content-Security-Policy in the page's `<meta>`: scripts only from the site, `connect-src` the forge's API plus
    `https:` for hooks. Hooks' hosts are only known at run time; the narrower form is an open question.
  - The idle lock.
  - A token for one repository only, which the docs require and Connect checks where the forge says (GitHub's
    `x-oauth-scopes` / fine-grained permissions headers).
- **Template.** `templates/static/`: `.github/workflows/pages.yml` (download, verify against the release's
  `SHA256SUMS`, upload with `actions/upload-pages-artifact`, deploy with `actions/deploy-pages`), `fetch-release.mjs`
  (the same download for other hosts, no dependencies), `config.json`, and a README. A script test checks that the
  template's download script and the release's file names agree, as `assemble` is checked now.
- **Build and release.** `scripts/build.mjs --flavour static` builds the page with `VITE_FLAVOUR=static` (no `/api`
  calls; the unlock and connect screens). The release package gains `static/`, and `deploy.mjs` is not involved.
- **Sizes.** Engine: plus `gitsync`, `forge` and `webapi` (part is already in). The hooks wasm is separate. Both are
  measured.

## Acceptance criteria

- [ ] A static build served from a plain static server (no Worker) connects to a GitHub repository and to a Forgejo
      in Docker. It syncs both ways with the real `task` 3.5.0 (`sync.git.*`) and with the git Worker on the same
      branch, including a snapshot written by one and read by another.
- [ ] Unlock: a later visit asks only for the encryption secret; a wrong one changes nothing; **Lock**, the idle lock
      and closing the tab drop the key and token from memory; **Forget this device** clears IndexedDB.
- [ ] Nothing secret is stored in the clear: a test reads IndexedDB after connecting and finds no token, secret or task
      text.
- [ ] Settings, taskrc files, `settings.json` and hooks are read and saved on the branch in the page. The git Worker and
      the phone see the same files.
- [ ] Hooks run in the page with secrets bound to hosts (0012). The Lua wasm is not downloaded until a hook runs.
- [ ] Works under a base path (GitHub Pages): every page address round-trips and a deep link loads.
- [ ] The template: a repository made from it, with Pages turned on, serves the app after one workflow run, and a
      version change updates it. Its download script works for Cloudflare Pages too.
- [ ] Tests: browser engine tests for the git path, web tests for the connect and unlock screens and the lock,
      `scripts/*.test.mjs` for the static build and package.
- [ ] Docs: README (the three flavours and how they differ in trust), `docs/deploy.md` (static hosting, the template,
      GitHub Pages),
      `docs/architecture.md` (In the browser, both diagrams), `docs/using/` (unlocking, locking, forgetting a device).
- [ ] Size: engine wasm measured before and after; the hooks wasm measured.

## Open questions

- **Sign-in with GitHub (OAuth)** instead of a pasted token. GitHub's token exchange has no CORS, so it needs a tiny
  server, about 20 lines on a Worker or Pages Function, that holds no data. Ship one as an optional extra, or stay
  with tokens?
- **Unlock with a passkey** (the WebAuthn PRF extension) as well as the encryption secret?
- **Tabs:** does each tab ask to unlock, or do unlocked tabs share the key over `BroadcastChannel`?
- **CSP for hooks:** list the hosts the secrets are bound to (0012) at build time, or allow `https:`?
- **Where the template and the tasks live:** GitHub Pages from a private repository needs a paid plan, and the tasks
  must be in a private one. So the default is two repositories: a public site from the template and a private tasks
  repository. Should the template also offer one private repository for both, deployed by Cloudflare Pages or
  Netlify (free for private repositories), with the site on `main` and the tasks on `taskchampion`?
- **Rate:** the page pulls every 30 seconds while visible, one request each when idle (120 an hour a tab, against
  GitHub's 5,000). Keep it, or slow it down for `static`?

## Implementation notes

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.
