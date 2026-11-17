#!/usr/bin/env node
// Builds everything a deploy needs: the web app (web/dist) and the Worker (crates/worker/build, or build-git for the
// git flavour: `--flavour git`; `--flavour all` builds both).
// It is wrangler's `build.command`, so `wrangler dev`, `wrangler deploy`, `npm run deploy` and
// Cloudflare's own build service all go through it. Missing Rust pieces are installed, which keeps
// a fresh build environment working without any extra setup.
import { has, missingTools, run, ROOT } from './deploy-lib.mjs';
import { join } from 'node:path';
import { readFileSync, writeFileSync } from 'node:fs';
import { dropCustomSections } from './wasm-strip.mjs';

const step = (m) => console.log(`\n[build] ${m}`);
const must = (r, what) => {
  if (r.status !== 0) {
    console.error(`[build] ${what} failed`);
    process.exit(r.status ?? 1);
  }
};

for (const m of missingTools()) {
  if (!m.fix) {
    console.error(`[build] ${m.what} is missing: ${m.hint}`);
    process.exit(1);
  }
  step(`installing ${m.what} (${m.fix[0]} ${m.fix[1].join(' ')})`);
  must(run(...m.fix), `installing ${m.what}`);
}

// `npm run dev` builds and serves the app and the engine itself (scripts/dev.mjs); it only needs the Worker from here.
if (!process.env.MEGHNAAD_DEV) {
  step('engine (Rust -> WASM, for the browser)');
  must(run('node', ['scripts/build-engine.mjs', '--if-stale']), 'the engine build');

  step('web app');
  must(run('npm', ['--prefix', 'web', 'run', 'build']), 'the web build');
}

// Which Worker: `r2` (the default; tasks in the bucket TASKS), `git` (tasks on a branch of a git repository, built with
// the `git` feature into crates/worker/build-git), or `all` (both, for a release).
const flavourArg = process.argv.indexOf('--flavour');
const flavour = flavourArg > 0 ? process.argv[flavourArg + 1] : 'r2';
if (!['r2', 'git', 'all'].includes(flavour)) {
  console.error(`[build] --flavour is r2, git or all, not ${flavour}`);
  process.exit(1);
}
if (!has('worker-build')) {
  console.error('[build] worker-build is not on PATH (try: cargo install worker-build --version "~0.8" --locked)');
  process.exit(1);
}
const flavours = flavour === 'all' ? ['r2', 'git'] : [flavour];
for (const f of flavours) {
  step(`Worker, ${f} (Rust -> WASM)`);
  const args = f === 'git' ? ['--release', '--out-dir', 'build-git', '.', '--features', 'git'] : ['--release'];
  must(run('worker-build', args, { cwd: join(ROOT, 'crates', 'worker') }), 'worker-build');

  // The function names (a fifth of the module) only help read a stack trace; leave them out of what is
  // uploaded. KEEP_WASM_NAMES=1 keeps them for debugging.
  if (!process.env.KEEP_WASM_NAMES) {
    const wasm = join(ROOT, 'crates', 'worker', f === 'git' ? 'build-git' : 'build', 'index_bg.wasm');
    const before = readFileSync(wasm);
    const after = dropCustomSections(before, ['name', 'producers', 'target_features']);
    writeFileSync(wasm, after);
    step(`dropped the debug names: ${(before.length / 1024).toFixed(0)} KB -> ${(after.length / 1024).toFixed(0)} KB`);
  }
}
