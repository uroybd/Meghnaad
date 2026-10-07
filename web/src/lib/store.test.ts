import { beforeEach, describe, expect, it } from 'vitest';
import { HISTORY_MAX, shortenUuids, store } from './store.svelte';

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
