import { beforeEach, describe, expect, it, vi } from 'vitest';
import { runCli } from './api';
vi.mock('./api', async (original) => ({ ...(await original<typeof import('./api')>()), runCli: vi.fn() }));

import { reportArgs } from './cmdline';
import { HISTORY_MAX, shortenUuids, store, type Entry } from './store.svelte';

beforeEach(() => {
  store.history = [];
  store.lastCommand = '';
  localStorage.clear();
});

describe('remember', () => {
  it('shows the last command with a task prefix, and keeps history without it', () => {
    store.remember('project:Home list');
    expect(store.lastCommand).toBe('task project:Home list');
    expect(store.history).toEqual(['project:Home list']);
    store.remember('task done');
    expect(store.history).toEqual(['project:Home list', 'done']);
  });

  it('keeps only the last 20, newest last', () => {
    for (let i = 0; i < 30; i++) store.remember(`cmd ${i}`);
    expect(store.history).toHaveLength(HISTORY_MAX);
    expect(store.history[0]).toBe('cmd 10');
    expect(store.history.at(-1)).toBe('cmd 29');
  });

  it('moves a repeated command to the end instead of duplicating it', () => {
    store.remember('a');
    store.remember('b');
    store.remember('a');
    expect(store.history).toEqual(['b', 'a']);
  });

  it('live-table updates replace each other so typing a filter does not flood history', () => {
    store.remember('add milk'); // a real command
    for (const f of ['p', 'pr', 'proj', 'project:Home work']) store.remember(f, true);
    expect(store.history).toEqual(['add milk', 'project:Home work']);
    store.remember('1 done'); // a real command is kept, and is not replaced by what follows
    store.remember('list', true);
    expect(store.history).toEqual(['add milk', 'project:Home work', '1 done', 'list']);
    store.remember('next', true);
    expect(store.history).toEqual(['add milk', 'project:Home work', '1 done', 'next']);
  });

  it('persists to localStorage', () => {
    store.remember('persist me');
    expect(JSON.parse(localStorage.getItem('tw-web-history')!)).toEqual(['persist me']);
  });

  it('ignores blanks', () => {
    store.remember('   ');
    expect(store.history).toEqual([]);
  });
});

describe('shortenUuids', () => {
  it('shortens full uuids for display only', () => {
    expect(shortenUuids('task 074a5bbe-86f4-4fae-b6cd-cb118064ef58 done')).toBe('task 074a5bbe done');
    expect(shortenUuids('task 12345678 done')).toBe('task 12345678 done');
  });
});

describe('focusReport', () => {
  const key = (filter: string, report: string) => JSON.stringify(reportArgs(filter, report));

  it('leaves the table to be run when no result is handed over', () => {
    // The Tasks view skips a run whose arguments match `liveKey`, so a filter that was never run
    // must not be recorded there (clicking a calendar day showed the unfiltered table).
    store.liveKey = key('', 'next');
    store.focusReport('next', ['due:2026-10-12']);
    expect(store.view).toBe('tasks');
    expect(store.filter).toBe('due:2026-10-12');
    expect(store.liveKey).not.toBe(key('due:2026-10-12', 'next'));
  });

  it('marks the handed-over result as the one on screen', () => {
    const from = {
      id: 1,
      input: { line: 'x' },
      title: 'x',
      result: null,
      loading: false,
      failure: null,
      at: 0,
    } as Entry;
    store.focusReport('next', ['project:Home'], from);
    expect(store.liveKey).toBe(key('project:Home', 'next'));
    expect(store.live?.input).toEqual({ args: reportArgs('project:Home', 'next') });
  });
});

describe('running a report', () => {
  const report = {
    result: { kind: 'report', report: 'list', rows: [], columns: [], matched: 0, sort: null, breaks: [] },
    command: { name: 'list', report: true, filter: ['project:Home'] },
    wrote: false,
  };

  beforeEach(() => {
    store.entries = [];
    store.live = null;
    vi.mocked(runCli).mockResolvedValue(report as never);
  });

  it('prints the table in the Console when typed there', async () => {
    store.view = 'console';
    const e = await store.run({ line: 'project:Home list' });
    expect(store.view).toBe('console');
    expect(store.entries.map((x) => x.id)).toEqual([e.id]);
    expect(e.result?.kind).toBe('report');
    expect(store.live).toBeNull();
  });

  it('opens it in the Tasks view when typed anywhere else', async () => {
    store.view = 'calendar';
    const e = await store.run({ line: 'project:Home list' });
    expect(store.view).toBe('tasks');
    expect(store.entries.find((x) => x.id === e.id)).toBeUndefined();
    expect(store.report).toBe('list');
    expect(store.filter).toBe('project:Home');
    expect(store.live?.result?.kind).toBe('report');
  });
});
