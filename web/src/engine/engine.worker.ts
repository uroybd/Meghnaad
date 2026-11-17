// The engine, in a Web Worker: the same Taskwarrior engine the Worker ran for every request, here, holding the
// replica in memory, so a command costs nothing on the server. The page talks to it by messages (protocol.ts), which
// are answered one at a time in the order they arrive, as the engine cannot be entered twice; a sync asks the store
// what is new outside that turn, so commands are answered meanwhile (queue.ts).

//
// The static flavour has no Worker behind it: the engine is unlocked at start with the git branch and its key, answers
// the settings routes itself (`api`), and runs the hooks with the hooks module, loaded the first time one runs.

import init, { Engine, fetchNew } from 'tc-engine';
import wasmUrl from 'tc-engine-wasm?url';
import { STATIC } from './flavour';
import { forgetKept, forgetReplica, keepAnswer, loadKept, loadReplica, saveReplica, sealWith } from './persist';
import { KEPT_ROUTES, type ApiAnswer, type Call, type EngineReply, type Refreshed, type Synced } from './protocol';
import * as hooks from './pageHooks';
import { serial, splitSync } from './queue';

let engine: Engine | undefined;
let persist = false;
/** The engine's change counter when it was last saved. */
let saved = -1;
let saveTimer: ReturnType<typeof setTimeout> | undefined;
/** Why the last save failed, until one works: told to the page with the next sync. */
let unkept: string | null = null;
/** The engine's turn: one message at a time. */
const inTurn = serial();

/** Save the replica if it changed. Throws when it cannot be saved, and then counts it as not saved. */
async function keep(): Promise<void> {
  if (!persist || !engine) return;
  const now = engine.changes();
  if (now === saved) return;
  await saveReplica(await engine.save());
  saved = now;
}

/** Soon, and once for a burst of changes. */
function keepSoon(): void {
  if (!persist || saveTimer) return;
  saveTimer = setTimeout(() => {
    saveTimer = undefined;
    inTurn(keep).then(
      () => (unkept = null),
      (err: unknown) => (unkept = String(err)),
    );
  }, 400);
}

async function handle(m: Call): Promise<unknown> {
  if (m.op === 'start') {
    await init({ module_or_path: wasmUrl });
    engine = new Engine();
    persist = m.persist;
    if (m.vault) {
      const v = m.vault;
      await sealWith(v.key);
      engine.connect_git(v.api, v.repo, v.branch, v.token, v.key);
      hooks.attach(engine);
    }
    let restored = false;
    if (persist) {
      const bytes = await loadReplica();
      if (bytes) {
        try {
          engine.restore(bytes);
          saved = engine.changes();
          restored = true;
        } catch {
          // An unusable copy (another version of the engine wrote it): start from the bucket. If it cannot be thrown
          // away either, the next start finds it unusable again and does the same.
          await forgetReplica().catch(() => undefined);
        }
      }
    }
    return restored;
  }
  if (!engine) throw new Error('the engine has not started');
  switch (m.op) {
    case 'config':
      engine.set_config(m.config);
      return null;
    case 'hooks':
      engine.set_hooks(m.defined, '');
      // The script and its secrets are read now, between commands: a hook runs in the middle of one, when the engine
      // can't be asked for anything.
      if (STATIC) await hooks.prepare(JSON.parse(m.defined) as string[]);
      return null;
    case 'api': {
      const keepable = STATIC && m.method === 'GET' && KEPT_ROUTES.has(m.path);
      if (keepable && m.kept) {
        const kept = await loadKept(m.path);
        if (kept) return { ...(JSON.parse(kept) as ApiAnswer), kept: true };
      }
      const answer =
        (await hooks.route(m.method, m.path, m.body)) ??
        (JSON.parse(await engine.api(m.method, m.path, new TextEncoder().encode(m.body))) as ApiAnswer);
      // Kept to start from next time; and after a write, none is kept, so the next read is the branch's.
      if (keepable && answer.status === 200) await keepAnswer(m.path, JSON.stringify(answer)).catch(() => undefined);
      else if (STATIC && m.method !== 'GET') await forgetKept().catch(() => undefined);
      return answer;
    }
    case 'run': {
      const before = engine.changes();
      const answer = await engine.run(m.request);
      if (engine.changes() !== before) keepSoon();
      return answer;
    }
    case 'import': {
      const before = engine.changes();
      const answer = await engine.import(m.text, m.apply, m.tz, m.now);
      if (engine.changes() !== before) keepSoon();
      return answer;
    }
    case 'forget':
      await forgetReplica();
      await forgetKept().catch(() => undefined);
      saved = -1;
      return null;
  }
}

/** One of `KEPT_ROUTES` read from the branch while other messages are answered, and kept if it changed. */
async function refresh(path: string): Promise<Refreshed> {
  const reader = await inTurn(() => {
    if (!engine) throw new Error('the engine has not started');
    return engine.reader();
  });
  let answer: ApiAnswer;
  try {
    if (path === '/api/hooks/defined') {
      // Answered by the hooks module from the script (pageHooks.ts), as the engine's own answer to this is.
      const script = JSON.parse(await reader.get('/api/hooks')) as ApiAnswer;
      const source = script.status === 200 ? ((JSON.parse(script.body) as { source?: string }).source ?? '') : '';
      answer =
        script.status === 200
          ? {
              status: 200,
              type: script.type,
              body: JSON.stringify({ defined: source.trim() ? hooks.namesIn(source) : [] }),
            }
          : script;
    } else answer = JSON.parse(await reader.get(path)) as ApiAnswer;
  } finally {
    reader.free();
  }
  if (answer.status !== 200) return { changed: false, answer };
  const now = JSON.stringify(answer);
  const before = await loadKept(path);
  if (before === now) return { changed: false, answer };
  await keepAnswer(path, now).catch(() => undefined);
  return { changed: true, answer };
}

/** Pull and push: what is new is fetched while other messages are answered, then applied in turn. */
function sync(): Promise<Synced> {
  return splitSync(
    inTurn,
    () => {
      if (!engine) throw new Error('the engine has not started');
      return engine.source('');
    },
    fetchNew,
    async (fetched) => {
      if (!engine) throw new Error('the engine has not started');
      const before = engine.changes();
      await engine.sync('', fetched);
      const changed = engine.changes() !== before;
      await keep().then(
        () => (unkept = null),
        (err: unknown) => (unkept = String(err)),
      );
      return { changed, unkept } satisfies Synced;
    },
  );
}

self.onmessage = (e: MessageEvent<Call>) => {
  const m = e.data;
  void (m.op === 'sync' ? sync() : m.op === 'refresh' ? refresh(m.path) : inTurn(() => handle(m))).then(
    (value) => self.postMessage({ id: m.id, ok: true, value } satisfies EngineReply),
    (err: unknown) => {
      const error = typeof err === 'string' ? err : err instanceof Error ? err.message : String(err);
      self.postMessage({ id: m.id, ok: false, error } satisfies EngineReply);
    },
  );
};
