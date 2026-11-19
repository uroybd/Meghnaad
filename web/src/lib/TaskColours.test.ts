// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import { getTaskrcText, importRcFiles, putRcFile } from './api';
import { store } from './store.svelte';
import TaskColours from './TaskColours.svelte';
import type { TaskrcResponse } from './types';

vi.mock('./api', async (original) => ({
  ...(await original<typeof import('./api')>()),
  getTaskrcText: vi.fn(),
  putRcFile: vi.fn(),
  importRcFiles: vi.fn(),
}));

const saved = (text: string) => ({ text }) as TaskrcResponse;
const tick = async () => {
  for (let i = 0; i < 8; i++) await new Promise((r) => setTimeout(r, 0));
  flushSync();
};

let app: ReturnType<typeof mount> | null = null;
beforeEach(() => {
  store.config = { config: { settings: {} } } as never;
  vi.spyOn(store, 'settingsChanged').mockResolvedValue(undefined);
  vi.mocked(getTaskrcText).mockResolvedValue('bulk=3\ncolor.due=red\n');
  vi.mocked(putRcFile).mockImplementation((_p, t) => Promise.resolve(saved(t)));
});
afterEach(() => {
  if (app) unmount(app);
  app = null;
  vi.restoreAllMocks();
});

it('saves the main taskrc at once when the colours are turned off, keeping the other lines', async () => {
  app = mount(TaskColours, { target: document.body });
  await tick();
  const box = document.querySelector('input[type=checkbox]') as HTMLInputElement;
  expect(box.checked).toBe(true);
  box.click();
  await tick();
  expect(putRcFile).toHaveBeenCalledTimes(1);
  const [path, text] = vi.mocked(putRcFile).mock.calls[0];
  expect(path).toBe('taskrc');
  expect(text).toContain('bulk=3');
  expect(text).toContain('color=off');
  expect(store.settingsChanged).toHaveBeenCalled();
  expect(document.querySelector('[role=status]')?.textContent).toContain('Restore previous');
});

it('saves a theme as its own file, included from the main taskrc, in one save', async () => {
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('color.active=black on yellow\n')));
  vi.mocked(importRcFiles).mockResolvedValue({ text: '' } as TaskrcResponse);
  app = mount(TaskColours, { target: document.body });
  await tick();
  vi.mocked(getTaskrcText).mockResolvedValue('bulk=3\n\ninclude themes/light-16.rc\n');
  const pick = document.querySelector('select') as HTMLSelectElement;
  pick.value = 'light-16';
  pick.dispatchEvent(new Event('change', { bubbles: true }));
  await tick();
  expect(importRcFiles).toHaveBeenCalledWith(
    [
      { path: 'themes/light-16.rc', text: 'color.active=black on yellow\n' },
      { path: 'taskrc', text: 'bulk=3\n\ninclude themes/light-16.rc\n' },
    ],
    'merge',
  );
  expect(putRcFile).not.toHaveBeenCalled();
  expect(pick.value).toBe('light-16');
  vi.unstubAllGlobals();
});
