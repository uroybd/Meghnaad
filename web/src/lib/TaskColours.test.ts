// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import { getTaskrcText, putRcFile } from './api';
import { store } from './store.svelte';
import TaskColours from './TaskColours.svelte';
import type { TaskrcResponse } from './types';

vi.mock('./api', async (original) => ({
  ...(await original<typeof import('./api')>()),
  getTaskrcText: vi.fn(),
  putRcFile: vi.fn(),
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
