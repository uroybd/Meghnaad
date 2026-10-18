# Meghnaad 0.6.10: prebuilt

This is the built app, ready to deploy: no Rust, no build. It is generated from
[the source](https://github.com/uroybd/Meghnaad) by the Release workflow; don't edit it here.

- **Deploy to Cloudflare**:
  [![Deploy to Cloudflare](https://deploy.workers.cloudflare.com/button)](https://deploy.workers.cloudflare.com/?url=https://github.com/uroybd/Meghnaad/tree/release)
- **From a terminal** (Node.js 20+): `npm install && npm run setup`, then `npm run deploy` to update.

Either way the app shows a setup page until Cloudflare Access is connected; nothing is reachable before that. The steps,
and pointing your `task` CLI at the bucket, are in
[Deploying](https://github.com/uroybd/Meghnaad/blob/main/docs/deploy.md).

| | |
| --- | --- |
| `worker/` | the Worker (WebAssembly) |
| `app/` | the web app, including the engine that runs in your browser |
| `wrangler.jsonc` | its configuration (the bucket `TASKS`; `keep_vars` keeps what you add in the dashboard) |
| `scripts/` | what `npm run setup` and `npm run deploy` run |
