#!/usr/bin/env node
// The dev server: one command for everything.
//
//   npm run dev                      the app at http://127.0.0.1:5273 (hot reload), the Worker behind it on 8787
//   npm run dev -- --persist-to DIR  keep the Worker's local R2 data somewhere else (it is .wrangler/state by default)
//   npm run dev -- --api-port 8790 --web-port 5290
//
// What runs: the engine is built (crates/browser, as WebAssembly) and rebuilt when the Rust under crates/tc-core or
// crates/browser changes; `wrangler dev --local` serves the API with a simulated R2 bucket and rebuilds the Worker when
// its Rust changes; Vite serves the app with hot reload and sends /api to the Worker. Ctrl-C stops all of it.
import { spawn } from 'node:child_process';
import { existsSync, watch } from 'node:fs';
import { join } from 'node:path';
import { parseArgs } from 'node:util';
import { buildEngine, stale } from './build-engine.mjs';
import { ROOT } from './deploy-lib.mjs';

const { values } = parseArgs({
  options: {
    'api-port': { type: 'string', default: '8787' },
    'web-port': { type: 'string', default: '5273' },
    'persist-to': { type: 'string' },
    'no-watch': { type: 'boolean', default: false },
  },
});
const API = values['api-port'];
const WEB = values['web-port'];

if (!existsSync(join(ROOT, '.dev.vars'))) {
  console.error('There is no .dev.vars. Make one first:  cp .dev.vars.example .dev.vars');
  process.exit(1);
}

const say = (tag, text) => {
  for (const line of String(text).split('\n')) if (line.trim()) console.log(`[${tag}] ${line}`);
};

const children = [];
function start(tag, cmd, args, env = {}) {
  const child = spawn(cmd, args, { cwd: ROOT, env: { ...process.env, ...env }, stdio: ['ignore', 'pipe', 'pipe'] });
  child.stdout.on('data', (d) => say(tag, d));
  child.stderr.on('data', (d) => say(tag, d));
  child.on('exit', (code) => {
    if (!stopping) {
      say(tag, `stopped (${code ?? 'killed'}); stopping the rest`);
      stop(code ?? 1);
    }
  });
  children.push(child);
  return child;
}

let stopping = false;
function stop(code = 0) {
  stopping = true;
  for (const c of children) c.kill('SIGTERM');
  setTimeout(() => process.exit(code), 300);
}
process.on('SIGINT', () => stop(0));
process.on('SIGTERM', () => stop(0));

try {
  if (stale()) buildEngine();
  else say('engine', 'up to date');
} catch (e) {
  console.error(`[engine] ${e.message}`);
  process.exit(1);
}

// MEGHNAAD_DEV: the Worker's build command leaves the web app and the engine to this script.
start(
  'api',
  'npx',
  [
    'wrangler',
    'dev',
    '--local',
    '--ip',
    '127.0.0.1',
    '--port',
    API,
    ...(values['persist-to'] ? ['--persist-to', values['persist-to']] : []),
  ],
  { MEGHNAAD_DEV: '1' },
);
start('web', 'npm', ['--prefix', 'web', 'run', 'dev', '--', '--host', '127.0.0.1', '--port', WEB, '--strictPort'], {
  API_URL: `http://127.0.0.1:${API}`,
});

console.log(
  `\n  App  http://127.0.0.1:${WEB}   (hot reload; the engine runs in the page)\n  API  http://127.0.0.1:${API}\n`,
);

if (!values['no-watch']) {
  // The engine follows the Rust it is made of. (The Worker is rebuilt by wrangler's own watch.)
  let timer;
  let building = false;
  let again = false;
  const rebuild = () => {
    if (building) {
      again = true;
      return;
    }
    building = true;
    try {
      buildEngine();
      say('engine', 'rebuilt; the page reloads');
    } catch (e) {
      say('engine', `${e.message} (fix it and save again)`);
    }
    building = false;
    if (again) {
      again = false;
      rebuild();
    }
  };
  for (const dir of ['crates/tc-core/src', 'crates/browser/src']) {
    watch(join(ROOT, dir), { recursive: true }, (_, file) => {
      if (!file || !file.endsWith('.rs')) return;
      clearTimeout(timer);
      timer = setTimeout(rebuild, 800);
    });
  }
}
