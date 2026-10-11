#!/usr/bin/env node
// Runs commands with the engine, from a shell, against a Worker: what the browser does, without the browser. For
// seeding demo data, for the real-CLI interop test, and for poking at a deployment from a terminal.
//
//   node scripts/engine-cli.mjs --api http://127.0.0.1:8787 'add Buy milk +errand' '{"args":["1","done"]}'
//   printf 'count\nnext\n' | node scripts/engine-cli.mjs --api http://127.0.0.1:8787 -
//
// Each command is a typed line or a JSON request (`{ "line": … }` or `{ "args": […] }`, as the page sends them),
// and each answer is one line of JSON, the same the page reads. The replica starts empty: it pulls everything first,
// and pushes after a command that wrote. A `config` command's new settings are saved on the Worker. Exits 1 if
// anything fails or a command is refused. The Worker has to accept the request: locally that is `DEV_AUTH_BYPASS`.
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { parseArgs } from 'node:util';
import { buildEngine, NODE_OUT, stale } from './build-engine.mjs';
import { join } from 'node:path';

const { values, positionals } = parseArgs({
  options: { api: { type: 'string', default: 'http://127.0.0.1:8787' }, tz: { type: 'string', default: '0' } },
  allowPositionals: true,
});
const api = values.api.replace(/\/$/, '');

if (stale('node')) {
  // What the build says goes to stderr: stdout is the answers, one line of JSON each.
  const say = console.log;
  console.log = console.error;
  buildEngine('node');
  console.log = say;
}
const { Engine } = createRequire(import.meta.url)(join(NODE_OUT, 'tc_browser.js'));

async function json(path, init) {
  const res = await fetch(api + path, init);
  const text = await res.text();
  if (!res.ok) throw new Error(`${path}: status ${res.status} ${text.slice(0, 200)}`);
  return JSON.parse(text);
}

const commands = positionals.flatMap((c) => (c === '-' ? readFileSync(0, 'utf8').split('\n').filter(Boolean) : [c]));
if (commands.length === 0) {
  console.error('Give one or more commands, or - to read them from the standard input.');
  process.exit(2);
}

let failed = false;
try {
  const engine = new Engine();
  engine.set_config(JSON.stringify((await json('/api/config')).config));
  await engine.sync(api);
  // The hooks the Worker's script defines: they run for these commands as they do in the page.
  engine.set_hooks(JSON.stringify((await json('/api/hooks/defined')).defined), api);
  for (const command of commands) {
    const request = command.trim().startsWith('{') ? JSON.parse(command) : { line: command };
    const text = await engine.run(JSON.stringify({ tz: Number(values.tz), ...request, now: Date.now() / 1000 }));
    const answer = JSON.parse(text);
    if (answer.error !== undefined) failed = true;
    if (answer.config) {
      await json('/api/config/engine', {
        method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(answer.config),
      });
      engine.set_config(JSON.stringify(answer.config));
      delete answer.config;
    }
    if (answer.wrote) await engine.sync(api);
    console.log(JSON.stringify(answer));
  }
} catch (e) {
  console.error(e instanceof Error ? e.message : e);
  failed = true;
}
process.exit(failed ? 1 : 0);
