#!/usr/bin/env node
// Builds the engine for the browser: crates/browser (tc-core as WebAssembly) into web/src/engine/pkg, where the web
// app imports it from. `node scripts/build-engine.mjs` always builds; `--if-stale` only when the Rust sources are
// newer than what is there (what `npm run dev` and `npm run build:web` use). `--node` builds the same engine for Node
// (into target/engine-node), which scripts/engine-cli.mjs uses to run commands against a Worker from a shell.
//
// wasm-bindgen's command-line tool has to be the same version as the `wasm-bindgen` crate in Cargo.lock. It is looked
// for on the PATH and in worker-build's cache (worker-build downloads one for the Worker), and installed with cargo if
// there is none.
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { capture, ROOT, run } from './deploy-lib.mjs';
import { dropCustomSections } from './wasm-strip.mjs';

const BUILT = join(ROOT, 'target', 'wasm32-unknown-unknown', 'release', 'tc_browser.wasm');
/** Where each kind of build goes, and how wasm-bindgen is asked to make it. */
const KINDS = {
  web: { dir: join(ROOT, 'web', 'src', 'engine', 'pkg'), target: 'web' },
  node: { dir: join(ROOT, 'target', 'engine-node'), target: 'nodejs' },
};
export const OUT = KINDS.web.dir;
export const NODE_OUT = KINDS.node.dir;
const wasmOf = (kind) => join(KINDS[kind].dir, 'tc_browser_bg.wasm');

/** The version of the `wasm-bindgen` crate in Cargo.lock. */
export function bindgenVersion(lock = readFileSync(join(ROOT, 'Cargo.lock'), 'utf8')) {
  const m = /name = "wasm-bindgen"\nversion = "([^"]+)"/.exec(lock);
  if (!m) throw new Error('wasm-bindgen is not in Cargo.lock');
  return m[1];
}

function candidates(version) {
  const found = [];
  const cache =
    process.platform === 'darwin'
      ? join(homedir(), 'Library', 'Caches')
      : (process.env.XDG_CACHE_HOME ?? join(homedir(), '.cache'));
  const dir = join(cache, 'worker-build');
  if (existsSync(dir)) {
    for (const name of readdirSync(dir)) {
      if (name.startsWith('wasm-bindgen-') && name.endsWith(`-${version}`)) found.push(join(dir, name, 'wasm-bindgen'));
    }
  }
  return ['wasm-bindgen', ...found];
}

/** A `wasm-bindgen` of the right version, installing one if need be. */
export function findBindgen(version = bindgenVersion()) {
  for (const c of candidates(version)) {
    const r = capture(c, ['--version']);
    if (r.ok && r.out.includes(` ${version}`)) return c;
  }
  console.log(`[engine] installing wasm-bindgen-cli ${version} (once)`);
  const r = run('cargo', ['install', 'wasm-bindgen-cli', '--version', version, '--locked']);
  if (r.status !== 0) throw new Error('installing wasm-bindgen-cli failed');
  return 'wasm-bindgen';
}

function newest(dir) {
  let latest = 0;
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, entry.name);
    if (entry.isDirectory()) latest = Math.max(latest, newest(p));
    else if (/\.(rs|toml)$/.test(entry.name)) latest = Math.max(latest, statSync(p).mtimeMs);
  }
  return latest;
}

/** Is the built package older than the Rust it comes from? */
export function stale(kind = 'web') {
  if (!existsSync(wasmOf(kind))) return true;
  const built = statSync(wasmOf(kind)).mtimeMs;
  const sources = [join(ROOT, 'crates', 'tc-core'), join(ROOT, 'crates', 'browser')].map(newest);
  return Math.max(...sources, statSync(join(ROOT, 'Cargo.lock')).mtimeMs) > built;
}

export function buildEngine(kind = 'web') {
  const { dir, target } = KINDS[kind];
  const wasm = wasmOf(kind);
  const version = bindgenVersion();
  console.log('[engine] compiling crates/browser to WebAssembly');
  let r = run('cargo', ['build', '--release', '--target', 'wasm32-unknown-unknown', '-p', 'tc-browser']);
  if (r.status !== 0) throw new Error('cargo build failed');
  const bindgen = findBindgen(version);
  mkdirSync(dir, { recursive: true });
  r = run(bindgen, ['--target', target, '--no-typescript', '--out-dir', dir, BUILT]);
  if (r.status !== 0) throw new Error('wasm-bindgen failed');
  // The repository's package.json says "module"; what wasm-bindgen writes for Node is CommonJS.
  if (kind === 'node') writeFileSync(join(dir, 'package.json'), '{ "type": "commonjs" }\n');
  // As for the Worker: the names only help read a stack trace.
  if (!process.env.KEEP_WASM_NAMES) {
    const before = readFileSync(wasm);
    const after = dropCustomSections(before, ['name', 'producers', 'target_features']);
    writeFileSync(wasm, after);
    console.log(
      `[engine] dropped the debug names: ${(before.length / 1024).toFixed(0)} KB -> ${(after.length / 1024).toFixed(0)} KB`,
    );
  }
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const kind = process.argv.includes('--node') ? 'node' : 'web';
    if (process.argv.includes('--if-stale') && !stale(kind)) console.log('[engine] up to date');
    else buildEngine(kind);
  } catch (e) {
    console.error(`[engine] ${e.message}`);
    process.exit(1);
  }
}
