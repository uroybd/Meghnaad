// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import { getConfig, putUrgency } from './api';
import UrgencyPanel from './UrgencyPanel.svelte';
import { store } from './store.svelte';
import type { ConfigResponse } from './types';

vi.mock('./api', async (original) => ({
  ...(await original<typeof import('./api')>()),
  getConfig: vi.fn(),
  putUrgency: vi.fn(),
}));

function config(inherit: boolean): ConfigResponse {
  return {
    config: { udas: {}, reports: {}, contexts: {}, active_context: null, urgency: {}, settings: {} },
    reports: [],
    urgency_defaults: {},
    urgency_inherit: inherit,
  } as unknown as ConfigResponse;
}

const tick = async () => {
  for (let i = 0; i < 5; i++) await Promise.resolve();
  flushSync();
};

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

it('takes the settings the save answered with, and says what an included file still sets', async () => {
  store.config = config(false);
  // An included file still turns inheriting off: the save answers with what is in force.
  vi.mocked(putUrgency).mockResolvedValue({ ok: true, settings: config(false), overridden: ['urgency.inherit'] });
  app = mount(UrgencyPanel, { target: document.body });
  flushSync();
  const box = document.querySelector<HTMLInputElement>('input[type="checkbox"]')!;
  box.click();
  flushSync();
  [...document.querySelectorAll('button')].find((b) => b.textContent === 'Apply')!.click();
  await tick();
  expect(putUrgency).toHaveBeenCalledWith({}, true);
  expect(getConfig).not.toHaveBeenCalled();
  await tick();
  const said = document.querySelector('[role="status"]')!.textContent!;
  expect(said).toContain('an included taskrc file still sets urgency.inherit');
  expect(box.checked).toBe(false);
});
