// The engine, in a Web Worker: the same Taskwarrior engine the Worker ran for every request, here, holding the
// replica in memory, so a command costs nothing on the server. The page talks to it by messages (protocol.ts), which
// are answered one at a time in the order they arrive, as the engine cannot be entered twice.

import init, { Engine } from 'tc-engine';
import wasmUrl from './pkg/tc_browser_bg.wasm?url';
import { forgetReplica, loadReplica, saveReplica } from './persist';
import type { Call, EngineReply } from './protocol';

let engine: Engine | undefined;
let persist = false;
/** The engine's change counter when it was last saved. */
let saved = -1;
let saveTimer: ReturnType<typeof setTimeout> | undefined;

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
    queue = queue.then(keep).catch(() => {});
  }, 400);
}

async function handle(m: Call): Promise<unknown> {
  if (m.op === 'start') {
    await init({ module_or_path: wasmUrl });
    engine = new Engine();
    engine.set_config(m.config);
    persist = m.persist;
    let restored = false;
    if (persist) {
      const bytes = await loadReplica();
      if (bytes) {
        try {
          engine.restore(bytes);
          saved = engine.changes();
          restored = true;
        } catch {
          // An unusable copy (another version of the engine wrote it): start from the bucket.
          await forgetReplica();
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
      return null;
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
      await keep();
      return changed;
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
