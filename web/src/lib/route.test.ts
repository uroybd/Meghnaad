import { describe, expect, it } from 'vitest';
import {
  PLAIN_PAGES,
  SETTINGS_TABS,
  boardFromPath,
  boardPath,
  isBoardsPath,
  parsePath,
  placePath,
  prefixFilter,
  prefixFromPath,
  taskLink,
  taskPath,
  type Place,
} from './route';

const uuid = '1a2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d';

describe('a task’s address', () => {
  it('is the first 8 characters of its uuid', () => {
    expect(taskPath(uuid)).toBe('/task/1a2b3c4d');
    expect(taskLink(uuid, 'https://tasks.example.com')).toBe('https://tasks.example.com/task/1a2b3c4d');
  });

  it('is read back, from the short form or a longer prefix', () => {
    expect(prefixFromPath('/task/1a2b3c4d')).toBe('1a2b3c4d');
    expect(prefixFromPath('/task/1A2B3C4D/')).toBe('1a2b3c4d');
    expect(prefixFromPath(`/task/${uuid}`)).toBe(uuid);
    expect(prefixFromPath('/task/12345678')).toBe('12345678');
  });

  it('is not read from anything else', () => {
    for (const p of [
      '/',
      '/task',
      '/task/',
      '/task/zzzzzzzz',
      '/task/1a2b3c',
      '/tasks/1a2b3c4d',
      '/task/1a2b3c4d/edit',
      '/api/task/1a2b3c4d',
    ]) {
      expect(prefixFromPath(p), p).toBeNull();
    }
  });

  it('is looked up with an attribute filter so digits-only prefixes are not task numbers', () => {
    expect(prefixFilter('12345678')).toBe('uuid.startswith:12345678');
  });
});

describe('a board’s address', () => {
  it('is its slug, and the list of boards is /boards', () => {
    expect(boardPath('weekly')).toBe('/board/weekly');
    expect(boardFromPath('/board/weekly')).toBe('weekly');
    expect(boardFromPath('/board/q4-review_2/')).toBe('q4-review_2');
    expect(isBoardsPath('/boards')).toBe(true);
    expect(isBoardsPath('/boards/')).toBe(true);
  });

  it('is not read from anything else', () => {
    for (const p of [
      '/board',
      '/board/',
      '/board/Weekly',
      '/board/a b',
      '/board/a/b',
      '/boards/x',
      `/board/${'a'.repeat(41)}`,
    ])
      expect(boardFromPath(p), p).toBeNull();
    expect(isBoardsPath('/boardsx')).toBe(false);
  });
});

describe('every page has an address', () => {
  const places: Place[] = [
    { page: 'tasks', report: 'next' },
    { page: 'tasks', report: '_reviewed' },
    { page: 'tasks', report: 'my report/2 é' },
    ...PLAIN_PAGES.map((page) => ({ page }) as Place),
    { page: 'boards', board: null },
    { page: 'boards', board: 'weekly' },
    { page: 'settings', tab: null },
    ...SETTINGS_TABS.map((tab) => ({ page: 'settings', tab }) as Place),
    { page: 'settings', tab: 'taskrc', file: 'taskrc' },
    { page: 'settings', tab: 'taskrc', file: 'holidays/en-US.rc' },
    { page: 'settings', tab: 'taskrc', file: 'my notes/a b%.rc' },
    { page: 'task', prefix: '1a2b3c4d' },
  ];

  it('is written and read back, for each page', () => {
    for (const p of places) expect(parsePath(placePath(p)), placePath(p)).toEqual(p);
  });

  it('reads like this', () => {
    expect(placePath({ page: 'tasks', report: 'next' })).toBe('/report/next');
    expect(placePath({ page: 'tasks', report: 'a b/c' })).toBe('/report/a%20b%2Fc');
    expect(placePath({ page: 'calendar' })).toBe('/calendar');
    expect(placePath({ page: 'settings', tab: 'hooks' })).toBe('/settings/hooks');
    expect(parsePath('/report/list/')).toEqual({ page: 'tasks', report: 'list' });
    expect(parsePath('/settings/')).toEqual({ page: 'settings', tab: null });
    expect(placePath({ page: 'settings', tab: 'taskrc', file: 'holidays/en-US.rc' })).toBe(
      '/settings/taskrc/holidays/en-US.rc',
    );
    expect(placePath({ page: 'settings', tab: 'taskrc', file: 'my notes.rc' })).toBe('/settings/taskrc/my%20notes.rc');
    expect(parsePath('/settings/taskrc/holidays/en-US.rc/')).toEqual({
      page: 'settings',
      tab: 'taskrc',
      file: 'holidays/en-US.rc',
    });
  });

  it('is nothing for the front page and for what is not a page', () => {
    for (const p of [
      '/',
      '',
      '/nope',
      '/report',
      '/report/',
      '/report/%E0%A4%A',
      '/settings/nope',
      '/settings/hooks/x',
      '/settings/taskrc/../x',
      '/settings/taskrc/a//b',
      '/settings/taskrc/a/./b',
      '/settings/taskrc/%2E%2E/x',
      '/settings/taskrc/%E0%A4%A',
      '/projects/x',
      '/Projects',
      '/api/config',
      '/assets/app.js',
    ])
      expect(parsePath(p), p).toBeNull();
  });
});
