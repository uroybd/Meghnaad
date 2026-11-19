import { expect, it } from 'vitest';
import { flushSync } from 'svelte';
import { waitFor, waiting } from './waiting.svelte';

it('counts what is on its way, and an effect that asks does not ask again when the count goes down', async () => {
  let runs = 0;
  let done!: () => void;
  const cleanup = $effect.root(() => {
    $effect(() => {
      runs++;
      void waitFor(new Promise<void>((r) => (done = r)));
    });
  });
  flushSync();
  expect(waiting.count).toBe(1);
  done();
  await Promise.resolve();
  await Promise.resolve();
  flushSync();
  expect(waiting.count).toBe(0);
  expect(runs).toBe(1);
  cleanup();
});
