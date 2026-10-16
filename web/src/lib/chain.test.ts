// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { CliResponse, CliResult } from './types';

const runCli = vi.hoisted(() => vi.fn());
vi.mock('./api', async (original) => ({ ...(await original<typeof import('./api')>()), runCli }));

import { store } from './store.svelte';

const answer = (result: CliResult): CliResponse => ({ wrote: false, result, command: null });
const ok = answer({ kind: 'text', lines: ['ok'] });
const bad = answer({ kind: 'error', message: 'no' });
const ask = answer({ kind: 'confirm', ask: 'plain', message: 'Sure?', items: [] });
const steps = (...parts: [string | null, string][]) => parts.map(([join, line]) => ({ join: join as never, line }));
const lines = () => runCli.mock.calls.map(([input]) => input.line);

beforeEach(() => {
  runCli.mockReset();
  store.entries = [];
  store.view = 'console';
});
afterEach(() => {
  store.clear();
});

describe('runChain', () => {
  it('runs each statement as an entry of its own, as a shell would', async () => {
    runCli.mockImplementation(async ({ line }) => (line === 'a' ? bad : ok));
    await store.runChain(steps([null, 'a'], ['&&', 'b'], ['||', 'c'], [';', 'd']), 'a && b || c ; d');
    expect(lines()).toEqual(['a', 'c', 'd']);
    expect(store.entries.map((e) => e.title)).toEqual(['task a', 'task c', 'task d']);
    expect(store.history.at(-1)).toBe('a && b || c ; d');
    expect(store.history.filter((h) => h === 'a' || h === 'c')).toEqual([]);
  });

  it('is busy until the last statement has ended', async () => {
    let release!: () => void;
    runCli.mockImplementation(({ line }) =>
      line === 'a' ? new Promise((done) => (release = () => done(ok))) : Promise.resolve(ok),
    );
    const done = store.runChain(steps([null, 'a'], [';', 'b']), 'a ; b');
    expect(store.chainBusy).toBe(true);
    await vi.waitFor(() => expect(lines()).toEqual(['a']));
    expect(store.chainBusy).toBe(true);
    release();
    await done;
    expect(lines()).toEqual(['a', 'b']);
    expect(store.chainBusy).toBe(false);
  });

  it('takes a failure to reach the engine for a failure', async () => {
    runCli.mockImplementation(async ({ line }) => {
      if (line === 'a') throw new Error('offline');
      return ok;
    });
    await store.runChain(steps([null, 'a'], ['&&', 'b'], ['||', 'c']), 'a && b || c');
    expect(lines()).toEqual(['a', 'c']);
  });

  it('waits for the answer to a question: yes goes on', async () => {
    runCli.mockImplementation(async ({ line, confirmed }) => (line === 'del' && !confirmed ? ask : ok));
    const done = store.runChain(steps([null, 'del'], ['&&', 'after']), 'del && after');
    await vi.waitFor(() => expect(store.entries[0]?.result?.kind).toBe('confirm'));
    expect(lines()).toEqual(['del']);
    expect(store.chainBusy).toBe(true);
    await store.confirm(store.entries[0], true);
    await done;
    expect(lines()).toEqual(['del', 'del', 'after']);
    expect(store.chainBusy).toBe(false);
  });

  it('waits for the answer to a question: no is a failure', async () => {
    runCli.mockImplementation(async ({ line }) => (line === 'del' ? ask : ok));
    const done = store.runChain(steps([null, 'del'], ['&&', 'never'], ['||', 'instead']), 'del && never || instead');
    await vi.waitFor(() => expect(store.entries[0]?.result?.kind).toBe('confirm'));
    await store.confirm(store.entries[0], false);
    await done;
    expect(lines()).toEqual(['del', 'instead']);
  });

  it('ends when the Console is cleared while it waits', async () => {
    runCli.mockImplementation(async () => ask);
    const done = store.runChain(steps([null, 'del'], [';', 'after']), 'del ; after');
    await vi.waitFor(() => expect(store.entries[0]?.result?.kind).toBe('confirm'));
    store.clear();
    await done;
    expect(lines()).toEqual(['del']);
    expect(store.chainBusy).toBe(false);
  });

  it('treats `clear` as a statement that works', async () => {
    runCli.mockImplementation(async () => ok);
    await store.runChain(steps([null, 'a'], [';', 'clear'], [';', 'b']), 'a ; clear ; b');
    expect(lines()).toEqual(['a', 'b']);
    expect(store.entries.map((e) => e.title)).toEqual(['task b']);
  });
});

describe('actSteps', () => {
  it('runs the statements of a button, as a shell does', async () => {
    runCli.mockImplementation(async ({ args }) => (args[1] === 'done' ? bad : ok));
    const win = await store.actSteps(null, [
      { join: null, args: ['u', 'done'] },
      { join: '&&', args: ['u', 'modify', '+failed'] },
      { join: '||', args: ['u', 'annotate', 'gave up'] },
    ]);
    expect(runCli.mock.calls.map(([i]) => i.args[1])).toEqual(['done', 'annotate']);
    expect(win).toBe(true);
  });
});
