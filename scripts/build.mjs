#!/usr/bin/env node
// Builds everything a deploy needs: the web app (web/dist) and the Worker (crates/worker/build).
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

step('web app');
must(run('npm', ['--prefix', 'web', 'run', 'build']), 'the web build');

step('Worker (Rust -> WASM)');
if (!has('worker-build')) {
  console.error('[build] worker-build is not on PATH (try: cargo install worker-build)');
  process.exit(1);
}
must(run('worker-build', ['--release'], { cwd: join(ROOT, 'crates', 'worker') }), 'worker-build');

// The function names (a fifth of the module) only help read a stack trace; leave them out of what is
// uploaded. KEEP_WASM_NAMES=1 keeps them for debugging.
if (!process.env.KEEP_WASM_NAMES) {
  const wasm = join(ROOT, 'crates', 'worker', 'build', 'index_bg.wasm');
  const before = readFileSync(wasm);
  const after = dropCustomSections(before, ['name', 'producers', 'target_features']);
  writeFileSync(wasm, after);
  step(`dropped the debug names: ${(before.length / 1024).toFixed(0)} KB -> ${(after.length / 1024).toFixed(0)} KB`);
}
