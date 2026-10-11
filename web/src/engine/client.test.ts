// The engine client with a stand-in for the Web Worker: what it sends, how it keeps the replica in step with the
// bucket (and does not make the other tabs answer each other for ever), and what it shows when a sync fails.

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Call } from './protocol';

let sent: Call[] = [];
let replies: (c: Call) => unknown = () => null;
let hold: { resolve: () => void } | undefined;
let delay = false;

class FakeWorker {
  onmessage: ((e: { data: unknown }) => void) | null = null;
  onerror: ((e: { message: string }) => void) | null = null;
  constructor(..._args: unknown[]) {
    void _args;
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

const syncs = () => sent.filter((c) => c.op === 'sync').length;
const tick = () => new Promise((r) => setTimeout(r, 0));

async function fresh() {
  vi.resetModules();
  return (await import('./client.svelte')).EngineClient;
}

beforeEach(() => {
  sent = [];
  posted.length = 0;
  FakeChannel.last = undefined;
  delay = false;
  hold = undefined;
  replies = (c) => (c.op === 'start' ? false : c.op === 'sync' ? false : c.op === 'run' ? '{}' : null);
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
    expect(sent.map((c) => c.op)).toEqual(['start', 'sync']);
    expect(sent[0]).toMatchObject({ op: 'start', config: '{"c":1}', persist: true });
    e.stop();
  });

  it('answers at once from the saved copy, and syncs behind it', async () => {
    replies = (c) => (c.op === 'start' ? true : c.op === 'sync' ? false : null);
    const Engine = await fresh();
    const e = new Engine();
    delay = true;
    await e.start('{}');
    // Started without waiting for the sync that is still out.
    expect(sent.map((c) => c.op)).toEqual(['start', 'sync']);
    expect(e.status).toBe('syncing');
    hold?.resolve();
    await tick();
    expect(e.status).toBe('idle');
    e.stop();
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
      return c.op === 'start' ? false : null;
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
    replies = (c) => (c.op === 'sync' ? false : null);
    await e.sync();
    expect(seen).not.toHaveBeenCalled();
    replies = (c) => (c.op === 'sync' ? true : null);
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
    replies = () => false;
    await e.sync();
    expect(e.status).toBe('idle');
    expect(e.error).toBeNull();
    expect(e.lastSync).not.toBeNull();
    e.stop();
  });

  it('tells the other tabs about a write it pushed, but never about a sync that only pulled', async () => {
    const Engine = await fresh();
    const e = new Engine();
    await e.start('{}');
    posted.length = 0;
    await e.sync();
    expect(posted).toEqual([]);
    e.pushSoon();
    await tick();
    await tick();
    expect(posted).toEqual(['pushed']);
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
