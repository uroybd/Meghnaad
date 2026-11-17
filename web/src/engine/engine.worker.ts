// The engine, in a Web Worker: the same Taskwarrior engine the Worker ran for every request, here, holding the
// replica in memory, so a command costs nothing on the server. The page talks to it by messages (protocol.ts), which
// are answered one at a time in the order they arrive, as the engine cannot be entered twice.

//
// The static flavour has no Worker behind it: the engine is unlocked at start with the git branch and its key, answers
// the settings routes itself (`api`), and runs the hooks with the hooks module, loaded the first time one runs.

import init, { Engine } from 'tc-engine';
import wasmUrl from 'tc-engine-wasm?url';
import { STATIC } from './flavour';
import { forgetReplica, loadReplica, saveReplica, sealWith } from './persist';
import type { ApiAnswer, Call, EngineReply, Synced } from './protocol';
import * as hooks from './pageHooks';

let engine: Engine | undefined;
let persist = false;
/** The engine's change counter when it was last saved. */
let saved = -1;
let saveTimer: ReturnType<typeof setTimeout> | undefined;
/** Why the last save failed, until one works: told to the page with the next sync. */
let unkept: string | null = null;

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
    queue = queue.then(keep).then(
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
    case 'api':
      return (
        (await hooks.route(m.method, m.path, m.body)) ??
        (JSON.parse(await engine.api(m.method, m.path, new TextEncoder().encode(m.body))) as ApiAnswer)
      );
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
    case 'sync': {
      const before = engine.changes();
      await engine.sync('');
      const changed = engine.changes() !== before;
      await keep().then(
        () => (unkept = null),
        (err: unknown) => (unkept = String(err)),
      );
      return { changed, unkept } satisfies Synced;
    }
    case 'forget':
      await forgetReplica();
      saved = -1;
      return null;
  }
}

let queue: Promise<unknown> = Promise.resolve();

self.onmessage = (e: MessageEvent<Call>) => {
  const m = e.data;
  queue = queue
    .then(() => handle(m))
    .then(
      (value) => self.postMessage({ id: m.id, ok: true, value } satisfies EngineReply),
      (err: unknown) => {
        const error = typeof err === 'string' ? err : err instanceof Error ? err.message : String(err);
        self.postMessage({ id: m.id, ok: false, error } satisfies EngineReply);
      },
    );
};
