# The web app

The web app is the same page however you host it: the engine runs in it, as WebAssembly in a Web Worker, with your
tasks saved in the browser. What differs is where your tasks live and who holds the encryption secret. There are three
setups, and none is the default:

| | Static site | Cloudflare Worker with R2 | Cloudflare Worker with git |
| --- | --- | --- | --- |
| **Your tasks** | a git repository (GitHub, Gitea, Forgejo) | an R2 bucket | a git repository (GitHub, Gitea, Forgejo) |
| **You need** | any static host (GitHub Pages, Cloudflare Pages, Netlify) and a repository token | Cloudflare Workers, R2 and Access | Cloudflare Workers and Access, and a repository token |
| **Sign-in** | the encryption secret unlocks the page | Cloudflare Access | Cloudflare Access |
| **The secret and token are held by** | your browser: sealed at rest, in memory while unlocked | the Worker, as Cloudflare secrets | the Worker, as Cloudflare secrets |
| **You trust** | the site's code, your browser and its extensions; the forge sees your settings, never your tasks | Cloudflare, and whoever can read your account's Worker secrets | the same; the forge sees your settings, never your tasks |
| **Hooks run** | in the page | on the Worker | on the Worker |
| **Costs** | a free static host | Cloudflare's free plans | Cloudflare's free plans |

- **[The static web app](web-static.md)**: no server of your own. The page reaches a branch of a git repository through
  the forge's API and unlocks with your encryption secret. Use the public copy at [mn.utsob.me](https://mn.utsob.me), or
  host the release on GitHub Pages, Cloudflare Pages or Netlify.
- **[The web app on Cloudflare](web-cloudflare.md)**: a small Worker serves the page and sits between it and your store.
  It checks Cloudflare Access (the login), holds the encryption secret, and opens and seals the versions that pass
  through; it never parses a task. It comes in two builds, for an R2 bucket or for a git repository.

Each setup syncs with the same store your `task` CLI uses; pointing `task` at it is in
[Where your tasks live](stores.md). The [Android app](android.md) needs none of them: it syncs with the store directly.
The app adapts to a phone's screen as well ([On a phone](using/phone.md)).
