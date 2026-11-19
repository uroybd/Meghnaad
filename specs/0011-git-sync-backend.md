# 0011: Git repository as the sync backend

- **Status:** done <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-17
- **Layer:** both (layer 1: a TaskChampion git server in `crates/tc-core`; hosts: the Worker and the phone; layer 2:
  the phone's sync settings)
- **Front ends:** web, Android. The terminal app needs nothing: its backend is the real `task`, which syncs to git
  itself (`sync.git.*`).
- **Supersedes / related:** none

## Why

Some people would rather keep their tasks in a git repository they already have than in an R2 bucket. `task` 3.5.0
already syncs to git (`man task-sync`, "Git Synchronization", through TaskChampion 3.1's `server/gitsync`). The web app
and the phone should be able to share that same repository and branch with the desktop `task`, as they share a bucket
today.

## What

- **Web:** a second Worker flavour that we ship and deploy, "git", beside "r2". `npm run setup` asks which one to use.
  The git flavour takes:
  - `GIT_REPO` (`owner/name`)
  - `GIT_BRANCH` (default `taskchampion`, the dedicated branch the man page recommends)
  - `GIT_API` (default `https://api.github.com`; set it for GitHub Enterprise or a Gitea/Forgejo host)
  - the Worker secret `GIT_TOKEN`, kept beside `TC_ENCRYPTION_SECRET`

  The page, the `/api/*` routes and `tc_core::wire` are the same in both flavours, so the web app does not change.
- **Android:** the sync settings get a third choice, Git, beside Bucket and Sync server, with fields for the API URL,
  the repository, the branch and the token. The token is kept in the Keystore. The settings and hooks files sync to the
  same branch, as they do to the bucket.
- **Desktop:** nothing is built. The docs show how to point `task` at the same repository:

  ```text
  task config sync.git.local_path ~/.task-sync
  task config sync.git.branch     taskchampion
  task config sync.git.remote     git@github.com:owner/name.git
  ```

## Not doing

- Running `git`, or reading and writing packfiles (smart HTTP), in the Worker or on the phone.
- GitLab's API.
- A backend chosen at run time inside one Worker. There are two builds.
- Any change to the terminal app.
- Moving an existing R2 bucket into a git repository (decided 2026-11-17).
- An offline mode on the phone like `sync.git.local_only`.

## Taskwarrior parity

TaskChampion 3.1, `src/server/gitsync/mod.rs`, is what `task` 3.5.0 runs. Its layout is **not** our bucket layout
(`tc_core::cloud`, `names.rs`), so the git backend has to follow it file for file:

| | bucket (ours) | git (`gitsync`) |
| --- | --- | --- |
| latest and salt | `latest`, `salt` objects | one `meta` file: JSON `{"latest_version": <simple uuid>, "salt": <base64>}` |
| versions | `v-<parent>-<child>` | `v-{parent.simple()}-{child.simple()}`, sealed with the child id (the same AAD as the bucket) |
| snapshot | `s-<version>`, possibly several | one `snapshot` file: JSON `{"version_id": <simple uuid>, "payload": <base64 sealed>}`, overwritten |
| atomicity | compare-and-swap on `latest` | one commit holding the version file and `meta`, then a push; a rejected push gives `ExpectedParentVersion` |
| snapshot urgency | random (about 1% high, 9% low) | versions since the snapshot: up to 50 none, up to 100 low, more high |
| cleanup | `with_cleanup`, by object date | after `add_snapshot`: `git rm` the versions the snapshot covers whose last commit is over 180 days old, with a cycle guard |

Also copied:
- When `meta.latest_version` is nil, any parent is accepted.
- The first sync to an empty branch writes `meta` with a new salt, in a commit named "init taskchampion repo".
- The key is `Cryptor::new(salt, secret)`, as for the bucket.

To check, run the real `task` 3.5.0 on a scratch `TASKRC` and data directory with `sync.git.remote` pointed at a
scratch repository, in both directions. The output goes here once it has been run.

## Design

- **`tc_core::gitsync`** (pure, no I/O): `GitServer<R: GitRepo>`, which implements `taskchampion::server::Server` by
  porting `gitsync`'s rules. `GitRepo` is a small async trait:
  - `head() -> sha`
  - `read(path, at) -> Option<bytes>`
  - `list(prefix, at) -> names`
  - `commit(parent_sha, changes: [(path, Some(bytes) | None)], message) -> Committed(sha) | Moved`
  - `last_changed(path) -> secs`

  It reuses `tc_core::crypto::Cryptor`.
- **`tc_core::forge`** (pure): builds the requests (method, path, JSON) and parses the replies, so the hosts only carry
  HTTP. Every write is **one multi-file commit in one request** (decided 2026-11-17), with two adapters:
  - **GitHub:** GraphQL `createCommitOnBranch` (`fileChanges` with additions and deletions, `expectedHeadOid` = the head
    the server read). A head that has moved comes back as `Moved`, the same check as a rejected push.
  - **Gitea and Forgejo:** `POST /repos/{owner}/{repo}/contents` with `files: [{operation, path, content, sha}]`. The
    check is the `sha` of `meta`, which every `add_version` changes. A sha mismatch comes back as `Moved`.
  - When the head moved but `meta.latest_version` is still the parent (someone else committed only under `web/`), the
    server retries the commit rather than reporting `ExpectedParentVersion`.
  - Reads use `GET contents` and `git/trees` (the same on both).
- **Cleanup** (decided 2026-11-17: as `gitsync` does, made cheaper by the multi-file API). It runs only after
  `add_snapshot`, so it is rare. All the covered versions old enough to go are deleted in **one** commit, not one each.
  What's left is finding each file's age (the time of the last commit that touched it):
  - GitHub: one GraphQL query with an aliased `history(path:, first: 1)` per covered file.
  - Gitea and Forgejo: `GET commits?path=<file>&limit=1` per file, oldest first, capped per cleanup (say 20). The rest
    wait for the next snapshot, which is safe because cleanup is best-effort in `gitsync` too.
- **Repository growth** follows `task` (decided 2026-11-17): deleted versions and overwritten snapshots stay in git
  history, as they do when `task` syncs to git.
- **Settings files** (`web/config.json`, `web/settings.json`, `web/hooks.lua`, `web/hook-secrets.enc`, `web/rc/*`) live
  under `web/` on the same branch. They go through an `ObjectStore` over `GitRepo` (`GitObjects`), where each put or
  delete is one commit. `WebSettings` and `tc_core::webapi` stay as they are. To verify: `task`'s
  `git clean -f -- v-* snapshot meta` and `reset --hard` leave `web/` alone.
- **Worker** (`crates/worker`):
  - Cargo features `r2` (the default) and `git`. `store.rs` stays for r2.
  - A new `gitrepo.rs` implements `GitRepo` over `worker::Fetch`, and `lib.rs`/`session.rs` opens `GitServer` or
    `CloudServer` depending on the feature.
  - Two wrangler configs, `wrangler.jsonc` and `wrangler.git.jsonc`. `deploy-lib.mjs` gets the keys `GIT_REPO`,
    `GIT_BRANCH` and `GIT_API`, and `setup.mjs` asks which flavour.
  - `package-release.mjs` assembles both builds, with a test.
  - `/api/health` reads `meta` with the token.
  - Each flavour stays under 1 MB gzipped. Git should come in at or below r2, since JSON and base64 are already
    compiled in.
- **Android** (`crates/android`, `app/`):
  - `TasksAt::Git { api, repo, branch }` in `engine.rs`.
  - `gitrepo.rs`, blocking `ureq` like `s3.rs`, with the token from `secrets.rs`.
  - `filesync.rs` gets a git target.
  - The switch and merge logic (`Switch`, `backend-changed`) covers git as it does the others.
  - Kotlin: a git variant of `Connection`/`SyncSettings`, with fields on the connect screen and the sync panel.
- **Security:**
  - Only tasks and hook secrets are encrypted. The settings and hooks are plain text in the repository, so the docs
    and setup require a **private** repository.
  - The token is a Worker secret or sits in the Keystore, and never reaches a page.
  - The importer, `show` and `config` already refuse `sync.*` names; add a test for `sync.git.*`.

## Acceptance criteria

- [x] `tc_core::gitsync` passes TaskChampion's own `gitsync` test cases, ported against an in-memory `GitRepo`.
- [x] Interop, on GitHub (a test repository) **and** on Gitea or Forgejo (in Docker): tasks added, modified and
      completed reach every side, both ways, between the real `task` 3.5.0, the engine on the git Worker and the
      Android library. A snapshot is written and read back. A concurrent push gives `ExpectedParentVersion`, and a
      retry succeeds.
- [x] Web: the git Worker deploys from a release package (no `crates/`), setup asks for the flavour, and the page
      works unchanged.
- [x] Android: a git connection syncs tasks and the settings files, and the token is stored only in the Keystore.
- [x] Secrets: tests show that `GIT_TOKEN` and `sync.git.*` never reach a page or the output of `show`.
- [x] Tests: tc-core unit tests, Worker `wasm32` check for both features, `scripts/*.test.mjs` for deploy and package,
      Kotlin tests for the git connection.
- [x] Docs: README and `docs/architecture.md` (the Mermaid diagram the same in both), `docs/android.md`, the sync guide
      in `docs/using/`, `docs/taskrc-support.md` (`sync.git.*`), and the deploy and setup docs.
- [x] Size: engine unchanged; both Worker flavours measured before and after, each under 1 MB gzipped.

## Open questions

None left. Settled 2026-11-17:

- Gitea and Forgejo use the multi-file contents API.
- Cleanup follows `gitsync`, with one deletion commit.
- Repository growth follows `task`.
- Moving an R2 bucket into git is not done (see Not doing).
- GitHub's rate limit (5000 requests, or GraphQL points, an hour per token) is enough for about 2 to 4 requests a sync
  and 1 for each push.
- Commit author: `taskchampion <taskchampion@local>`, as `gitsync` sets it, so every replica's commits look alike. On
  GitHub, `createCommitOnBranch` always commits as the token's owner, so this applies to Gitea and Forgejo only.

## Implementation notes

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.

### 2026-11-17

- Parity, checked on a scratch `TASKRC` with `sync.git.remote` set to a scratch bare repository: `task` 3.5.0 writes
  `meta` (`{"latest_version":"<simple>","salt":"<base64>"}`), `v-<simple>-<simple>` and, after 101 versions, a
  `snapshot` JSON file; it commits as `taskchampion <taskchampion@local>` ("init taskchampion repo", "add version",
  "add snapshot"). A `web/settings.json` committed to the branch by someone else is left alone by its sync.
- Built `tc_core::gitsync` (`GitRepo` trait, `GitServer`, `GitObjects`, `load_salt`/`load_cryptor`, `MemRepo`) and
  `tc_core::forge` (`Http` trait, `GitRemote`, `ForgeRepo` for GitHub and Gitea/Forgejo). `base64` added to tc-core (the
  Worker already carries it).
- Departure: `add_snapshot` does not fail the sync when the snapshot cannot be stored (the CLI returns an error), for the
  reason `crate::cloud` gives: the version is already on the branch, and failing would make the replica send it again.
- Departure: GitHub's `createCommitOnBranch` checks the head, so a commit refused because only a settings file moved is
  tried again (up to 4 times) after reading `meta` again.
- Forgejo 11 (Gitea 1.22 API): `POST /contents` answers 422 for a stale `sha` or an existing file, and a refused commit
  leaves nothing; `git/trees/<branch name>` does not find every branch (`tc-test-<hex>` is "sha not found"), so the root
  is listed by the head's commit id from `GET /branches/<name>`.
- `tests/gitsync.rs` (tc-core): TaskChampion's gitsync cases on `MemRepo`, and a branch the real CLI wrote
  (`tests/fixtures/gitsync-cli`, secret `s3cret`) read back into a replica with all 108 tasks.
- Android: `TasksAt::Git { api, repo, branch, token }`, `gitrepo.rs` (`Http` over ureq); shared settings files go on the
  same branch when there is no `files_bucket`. `tests/gitsync.rs` (ignored; needs a forge) passes on a scratch Forgejo
  in Docker: two phones, settings files, and the real `task` 3.5.0 both ways, including a snapshot the CLI wrote that a
  new phone starts from.
- Left: the Worker flavour, deploy and release scripts, the Kotlin screens, docs, sizes, and GitHub (needs a test
  repository and a token).
- Worker: one crate, one cargo feature `git` (departure from "features `r2` and `git`": R2 is simply the build without
  it). `backend.rs` is the only place that differs (store, salt, server, the settings `/api/setup` names);
  `gitrepo.rs` is `Http` over the global `fetch`. Departure from "two wrangler configs": `deploy-lib.mjs` overlays
  `BACKEND=git` onto the one `wrangler.jsonc` (drops `r2_buckets`, points `main` at `build-git`/`worker-git`, adds
  `--flavour git` to the build), so the deploy, the release package and Actions share one path. `scripts/build.mjs
  --flavour r2|git|all`; the package carries `worker/` and `worker-git/`; setup asks for the backend; CI clippies the
  `git` feature; the Release workflow builds both.
- Kotlin: `GitAddress` in `Connection` (the token sealed by the Keystore with the rest), `typedGit` (GitHub's API and
  `taskchampion` by default), a **Git** choice in `ConnectForm`, `GitEditForm` in the Sync tab, `Store.changeGit`; the
  shared files go on the branch without a bucket. 3 new tests in `SyncTest`; all 426 Kotlin tests pass and the debug APK
  builds. Not yet tried on an emulator.
- Secrets: `sync.git.*` (and a URL with a token in it) added to the importer's and `config`'s refusal tests;
  `tests/forge.rs` (11 tests, a recording `Http`) shows the token only ever in the `authorization` header, never in a
  URL, a body or an error.
- Fix: `GitObjects::list_dated` gave every file the date 0, but the phone's `filesync.rs` keeps the newer of two copies
  changed on both sides by that date, so the phone's copy always won. It now dates each file by its last commit
  (`last_changed`, batched); `list` stays the cheap listing. Tests: `a_settings_file_is_dated_by_its_last_commit`
  (tc-core), `with_a_git_branch_two_changes_go_to_the_newer_commit` (android, fails without the fix), and the Forgejo
  test below.
- Every shared file checked on a real branch: through the packaged git Worker, the taskrc folder (with an `include`),
  `config.json` and `config.prev.json`, `rc.prev.json`, `settings.json`, `hooks.lua` and `hooks.prev.lua` and
  `hook-secrets.enc` (sealed: neither the value nor the name is in it) all land under `web/`, and a restore works. The
  phone's Forgejo test now sends all of them to a second phone and settles a both-changed `settings.json` by the later
  commit. Each file is a commit of its own, so a `config` change makes four commits (four requests); fine for now.
- Release package: assembled with `package-release.mjs`; with `BACKEND=git` its deploy config has no bucket and runs
  `worker-git/`. Run locally (`wrangler dev`, the scratch Forgejo): `/api/setup` configured, `/api/health` ok, the page
  served, and a task added through it read back through the other git Worker on the branch.
- Sizes (gzip -9, debug names dropped): R2 Worker 697,163 → 698,457; git Worker 715,531 (about 17 KB over R2, not "at
  or below" as the design guessed: the GraphQL and forge requests); engine 861,823 → 861,767 (unchanged).
- Left: GitHub interop (a test repository and a token from the person), and the Kotlin screens on an emulator.
- Sizes after 0012 (which adds to both Workers): R2 707,281, git 724,365.

2026-11-20: done; the last criteria checked by uroybd: git interop runs smoothly in daily use, GitHub included, more so after the sync optimizations of 0014.
