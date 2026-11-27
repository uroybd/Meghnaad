# The static web app

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

## Connect your `task` CLI

Skip this if your CLI already syncs to the branch. Otherwise [a git repository](stores.md#a-git-repository) has
the `sync.git.*` settings that point `task` at it.
