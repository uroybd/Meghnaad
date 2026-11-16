// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, tick, unmount } from 'svelte';

const api = vi.hoisted(() => ({ putAppSettings: vi.fn(), getAppSettings: vi.fn() }));
vi.mock('./api', async (original) => ({ ...(await original<typeof import('./api')>()), ...api }));

import { parseSettings } from './appsettings';
import GeneralPanel from './GeneralPanel.svelte';
import { store } from './store.svelte';

let app: ReturnType<typeof mount> | null = null;

async function open(settings: Record<string, unknown>) {
  store.appSettings = parseSettings(JSON.stringify(settings));
  store.keymapDefaults = null;
  app = mount(GeneralPanel, { target: document.body });
  await tick();
}

const box = () => document.querySelector('textarea[aria-label="settings.json"]') as HTMLTextAreaElement;
const button = (label: string) =>
  [...document.querySelectorAll('details.json button')].find(
    (b) => b.textContent?.trim() === label,
  ) as HTMLButtonElement;
const lint = () => [...document.querySelectorAll('.lint li')].map((l) => l.textContent?.trim());
function paste(text: string) {
  box().value = text;
  box().dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
}

beforeEach(() => {
  api.putAppSettings.mockResolvedValue({ ok: true });
  api.getAppSettings.mockImplementation(async () => JSON.parse(api.putAppSettings.mock.calls.at(-1)?.[0] ?? '{}'));
});
afterEach(() => {
  if (app) unmount(app);
  app = null;
  vi.clearAllMocks();
});

describe('settings.json on the General tab', () => {
  it('is the whole file, saved as written once there is no error', async () => {
    await open({ homepage: { report: 'list' }, future: 1 });
    expect(JSON.parse(box().value)).toEqual({ homepage: { report: 'list' }, future: 1 });
    expect(button('Apply').disabled).toBe(true);

    paste('{ "boards": {} }');
    expect(lint()).toEqual([expect.stringContaining('boards must be a list')]);
    expect(button('Apply').disabled).toBe(true);

    button('Reset').click();
    flushSync();
    expect(JSON.parse(box().value)).toEqual({ homepage: { report: 'list' }, future: 1 });

    const pasted = '{"taskCommands": [{ "name": "two words", "command": "done" }], "mine": true}';
    paste(pasted);
    expect(lint()).toEqual([expect.stringMatching(/two words.*one word.*\(kept\)/)]);
    expect(button('Apply').disabled).toBe(false);
    button('Apply').click();
    await vi.waitFor(() => expect(api.putAppSettings).toHaveBeenCalledWith(pasted));
  });
});
