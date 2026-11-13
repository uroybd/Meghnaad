// @vitest-environment node
// (jsdom does not always carry the streams API the browser has; Node does.)
import { afterEach, describe, expect, it, vi } from 'vitest';
import { forgetReplica, loadReplica, pack, saveReplica, unpack } from './persist';

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
