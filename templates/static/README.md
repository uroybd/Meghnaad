# Meghnaad, as a static site

This repository deploys [Meghnaad](https://github.com/uroybd/Meghnaad)'s web app with no server of its own: a page that
reaches your tasks on a branch of a git repository directly, through GitHub's (or Gitea's, or Forgejo's) API, and is
unlocked in your browser with your encryption secret. It holds no code: `fetch-release.mjs` downloads a release of the
built site, checks it against its published SHA-256, and puts it in `site/`.

Your tasks are the branch your `task` CLI syncs to with `sync.git.*`, encrypted as `task` encrypts them. The app's
settings and hooks script are kept beside them under `web/`, in plain text, so **the tasks repository must be private**.

## Choose a setup

**A. GitHub Pages, two repositories.** Pages for a private repository needs a paid plan, so the site and the tasks are
apart:

1. Make a repository from this one (it can be public: it holds no tasks and no secrets).
2. Settings → Pages → Source: **GitHub Actions**. The workflow in `.github/workflows/pages.yml` runs on every push to
   `main`, by hand, and weekly.
3. Your tasks stay in their own **private** repository.

The site is then at `https://<you>.github.io/<this-repository>/`.

**B. Cloudflare Pages or Netlify, one private repository.** These serve a private repository for free, so one repository
can hold both: the site on `main`, the tasks on the `taskchampion` branch (what `task` uses with
`sync.git.branch=taskchampion`).

1. Make a **private** repository from this one.
2. In Cloudflare Pages (or Netlify), connect it: build command `node fetch-release.mjs`, output directory `site`.
3. Point `task` and the app at the same repository, branch `taskchampion` (`task`'s settings are in
   [Where your tasks live](https://github.com/uroybd/Meghnaad/blob/main/docs/stores.md#a-git-repository)).

## Connect a device

Open the site. The first time on a device it asks for:

- **Repository** (`owner/name`), **Branch** (`taskchampion`, or what your taskrc's `sync.git.branch` says) and **API
  address** (empty for GitHub; `https://<host>/api/v1` for Gitea or Forgejo);
- **Token**: on GitHub a fine-grained token for that one repository with **Contents: Read and write**; on Gitea and
  Forgejo an access token with the repository scope;
- **Encryption secret**: your `sync.encryption_secret`.

The token is kept on the device, sealed with the encryption secret (in OpenPGP's format: `gpg --decrypt` opens it too).
Every later visit asks only for the secret. A password manager can keep the token and the secret as one login (the
token as the username, the secret as the password), so connecting another device is one fill. **Lock** (in the header) and an idle time lock every tab; **Settings →
General → Forget this device** removes the token and the device's copy of the tasks.

Set `config.json` to fill in the form for everyone who opens this site:

```json
{ "repo": "you/tasks", "branch": "taskchampion", "api": "" }
```

**Gitea and Forgejo** must let the page call their API: in `app.ini`,

```ini
[cors]
ENABLED = true
ALLOW_DOMAIN = https://<this site's host>
ALLOW_HEADERS = Content-Type,Authorization
```

GitHub's API allows it already.

## Updating

`meghnaad-version` says which release to take: `latest` (the default; the weekly run picks up new releases) or a
version such as `0.8.0`. Change it and push, or run the workflow by hand.

## What it trusts

No server sees your tasks: the forge stores them encrypted, and this page decrypts them in your browser. What an
unlocked page runs can read them, as with any web app: the site's own code (from the release it downloaded and
checked), and your browser and its extensions. See
[The web app](https://github.com/uroybd/Meghnaad/blob/main/docs/deploy.md) for how this compares with the
Worker.
