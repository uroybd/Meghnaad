# Where your tasks live

Every app here is another replica of your Taskwarrior data, so they all need one place to meet: the store `task sync`
already uses, or a new one. You choose it once, and every app uses the same place with the same encryption secret
(`sync.encryption_secret`). There are three kinds:

- **A git repository**: a branch of a repository on GitHub, Gitea or Forgejo, in TaskChampion's git layout (`task`
  3.5.0's `sync.git.*`).
- **An S3 or R2 bucket**: TaskChampion's cloud layout in a bucket on Cloudflare R2, AWS S3, MinIO or another
  S3-compatible service (`sync.aws.*`).
- **A TaskChampion sync server**: `taskchampion-sync-server`, or anything speaking its protocol (`sync.server.*`).

Not every app reaches every store:

| | Git repository | S3 / R2 bucket | Sync server |
| --- | --- | --- | --- |
| Web app, static site | yes | no | no |
| Web app, Cloudflare Worker | yes | R2 only | no |
| Android app | yes | yes | yes |
| Terminal app (`task`'s own sync) | yes | yes | yes |

The static site talks to its store from the browser, through a forge's API; a sync server sends no CORS headers, so a
page can't reach one. The Worker reaches R2 through a binding, not over S3. The terminal app
runs the real `task`, so it syncs wherever `task` does.

In every store the tasks are encrypted before they leave a replica, with a key derived from the encryption secret; the
store never sees them in plain form. The web app also keeps **its own files** in the bucket or on the git branch, under
`web/`: the taskrc files, `config.json`, `settings.json` (task commands, boards, keys), the hooks script, and the hook
secrets, sealed. Only the hook secrets are encrypted; the rest is plain text. The Android app can share the same files
(never the hook secrets). A sync server holds tasks only.

## A git repository

Make a **private** repository for the tasks; on GitHub give it one commit (a README will do) so the branch can be made
from it. Gitea and Forgejo make the branch from nothing. The branch (`taskchampion` unless you choose another) is made
on the first sync if it isn't there.

Point `task` at it with a local clone it keeps for itself and a remote it can push to without a prompt (an SSH key or a
credential helper):

```bash
task config sync.git.local_path  ~/.task-sync                     # a folder of its own; made if missing
task config sync.git.branch      taskchampion
task config sync.git.remote      git@github.com:alice/tasks.git   # or https://, with your git credentials
task config sync.encryption_secret '<your encryption secret>'
task sync
```

The web app and the phone have no `git`: they reach the same branch through the forge's API with a token that may read
and write the repository's contents. On GitHub that is a fine-grained personal access token for that one repository with
**Contents: Read and write** and nothing else; on Gitea and Forgejo an access token with the **repository** scope, read
and write. The API address is `https://api.github.com` for GitHub, `https://<host>/api/v1` for Gitea or Forgejo and
`https://<host>/api/v3` for GitHub Enterprise.

- **Keep the repository private.** The tasks and hook secrets are encrypted, but the app's settings, taskrc files and
  hooks script sit on the same branch as plain text. `task` leaves them alone.
- **History grows with every sync**, as it does with `task`: removed versions stay in older commits.
- **GitHub allows 5,000 API requests an hour per token.** An idle sync is one request and a change three or four, so even
  the web app's sync every 30 seconds while a page is visible stays far below it.

Setting it up in each app: [the static site](web-static.md), [the git
Worker](web-cloudflare.md#a-git-repository-instead-of-r2), [the Android app](android.md#a-git-repository).

## An S3 or R2 bucket

Any S3-compatible bucket that accepts conditional writes (`If-Match` and `If-None-Match` on upload) works: R2, MinIO and
current AWS S3 do. The web app reaches **R2 only**, through the Worker's binding; the Android app and `task` reach any of
them over S3 with an access key.

For R2, create an **R2 API token** (R2 → *Manage R2 API Tokens* → **Object Read & Write**, scoped to your bucket) and
note your account ID (in the dashboard's right-hand column). Then point `task` at the bucket:

```bash
task config sync.encryption_secret '<your encryption secret>'
task config sync.aws.region        auto
task config sync.aws.bucket        <your bucket name>
task config sync.aws.access_key_id     <R2 access key id>
task config sync.aws.secret_access_key <R2 secret access key>
task config sync.aws.endpoint_url      https://<ACCOUNT_ID>.r2.cloudflarestorage.com
task sync
```

`sync.aws.endpoint_url` (Taskwarrior 3.6.0) is what points `task` at R2 or MinIO instead of AWS; with the setting
missing, `task` talks to real AWS and fails with a `PermanentRedirect` error. A store that wants the bucket in the path
rather than the host name (MinIO on its own domain, often) also needs `task config sync.aws.force_path_style true`. For
AWS S3 itself, leave both unset and use the bucket's real region; `man task-sync` has the IAM policy it needs.

Taskwarrior 3.5 and older have no such setting. There the AWS SDK's `AWS_ENDPOINT_URL` environment variable does the
same, and must be set **everywhere `task sync` runs** (your shell, cron, hooks, GUI launchers).

- **Keep the bucket private** and the key scoped to it. The tasks and hook secrets are encrypted; the app's settings,
  taskrc files and hooks script are plain text under `web/`.
- **No lifecycle rules** on the bucket: they could delete or move the versions TaskChampion keeps.

Setting it up in each app: [the R2 Worker](deploy.md), [the Android app](android.md#first-run).

## A TaskChampion sync server

A sync server holds the tasks of every client ID it is given, encrypted; it never sees them in plain form. Invent a
client ID (a UUID, from `uuidgen`) once and use it on every replica that shares the tasks. Point `task` at it:

```bash
task config sync.server.url        https://tasks.example.com
task config sync.server.client_id  <your client id>
task config sync.encryption_secret '<your encryption secret>'
task sync
```

Only the Android app and `task` can use one. A sync server holds no files besides the tasks, so the phone's settings and
hooks stay on the phone unless you give it a bucket for them as well
([the Android app](android.md#a-sync-server)).

- **Use `https://`** for anything but a server on your own machine; the phone accepts `http://` only for `localhost`.

## What must stay private

| | Where it is kept |
| --- | --- |
| **The encryption secret** | Your taskrc; a Worker secret; the phone's Keystore. The static site doesn't store it: you type it to unlock, and it stays in memory while unlocked |
| **A repository token** | A Worker secret; the phone's Keystore; on the static site, in the browser, sealed with the secret |
| **A bucket's access key** | Your taskrc; the phone's Keystore. The Worker needs none: it uses an R2 binding |
| **A sync server's client ID** | Your taskrc; the phone's Keystore |

None of them is ever stored in the bucket or the repository, and the taskrc importer, `show` and `config` refuse sync and
credential settings by name ([taskrc support](taskrc-support.md)).

