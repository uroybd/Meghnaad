// The engine client with a stand-in for the Web Worker: what it sends, how it keeps the replica in step with the
// bucket (and does not make the other tabs answer each other for ever), and what it shows when a sync fails.

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Call } from './protocol';
import { PUSH_AFTER_MS } from './client.svelte';

// This tests the real client, not the stand-in that test-setup.ts gives every other test.
vi.unmock('./client.svelte');

let sent: Call[] = [];
let replies: (c: Call) => unknown = () => null;
let hold: { resolve: () => void } | undefined;
let delay = false;
let made = 0;

class FakeWorker {
  onmessage: ((e: { data: unknown }) => void) | null = null;
  onerror: ((e: { message: string }) => void) | null = null;
  constructor(..._args: unknown[]) {
    void _args;
    made++;
  }
  postMessage(c: Call) {
    sent.push(c);
    const answer = () => {
      try {
        this.onmessage?.({ data: { id: c.id, ok: true, value: replies(c) } });
      } catch (e) {
        this.onmessage?.({ data: { id: c.id, ok: false, error: (e as Error).message } });
      }
    };
    if (delay && c.op === 'sync') {
      new Promise<void>((resolve) => (hold = { resolve })).then(answer);
    } else queueMicrotask(answer);
  }
  terminate() {}
}

const posted: string[] = [];
class FakeChannel {
  /** The channel the client opened, so a test can play another tab. */
  static last: FakeChannel | undefined;
  onmessage: (() => void) | null = null;
  constructor() {
    FakeChannel.last = this;
  }
  postMessage(m: string) {
    posted.push(m);
  }
  close() {}
}

/** What the Worker answers a sync with. */
const synced = (changed: boolean, unkept: string | null = null) => ({ changed, unkept });
const syncs = () => sent.filter((c) => c.op === 'sync').length;
const tick = () => new Promise((r) => setTimeout(r, 0));

async function fresh() {
  vi.resetModules();
  return (await import('./client.svelte')).EngineClient;
}

beforeEach(() => {
  sent = [];
  made = 0;
  posted.length = 0;
  FakeChannel.last = undefined;
  delay = false;
  hold = undefined;
  replies = (c) =>
    c.op === 'start'
      ? { restored: false, damaged: false }
      : c.op === 'sync'
        ? synced(false)
        : c.op === 'run'
          ? '{}'
          : null;
  vi.stubGlobal('Worker', FakeWorker);
  vi.stubGlobal('BroadcastChannel', FakeChannel);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe('starting', () => {
  it('pulls everything before it answers, when nothing was saved in this browser', async () => {
    const Engine = await fresh();
    const e = new Engine();
    await e.start('{"c":1}');
    expect(sent.map((c) => c.op)).toEqual(['start', 'config', 'sync']);
    expect(sent[0]).toMatchObject({ op: 'start', persist: true });
    expect(sent[1]).toMatchObject({ op: 'config', config: '{"c":1}' });
    e.stop();
  });

  it('says when the copy this browser kept could not be read, and starts from the bucket', async () => {
    replies = (c) => (c.op === 'start' ? { restored: false, damaged: true } : c.op === 'sync' ? synced(false) : null);
    const Engine = await fresh();
    const e = new Engine();
    await e.start('{}');
    expect(e.damaged).toBe(true);
    // Nothing to answer from: the first command waits for the bucket.
    expect(sent.map((c) => c.op)).toEqual(['start', 'config', 'sync']);
    e.stop();
  });

  it('answers at once from the saved copy, and syncs behind it', async () => {
    replies = (c) => (c.op === 'start' ? { restored: true, damaged: false } : c.op === 'sync' ? synced(false) : null);
    const Engine = await fresh();
    const e = new Engine();
    delay = true;
    await e.start('{}');
    // Started without waiting for the sync that is still out.
    expect(sent.map((c) => c.op)).toEqual(['start', 'config', 'sync']);
    expect(e.status).toBe('syncing');
    hold?.resolve();
    await tick();
    expect(e.status).toBe('idle');
    e.stop();
  });

  it('can load the Worker before the settings are known, and then starts on that one', async () => {
    const Engine = await fresh();
    const e = new Engine();
    e.preload();
    e.preload();
    await tick();
    expect(sent.map((c) => c.op)).toEqual(['start']);
    await e.start('{"c":1}');
    expect(made).toBe(1);
    expect(sent.map((c) => c.op)).toEqual(['start', 'config', 'sync']);
    expect(sent[1]).toMatchObject({ op: 'config', config: '{"c":1}' });
    e.stop();
  });

  it('says why it could not start even when the Worker was loaded ahead', async () => {
    replies = (c) => {
      if (c.op === 'start') throw new Error('the saved copy is unreadable');
      return null;
    };
    const Engine = await fresh();
    const e = new Engine();
    e.preload();
    await tick();
    await expect(e.start('{}')).rejects.toThrow('the saved copy is unreadable');
    e.stop();
  });

  it('does nothing to preload where there is no Worker', async () => {
    vi.stubGlobal('Worker', undefined);
    const Engine = await fresh();
    const e = new Engine();
    e.preload();
    expect(sent).toEqual([]);
  });

  it('starts only once however many ask', async () => {
    const Engine = await fresh();
    const e = new Engine();
    await Promise.all([e.start('{}'), e.start('{}'), e.start('{}')]);
    expect(sent.filter((c) => c.op === 'start')).toHaveLength(1);
    e.stop();
  });

  it('does not start if the first pull fails, and says why', async () => {
    replies = (c) => {
      if (c.op === 'sync') throw new Error('could not reach the server');
      return c.op === 'start' ? { restored: false, damaged: false } : null;
    };
    const Engine = await fresh();
    const e = new Engine();
    await expect(e.start('{}')).rejects.toThrow('could not reach the server');
    expect(e.status).toBe('failed');
    expect(e.error).toBe('could not reach the server');
    e.stop();
  });
});

describe('syncing', () => {
  it('runs one at a time and once more if asked while one was out', async () => {
    const Engine = await fresh();
    const e = new Engine();
    await e.start('{}');
    sent = [];
    delay = true;
    void e.sync();
    void e.sync();
    void e.sync();
    await tick();
    expect(syncs()).toBe(1);
    hold?.resolve();
    await tick();
    await tick();
    // The three asks became two syncs: the one under way, and one more for what came after it started.
    expect(syncs()).toBe(2);
    hold?.resolve();
    await tick();
    await tick();
    // And that is the end of it: asking again while one was out does not keep them coming for ever.
    expect(syncs()).toBe(2);
    e.stop();
  });

  it('tells the page when a sync brought something in, and not when it did not', async () => {
    const Engine = await fresh();
    const e = new Engine();
    await e.start('{}');
    const seen = vi.fn();
    e.onChanged(seen);
    replies = (c) => (c.op === 'sync' ? synced(false) : null);
    await e.sync();
    expect(seen).not.toHaveBeenCalled();
    replies = (c) => (c.op === 'sync' ? synced(true) : null);
    await e.sync();
    expect(seen).toHaveBeenCalledTimes(1);
    e.stop();
  });

  it('keeps what failed to show, and clears it when the next sync works', async () => {
    const Engine = await fresh();
    const e = new Engine();
    await e.start('{}');
    replies = (c) => {
      if (c.op === 'sync') throw new Error('push: status 502');
      return null;
    };
    await e.sync();
    expect(e.status).toBe('failed');
    expect(e.error).toBe('push: status 502');
    replies = () => synced(false);
    await e.sync();
    expect(e.status).toBe('idle');
    expect(e.error).toBeNull();
    expect(e.lastSync).not.toBeNull();
    e.stop();
  });

  it('says when this browser could not keep the changes, and stops saying so once it can', async () => {
    const Engine = await fresh();
    const e = new Engine();
    await e.start('{}');
    expect(e.unkept).toBeNull();
    replies = (c) => (c.op === 'sync' ? synced(false, 'QuotaExceededError') : null);
    await e.sync();
    // The push worked: the sync itself did not fail, only the copy in this browser.
    expect(e.status).toBe('idle');
    expect(e.unkept).toBe('QuotaExceededError');
    replies = (c) => (c.op === 'sync' ? synced(false) : null);
    await e.sync();
    expect(e.unkept).toBeNull();
    e.stop();
  });

  it('tells the other tabs about a write it pushed, but never about a sync that only pulled', async () => {
    vi.useFakeTimers();
    const Engine = await fresh();
    const e = new Engine();
    const started = e.start('{}');
    await vi.advanceTimersByTimeAsync(0);
    await started;
    posted.length = 0;
    await e.sync();
    expect(posted).toEqual([]);
    e.pushSoon();
    await vi.advanceTimersByTimeAsync(PUSH_AFTER_MS);
    expect(posted).toEqual(['pushed']);
    e.stop();
  });

  it('pushes a run of writes once, a moment after the last', async () => {
    vi.useFakeTimers();
    const Engine = await fresh();
    const e = new Engine();
    const started = e.start('{}');
    await vi.advanceTimersByTimeAsync(0);
    await started;
    sent = [];
    e.pushSoon();
    await vi.advanceTimersByTimeAsync(1_500);
    e.pushSoon();
    await vi.advanceTimersByTimeAsync(1_500);
    expect(syncs()).toBe(0);
    await vi.advanceTimersByTimeAsync(500);
    expect(syncs()).toBe(1);
    await vi.advanceTimersByTimeAsync(10_000);
    expect(syncs()).toBe(1);
    e.stop();
  });

  it('pushes at once when the tab is hidden or closed, and not when nothing waits', async () => {
    vi.useFakeTimers();
    const Engine = await fresh();
    const e = new Engine();
    const started = e.start('{}');
    await vi.advanceTimersByTimeAsync(0);
    await started;
    sent = [];
    const visibility = vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('hidden');
    document.dispatchEvent(new Event('visibilitychange'));
    await vi.advanceTimersByTimeAsync(0);
    expect(syncs()).toBe(0);
    e.pushSoon();
    document.dispatchEvent(new Event('visibilitychange'));
    await vi.advanceTimersByTimeAsync(0);
    expect(syncs()).toBe(1);
    visibility.mockRestore();
    e.pushSoon();
    window.dispatchEvent(new Event('pagehide'));
    await vi.advanceTimersByTimeAsync(0);
    expect(syncs()).toBe(2);
    // Neither is pushed again when the moment comes.
    await vi.advanceTimersByTimeAsync(PUSH_AFTER_MS);
    expect(syncs()).toBe(2);
    e.stop();
  });

  it('lets a sync that runs meanwhile carry the push', async () => {
    vi.useFakeTimers();
    const Engine = await fresh();
    const e = new Engine();
    const started = e.start('{}');
    await vi.advanceTimersByTimeAsync(0);
    await started;
    sent = [];
    posted.length = 0;
    e.pushSoon();
    await e.sync();
    expect(syncs()).toBe(1);
    expect(posted).toEqual(['pushed']);
    await vi.advanceTimersByTimeAsync(PUSH_AFTER_MS);
    expect(syncs()).toBe(1);
    e.stop();
  });

  it('pulls when another tab says it pushed, without announcing that in turn', async () => {
    const Engine = await fresh();
    const e = new Engine();
    await e.start('{}');
    sent = [];
    posted.length = 0;
    FakeChannel.last?.onmessage?.();
    await tick();
    await tick();
    expect(syncs()).toBe(1);
    expect(posted).toEqual([]);
    e.stop();
  });

  it('pulls on a timer while visible, and when the tab is looked at again after a while', async () => {
    vi.useFakeTimers();
    const Engine = await fresh();
    const e = new Engine();
    const started = e.start('{}');
    await vi.advanceTimersByTimeAsync(0);
    await started;
    sent = [];
    await vi.advanceTimersByTimeAsync(30_000);
    expect(syncs()).toBe(1);
    // Looked at again at once: too soon since the last pull to be worth a request.
    document.dispatchEvent(new Event('visibilitychange'));
    await vi.advanceTimersByTimeAsync(0);
    expect(syncs()).toBe(1);
    await vi.advanceTimersByTimeAsync(5_000);
    document.dispatchEvent(new Event('visibilitychange'));
    await vi.advanceTimersByTimeAsync(0);
    expect(syncs()).toBe(2);
    e.stop();
  });
});

describe('the settings the static site kept', () => {
  const answer = (body: unknown) => ({ status: 200, type: 'application/json', body: JSON.stringify(body) });

  it('answers the first read from what was kept, then reads the branch and says when it differed', async () => {
    const Engine = await fresh();
    const e = new Engine();
    replies = (c) =>
      c.op === 'start'
        ? true
        : c.op === 'api'
          ? { ...answer({ config: { dateformat: 'kept' } }), kept: c.kept }
          : c.op === 'refresh'
            ? { changed: true, answer: answer({ config: { dateformat: 'branch' } }) }
            : c.op === 'sync'
              ? synced(false)
              : null;
    const told: string[] = [];
    e.onSettingsChanged((path) => told.push(path));
    const first = await e.api('GET', '/api/config', '');
    expect(first.kept).toBe(true);
    expect(JSON.parse(first.body)).toEqual({ config: { dateformat: 'kept' } });
    await e.start(JSON.stringify({ dateformat: 'kept' }));
    await tick();
    await tick();
    expect(sent.filter((c) => c.op === 'refresh').map((c) => c.op === 'refresh' && c.path)).toEqual(['/api/config']);
    // The engine is given the branch's settings, after the kept ones it started on.
    const configs = sent.flatMap((c) => (c.op === 'config' ? [c.config] : []));
    expect(configs).toEqual([JSON.stringify({ dateformat: 'kept' }), JSON.stringify({ dateformat: 'branch' })]);
    expect(told).toEqual(['/api/config']);
    // Only the first read is answered from what was kept.
    await e.api('GET', '/api/config', '');
    const asked = sent.flatMap((c) => (c.op === 'api' ? [c.kept] : []));
    expect(asked).toEqual([true, false]);
    e.stop();
  });

  it('says nothing when the branch has what was kept, or when it could not be read', async () => {
    const Engine = await fresh();
    const e = new Engine();
    let refreshes = 0;
    replies = (c) => {
      if (c.op === 'start') return true;
      if (c.op === 'api') return { ...answer({}), kept: true };
      if (c.op === 'refresh') {
        if (++refreshes === 2) throw new Error('offline');
        return { changed: false, answer: answer({}) };
      }
      return null;
    };
    const told: string[] = [];
    e.onSettingsChanged((path) => told.push(path));
    await e.api('GET', '/api/settings', '');
    await e.api('GET', '/api/hooks/defined', '');
    await tick();
    await tick();
    expect(refreshes).toBe(2);
    expect(told).toEqual([]);
    e.stop();
  });

  it('never asks for a kept answer for a write or a route that is not kept', async () => {
    const Engine = await fresh();
    const e = new Engine();
    replies = (c) => (c.op === 'start' ? { restored: true, damaged: false } : c.op === 'api' ? answer({}) : null);
    await e.api('PUT', '/api/settings', '{}');
    await e.api('GET', '/api/config/taskrc', '');
    expect(sent.flatMap((c) => (c.op === 'api' ? [c.kept] : []))).toEqual([false, false]);
    expect(sent.some((c) => c.op === 'refresh')).toBe(false);
    e.stop();
  });
});

describe('commands', () => {
  it('sends a command as JSON text, and the settings and an import as they are', async () => {
    const Engine = await fresh();
    const e = new Engine();
    await e.start('{}');
    sent = [];
    await e.run({ line: 'count', tz: 0, now: 5 });
    await e.setConfig('{"a":1}');
    await e.import('[]', false, 3600);
    expect(sent[0]).toMatchObject({ op: 'run', request: '{"line":"count","tz":0,"now":5}' });
    expect(sent[1]).toMatchObject({ op: 'config', config: '{"a":1}' });
    expect(sent[2]).toMatchObject({ op: 'import', text: '[]', apply: false, tz: 3600 });
    e.stop();
  });

  it('rejects with what the engine said when it refuses', async () => {
    const Engine = await fresh();
    const e = new Engine();
    await e.start('{}');
    replies = (c) => {
      if (c.op === 'run') throw new Error('invalid request');
      return null;
    };
    await expect(e.run({})).rejects.toThrow('invalid request');
    e.stop();
  });
});
