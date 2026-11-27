// @vitest-environment jsdom
// Settings → This device (spec 0033): the AI agents switch waits for its Apply, as the page's other settings do.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { mount, tick, unmount } from 'svelte';

const api = vi.hoisted(() => ({
  putAppSettings: vi.fn(),
  getAppSettings: vi.fn(),
  putDeviceSettings: vi.fn(),
  getDeviceSettings: vi.fn(),
  listDevices: vi.fn(),
}));
vi.mock('./api', async (original) => ({ ...(await original<typeof import('./api')>()), ...api }));

import { parseSettings } from './appsettings';
import ThisDevicePanel from './ThisDevicePanel.svelte';
import { store } from './store.svelte';

let app: ReturnType<typeof mount> | null = null;

const agents = () =>
  [...document.querySelectorAll('label.pick')]
    .find((l) => /AI agents/.test(l.textContent ?? ''))
    ?.querySelector('input') as HTMLInputElement;
const applies = () => [...document.querySelectorAll('button')].filter((b) => b.textContent?.trim() === 'Apply');

beforeEach(() => {
  api.putDeviceSettings.mockResolvedValue({ ok: true });
  api.listDevices.mockResolvedValue([]);
  store.appSettings = parseSettings(JSON.stringify({ version: 7 }));
  store.keymapDefaults = null;
  store.deviceId = 'laptop';
  store.deviceRaw = { deviceId: 'laptop' };
});
afterEach(() => {
  if (app) unmount(app);
  app = null;
  store.deviceRaw = null;
  store.deviceId = null;
  vi.clearAllMocks();
});

describe('AI agents in This device', () => {
  it('is saved by its Apply, not as it is switched', async () => {
    const dirty: boolean[] = [];
    app = mount(ThisDevicePanel, { target: document.body, props: { ondirty: (v: boolean) => dirty.push(v) } });
    await tick();
    expect(agents().checked).toBe(false);
    agents().click();
    await tick();
    expect(api.putDeviceSettings).not.toHaveBeenCalled();
    expect(dirty.at(-1)).toBe(true);

    const apply = applies().find((b) => !b.disabled);
    expect(apply).toBeDefined();
    apply?.click();
    await vi.waitFor(() => expect(api.putDeviceSettings).toHaveBeenCalled());
    const [id, text] = api.putDeviceSettings.mock.calls[0];
    expect(id).toBe('laptop');
    expect(JSON.parse(text).web).toEqual({ agents: true });
  });
});
