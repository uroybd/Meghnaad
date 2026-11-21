import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { parseSettings } from './appsettings';
import { store } from './store.svelte';
import type { ConfigResponse } from './types';

const settings = (homepage: unknown, boards: unknown[] = []) =>
  parseSettings(JSON.stringify({ homepage, boardsVersion: 1, boards }));
const board = { id: 'week', name: 'Week', sections: [] };

beforeEach(() => {
  store.config = {
    reports: [{ name: 'next' }, { name: 'list' }],
    config: { settings: {} },
  } as unknown as ConfigResponse;
  store.view = 'tasks';
  store.report = 'next';
  store.boardId = null;
  store.detail = null;
});
afterEach(() => {
  store.appSettings = null;
});

describe('opening on the home page', () => {
  it('stays on next when nothing is chosen', () => {
    store.appSettings = settings(undefined);
    store.goHome();
    expect([store.view, store.report]).toEqual(['tasks', 'next']);
  });

  it('opens a report', () => {
    store.appSettings = settings({ report: 'list' });
    store.goHome();
    expect([store.view, store.report]).toEqual(['tasks', 'list']);
  });

  it('opens a board', () => {
    store.appSettings = settings({ board: 'week' }, [board]);
    store.goHome();
    expect([store.view, store.boardId]).toEqual(['boards', 'week']);
  });

  it('opens a kanban, while Kanban is on', () => {
    const kanban = { enabled: true, kanbans: [{ id: 'work', name: 'Work' }] };
    store.appSettings = parseSettings(JSON.stringify({ homepage: { kanban: 'work' }, kanban }));
    store.goHome();
    expect([store.view, store.kanbanId]).toEqual(['kanban', 'work']);
    // Off, or gone: next.
    store.view = 'tasks';
    store.kanbanId = null;
    store.appSettings = parseSettings(
      JSON.stringify({ homepage: { kanban: 'work' }, kanban: { ...kanban, enabled: false } }),
    );
    store.goHome();
    expect(store.view).toBe('tasks');
    store.appSettings = parseSettings(JSON.stringify({ homepage: { kanban: 'gone' }, kanban }));
    store.goHome();
    expect(store.view).toBe('tasks');
  });

  it('stays on next when the report or the board is gone', () => {
    store.appSettings = settings({ report: 'nope' });
    store.goHome();
    expect([store.view, store.report]).toEqual(['tasks', 'next']);
    store.appSettings = settings({ board: 'nope' }, [board]);
    store.goHome();
    expect(store.view).toBe('tasks');
  });

  it('goes home from anywhere when asked to, and leaves a task’s drawer', () => {
    store.appSettings = settings({ board: 'week' }, [board]);
    store.view = 'summary';
    store.detail = { uuid: 'x', from: null };
    store.goHome(true);
    expect([store.view, store.boardId, store.detail]).toEqual(['boards', 'week', null]);
    // With nothing chosen it is next.
    store.appSettings = settings(undefined);
    store.view = 'tags';
    store.report = 'list';
    store.goHome(true);
    expect([store.view, store.report]).toEqual(['tasks', 'next']);
  });

  it('leaves a person who has already gone somewhere where they are', () => {
    store.appSettings = settings({ report: 'list' });
    store.view = 'tags';
    store.goHome();
    expect([store.view, store.report]).toEqual(['tags', 'next']);
    store.view = 'tasks';
    store.detail = { uuid: 'x', from: null };
    store.goHome();
    expect(store.report).toBe('next');
  });
});
