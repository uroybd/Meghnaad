import { describe, expect, it } from 'vitest';
import { serial, splitSync } from './queue';

/** A promise and the function that settles it. */
function later<T>(): { promise: Promise<T>; resolve: (v: T) => void } {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

describe('the engine queue', () => {
  it('runs jobs one at a time, in order, and goes on after one fails', async () => {
    const inTurn = serial();
    const seen: string[] = [];
    const slow = later<void>();
    const a = inTurn(async () => {
      await slow.promise;
      seen.push('a');
    });
    const b = inTurn(() => {
      throw new Error('b failed');
    });
    const c = inTurn(() => seen.push('c'));
    slow.resolve();
    await a;
    await expect(b).rejects.toThrow('b failed');
    await c;
    expect(seen).toEqual(['a', 'c']);
  });

  it('answers a command sent while a sync fetches, before the fetch ends', async () => {
    const inTurn = serial();
    const seen: string[] = [];
    const store = later<string>();
    const synced = splitSync(
      inTurn,
      () => 'base',
      async (base) => {
        seen.push(`fetch from ${base}`);
        return store.promise;
      },
      async (fetched) => {
        seen.push(`apply ${fetched}`);
        return 'done';
      },
    );
    // Let the source be taken and the fetch begin.
    await new Promise((r) => setTimeout(r, 0));
    const command = await inTurn(() => {
      seen.push('command');
      return 'answered';
    });
    expect(command).toBe('answered');
    expect(seen).toEqual(['fetch from base', 'command']);
    store.resolve('v2');
    expect(await synced).toBe('done');
    expect(seen).toEqual(['fetch from base', 'command', 'apply v2']);
  });

  it('applies the fetch in turn, after a command already waiting', async () => {
    const inTurn = serial();
    const seen: string[] = [];
    const busy = later<void>();
    const store = later<string>();
    const synced = splitSync(
      inTurn,
      () => 'base',
      () => store.promise,
      async (v) => void seen.push(`apply ${v}`),
    );
    await new Promise((r) => setTimeout(r, 0));
    // A slow command takes its turn while the fetch is under way; the fetch ends before it does.
    const command = inTurn(async () => {
      await busy.promise;
      seen.push('command');
    });
    store.resolve('v2');
    await new Promise((r) => setTimeout(r, 0));
    busy.resolve();
    await Promise.all([command, synced]);
    expect(seen).toEqual(['command', 'apply v2']);
  });
});
