// @vitest-environment node
// (jsdom does not always carry the streams API the browser has; Node does.)
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  forgetKept,
  forgetReplica,
  keepAnswer,
  loadKept,
  loadReplica,
  pack,
  saveReplica,
  sealWith,
  unpack,
} from './persist';

const text = (bytes: Uint8Array) => new TextDecoder().decode(bytes);

describe('what the engine saves, kept compressed', () => {
  // What the engine writes: JSON, with the same words over and over.
  const saved = new TextEncoder().encode(
    JSON.stringify({
      format: 1,
      operations: Array.from({ length: 500 }, (_, i) => [true, { Update: { i, property: 'project' } }]),
    }),
  );

  it('comes back exactly as it went in, and smaller', async () => {
    const kept = await pack(saved);
    expect(kept.length).toBeLessThan(saved.length / 5);
    expect(await unpack(kept)).toEqual(saved);
  });

  it('still reads a copy kept before it was compressed', async () => {
    expect(await unpack(saved)).toBe(saved);
    expect(text(await unpack(saved))).toMatch(/^\{"format":1/);
  });

  it('refuses a damaged compressed copy rather than passing it on', async () => {
    const kept = await pack(saved);
    await expect(unpack(kept.slice(0, kept.length - 20))).rejects.toThrow();
  });

  it('keeps small and empty inputs apart from gzip', async () => {
    expect(await unpack(new Uint8Array([0x1f]))).toEqual(new Uint8Array([0x1f]));
    expect(await unpack(new Uint8Array())).toEqual(new Uint8Array());
  });
});

describe('when the browser will not keep it (a full disk, a blocked database)', () => {
  // An IndexedDB whose every open fails, as a private window or a full disk makes it.
  const broken = {
    open() {
      const req: { error: Error; onerror?: () => void } = { error: new Error('QuotaExceededError') };
      queueMicrotask(() => req.onerror?.());
      return req;
    },
  };
  afterEach(() => vi.unstubAllGlobals());

  it('reads as no copy at all, so the page starts from the bucket', async () => {
    vi.stubGlobal('indexedDB', broken);
    expect(await loadReplica()).toBeUndefined();
  });

  it('says a save failed rather than letting the page think its changes are kept', async () => {
    vi.stubGlobal('indexedDB', broken);
    await expect(saveReplica(new Uint8Array([1, 2, 3]))).rejects.toThrow('QuotaExceededError');
  });

  it('says a copy could not be thrown away, so the page does not reload into it', async () => {
    vi.stubGlobal('indexedDB', broken);
    await expect(forgetReplica()).rejects.toThrow('QuotaExceededError');
  });
});

/** An IndexedDB in memory: one database, one store, the few calls persist.ts makes. */
function memoryDb() {
  const data = new Map<unknown, unknown>();
  const request = <T>(value: () => T) => {
    const req: { result?: T; onsuccess?: () => void } = {};
    queueMicrotask(() => {
      req.result = value();
      req.onsuccess?.();
    });
    return req;
  };
  const db = {
    transaction() {
      const t: { oncomplete?: () => void; objectStore: () => unknown } = {
        objectStore: () => ({
          get: (k: unknown) => request(() => data.get(k)),
          put: (v: unknown, k: unknown) => request(() => data.set(k, v)),
          delete: (k: unknown) =>
            request(() => {
              // A key range (the stand-in for IDBKeyRange.bound below), or one key.
              if (k && typeof k === 'object' && 'lo' in k && 'hi' in k) {
                const { lo, hi } = k as { lo: string; hi: string };
                for (const key of [...data.keys()])
                  if (typeof key === 'string' && key >= lo && key <= hi) data.delete(key);
              } else data.delete(k);
            }),
        }),
      };
      setTimeout(() => t.oncomplete?.(), 0);
      return t;
    },
    close() {},
  };
  return {
    data,
    indexedDB: {
      open() {
        const req: { result: typeof db; onsuccess?: () => void } = { result: db };
        queueMicrotask(() => req.onsuccess?.());
        return req;
      },
    },
  };
}

describe('the static flavour seals the copy', () => {
  afterEach(() => vi.unstubAllGlobals());
  const replica = new TextEncoder().encode('{"format":1,"tasks":"Buy milk for the party"}');

  it('keeps nothing of the tasks readable, and opens it again with the same key', async () => {
    const mem = memoryDb();
    vi.stubGlobal('indexedDB', mem.indexedDB);
    const key = crypto.getRandomValues(new Uint8Array(32));
    await sealWith(key);
    await saveReplica(replica);
    const kept = mem.data.get('replica') as Uint8Array;
    expect(text(kept)).not.toContain('Buy milk');
    expect(text(kept.subarray(0, 4))).toBe('MGS1');
    expect(await loadReplica()).toEqual(replica);

    // Another key (another secret) reads it as no copy at all: the tasks come from the branch again.
    await sealWith(crypto.getRandomValues(new Uint8Array(32)));
    expect(await loadReplica()).toBeUndefined();
  });
});

describe('the settings the static flavour keeps', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('are kept sealed, read back with the key, and forgotten together without touching the tasks', async () => {
    const mem = memoryDb();
    vi.stubGlobal('indexedDB', mem.indexedDB);
    vi.stubGlobal('IDBKeyRange', { bound: (lo: string, hi: string) => ({ lo, hi }) });
    await sealWith(crypto.getRandomValues(new Uint8Array(32)));
    await saveReplica(new TextEncoder().encode('{"format":1}'));
    await keepAnswer('/api/config', '{"status":200,"body":"dateformat Y-M-D"}');
    await keepAnswer('/api/settings', '{"status":200,"body":"{}"}');
    expect(text(mem.data.get('kept:/api/config') as Uint8Array)).not.toContain('dateformat');
    expect(await loadKept('/api/config')).toBe('{"status":200,"body":"dateformat Y-M-D"}');

    await forgetKept();
    expect(await loadKept('/api/config')).toBeUndefined();
    expect(await loadKept('/api/settings')).toBeUndefined();
    expect(await loadReplica()).toBeDefined();
  });

  it('are not read with another key', async () => {
    const mem = memoryDb();
    vi.stubGlobal('indexedDB', mem.indexedDB);
    await sealWith(crypto.getRandomValues(new Uint8Array(32)));
    await keepAnswer('/api/config', '{}');
    await sealWith(crypto.getRandomValues(new Uint8Array(32)));
    expect(await loadKept('/api/config')).toBeUndefined();
  });
});
