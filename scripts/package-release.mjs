#!/usr/bin/env node
// Assembles a prebuilt release: everything a deploy needs and nothing to build. It is what the Release workflow
// pushes to the `release` branch (the target of the Deploy to Cloudflare button) and attaches to a GitHub release.
//   node scripts/package-release.mjs [--out release] [--version v1.2.3]
// It packages what `node scripts/build.mjs` has already built (crates/worker/build and web/dist).
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { ROOT, stripJsonc } from './deploy-lib.mjs';

/** The Worker's config for a package: the source config, with the build step gone and the paths where the files land. */
export function releaseConfig(base) {
  const c = structuredClone(base);
  delete c.build;
  c.main = 'worker/index.js';
  c.assets = { ...c.assets, directory: 'app' };
  return c;
}

/** The package.json of a release: the scripts that deploy it, what the Deploy button shows, and wrangler. */
export function releasePackage(source, version) {
  return {
    name: source.name,
    version,
    private: true,
    license: source.license,
    type: 'module',
    scripts: { setup: 'node scripts/setup.mjs', deploy: 'node scripts/deploy.mjs' },
    devDependencies: { wrangler: source.devDependencies.wrangler },
    cloudflare: source.cloudflare,
  };
}

const README = (version) => `# Meghnaad ${version}: prebuilt

This is the built app, ready to deploy: no Rust, no build. It is generated from
[the source](https://github.com/uroybd/Meghnaad) by the Release workflow; don't edit it here.

- **Deploy to Cloudflare**:
  [![Deploy to Cloudflare](https://deploy.workers.cloudflare.com/button)](https://deploy.workers.cloudflare.com/?url=https://github.com/uroybd/Meghnaad/tree/release)
- **From a terminal** (Node.js 20+): \`npm install && npm run setup\`, then \`npm run deploy\` to update.

Either way the app shows a setup page until Cloudflare Access is connected; nothing is reachable before that. The steps,
and pointing your \`task\` CLI at the bucket, are in
[Deploying](https://github.com/uroybd/Meghnaad/blob/main/docs/deploy.md).

| | |
| --- | --- |
| \`worker/\` | the Worker (WebAssembly) |
| \`app/\` | the web app, including the engine that runs in your browser |
| \`wrangler.jsonc\` | its configuration (the bucket \`TASKS\`; \`keep_vars\` keeps what you add in the dashboard) |
| \`scripts/\` | what \`npm run setup\` and \`npm run deploy\` run |
`;

/** Writes the package into \`out\`, which is emptied first. \`root\` is a source tree that has been built. */
export function assemble(root, out, version) {
  const worker = join(root, 'crates', 'worker', 'build');
  const dist = join(root, 'web', 'dist');
  for (const need of [join(worker, 'index.js'), join(worker, 'index_bg.wasm'), join(dist, 'index.html')])
    if (!existsSync(need)) throw new Error(`${need} is missing: build first (node scripts/build.mjs)`);

  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  cpSync(worker, join(out, 'worker'), { recursive: true, filter: (p) => !p.endsWith('.gitignore') });
  cpSync(dist, join(out, 'app'), { recursive: true });
  cpSync(join(root, 'LICENSE'), join(out, 'LICENSE'));

  mkdirSync(join(out, 'scripts'));
  for (const f of ['setup.mjs', 'deploy.mjs', 'deploy-lib.mjs'])
    cpSync(join(root, 'scripts', f), join(out, 'scripts', f));

  const base = JSON.parse(stripJsonc(readFileSync(join(root, 'wrangler.jsonc'), 'utf8')));
  writeFileSync(join(out, 'wrangler.jsonc'), JSON.stringify(releaseConfig(base), null, 2) + '\n');
  const source = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
  writeFileSync(join(out, 'package.json'), JSON.stringify(releasePackage(source, version), null, 2) + '\n');
  // The Deploy to Cloudflare button asks for the secrets named here.
  writeFileSync(join(out, '.dev.vars.example'), 'TC_ENCRYPTION_SECRET=\n');
  writeFileSync(join(out, 'README.md'), README(version));
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const arg = (name, fallback) => {
    const i = process.argv.indexOf(name);
    return i > 0 ? process.argv[i + 1] : fallback;
  };
  const out = resolve(arg('--out', join(ROOT, 'release')));
  const version = arg('--version', '0.0.0-dev').replace(/^v/, '');
  assemble(ROOT, out, version);
  console.log(`[package] ${version} -> ${out}`);
}
