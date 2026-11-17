// The static flavour's hooks, in the engine's Web Worker. With no Worker behind the page, what the Worker's hook routes
// do is done here: the script is checked and run by the hooks module (crates/browser-hooks: the same Lua runner, as
// WebAssembly), which is downloaded the first time it is needed. The engine asks for a hook as it would ask a Worker
// (`POST /api/run`, see crates/browser/src/hooks.rs), and that one request is answered here instead of going out.

import initHooks, { check, Hooks } from 'tc-hooks';
import hooksWasm from 'tc-hooks-wasm?url';
import type { Engine } from 'tc-engine';
import type { ApiAnswer } from './protocol';

const HOOK_NAMES = ['on_launch', 'on_add', 'on_modify', 'on_exit'];
const JSON_TYPE = 'application/json; charset=utf-8';
const nothing = new Uint8Array();

let engine: Engine | undefined;
/** The saved script and its secrets, read between commands (a hook runs inside one, when the engine is busy). */
let script: { source: string; secrets: Uint8Array } | undefined;
let defined: string[] = [];
let module: Promise<unknown> | undefined;
let loaded: Promise<Hooks> | undefined;

const loadModule = () => (module ??= initHooks({ module_or_path: hooksWasm }));

const answer = (status: number, body: unknown): ApiAnswer => ({ status, type: JSON_TYPE, body: JSON.stringify(body) });

/** Which hooks a script has, without loading the Lua: a name that appears in it. One too many is harmless (the runner
 * answers nothing for a hook the script doesn't define); the module is then loaded only for a script that has one. */
export function namesIn(source: string): string[] {
  return HOOK_NAMES.filter((n) => new RegExp(`\\b${n}\\b`).test(source));
}

async function source(): Promise<string> {
  if (!engine) return '';
  const a = JSON.parse(await engine.api('GET', '/api/hooks', nothing)) as ApiAnswer;
  return a.status === 200 ? ((JSON.parse(a.body) as { source?: string }).source ?? '') : '';
}

/** Answer the engine's `POST /api/run` here; every other request goes out as it would. */
export function attach(e: Engine): void {
  engine = e;
  const real = self.fetch.bind(self);
  self.fetch = (input: RequestInfo | URL, init?: RequestInit) =>
    input === '/api/run' ? run(init?.body) : real(input, init);
}

/** Read the script and its secrets now, for the hooks the engine was told about. */
export async function prepare(names: string[]): Promise<void> {
  defined = names;
  void loaded?.then((h) => h.free()).catch(() => undefined);
  loaded = undefined;
  script = undefined;
  if (!names.length || !engine) return;
  script = { source: await source(), secrets: await engine.hook_secrets() };
}

async function run(body: BodyInit | null | undefined): Promise<Response> {
  const reply = (text: string) => new Response(text, { status: 200, headers: { 'content-type': JSON_TYPE } });
  const kept = script;
  if (!kept) return reply(JSON.stringify({ ok: true, value: null, say: [] }));
  loaded ??= loadModule().then(() => new Hooks(kept.source, kept.secrets));
  const text = body instanceof Uint8Array ? new TextDecoder().decode(body) : String(body ?? '');
  try {
    return reply(await (await loaded).run(text));
  } catch (e) {
    loaded = undefined;
    return reply(JSON.stringify({ ok: false, error: `The hooks could not run: ${String(e)}`, say: [] }));
  }
}

/** The hook routes the Worker answers, answered here; `undefined` for any other route (the engine answers those). */
export async function route(method: string, path: string, body: string): Promise<ApiAnswer | undefined> {
  if (!engine) return undefined;
  if (method === 'GET' && path === '/api/hooks/defined') {
    const text = await source();
    return answer(200, { defined: text.trim() ? namesIn(text) : [] });
  }
  if (method === 'POST' && path === '/api/hooks/check') {
    await loadModule();
    return { status: 200, type: JSON_TYPE, body: check(body) };
  }
  if (method === 'PUT' && path === '/api/hooks') {
    let names: string[] = [];
    if (body.trim()) {
      await loadModule();
      const checked = JSON.parse(check(body)) as { ok: boolean; defined?: string[]; line?: number; message?: string };
      if (!checked.ok) {
        const at = checked.line ? `line ${checked.line}: ` : '';
        return answer(400, { error: `The script was not saved. ${at}${checked.message ?? ''}` });
      }
      names = checked.defined ?? [];
    }
    const saved = JSON.parse(await engine.api(method, path, new TextEncoder().encode(body))) as ApiAnswer;
    return saved.status === 200 ? answer(200, { ok: true, defined: names }) : saved;
  }
  if (path.startsWith('/api/secrets/') && method !== 'GET') {
    // A changed secret is the one the next hook gets.
    const done = JSON.parse(await engine.api(method, path, new TextEncoder().encode(body))) as ApiAnswer;
    await prepare(defined);
    return done;
  }
  return undefined;
}
