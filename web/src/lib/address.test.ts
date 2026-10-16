import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { parseSettings } from './appsettings';
import { placePath } from './route';
import { store } from './store.svelte';
import type { ConfigResponse } from './types';

const board = { id: 'week', name: 'Week', sections: [] };
const settings = (extra: object = {}) => parseSettings(JSON.stringify({ boardsVersion: 1, boards: [board], ...extra }));
const config = () =>
  ({
    reports: [{ name: 'next' }, { name: 'list' }, { name: '_reviewed' }],
    config: { settings: {} },
  }) as unknown as ConfigResponse;

beforeEach(() => {
  store.config = config();
  store.appSettings = settings();
  store.view = 'tasks';
  store.report = 'next';
  store.boardId = null;
  store.settingsTab = 'taskrc';
  store.detail = null;
  store.toast = null;
});
afterEach(() => {
  store.appSettings = null;
  store.config = null;
});

const go = (path: string) => store.openFromAddress(path);

describe('where the app is', () => {
  it('is named by the page it shows', () => {
    expect(placePath(store.place)).toBe('/report/next');
    store.report = 'list';
    expect(placePath(store.place)).toBe('/report/list');
    store.view = 'summary';
    expect(placePath(store.place)).toBe('/summary');
    store.view = 'boards';
    expect(placePath(store.place)).toBe('/boards');
    store.boardId = 'week';
    expect(placePath(store.place)).toBe('/board/week');
    store.view = 'settings';
    store.settingsTab = 'hooks';
    expect(placePath(store.place)).toBe('/settings/hooks');
  });

  it('is the task in the drawer, when there is one', () => {
    store.detail = { uuid: '1a2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d', from: null };
    expect(placePath(store.place)).toBe('/task/1a2b3c4d');
  });
});

describe('following an address', () => {
  it('opens a report, one of the plain pages, a board and a tab of Settings', async () => {
    await go('/report/list');
    expect([store.view, store.report]).toEqual(['tasks', 'list']);
    await go('/report/_reviewed');
    expect(store.report).toBe('_reviewed');
    await go('/calendar');
    expect(store.view).toBe('calendar');
    await go('/board/week');
    expect([store.view, store.boardId]).toEqual(['boards', 'week']);
    await go('/boards');
    expect([store.view, store.boardId]).toEqual(['boards', null]);
    await go('/settings/urgency');
    expect([store.view, store.settingsTab]).toEqual(['settings', 'urgency']);
    await go('/settings');
    expect([store.view, store.settingsTab]).toEqual(['settings', 'urgency']);
  });

  it('closes the drawer', async () => {
    store.detail = { uuid: 'x', from: null };
    await go('/summary');
    expect(store.detail).toBeNull();
  });

  it('waits for the reports and the boards when a page needs them, and does not guess', async () => {
    store.config = null;
    expect(await go('/report/list')).toBeUndefined();
    expect([store.view, store.report]).toEqual(['tasks', 'next']);
    store.appSettings = null;
    expect(await go('/board/week')).toBeUndefined();
    expect(store.view).toBe('tasks');
    // A page that needs neither is opened at once.
    await go('/tags');
    expect(store.view).toBe('tags');
  });

  it('says so when a report or a board is not there, and opens the home page', async () => {
    store.appSettings = settings({ homepage: { board: 'week' } });
    expect(await go('/report/nope')).toBe('/');
    expect(store.toast?.text).toContain('no report called nope');
    expect([store.view, store.boardId]).toEqual(['boards', 'week']);

    store.view = 'tasks';
    expect(await go('/board/gone')).toBe('/');
    expect(store.toast?.text).toContain('no board called gone');
  });

  it('opens the home page for the front page, whatever was shown, and next when there is none', async () => {
    store.view = 'tags';
    await go('/');
    expect([store.view, store.report]).toEqual(['tasks', 'next']);
    store.appSettings = settings({ homepage: { report: 'list' } });
    store.view = 'console';
    await go('/');
    expect([store.view, store.report]).toEqual(['tasks', 'list']);
    // An address that is nothing is the front page.
    store.view = 'tags';
    await go('/no-such-page');
    expect([store.view, store.report]).toEqual(['tasks', 'list']);
  });
});
