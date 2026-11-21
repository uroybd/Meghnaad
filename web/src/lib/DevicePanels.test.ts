// @vitest-environment jsdom
// This device's own settings on the Settings screens (spec 0027): the home page framed when it is the device's, put
// back to everyone's, a note under Everyone, and the tabs a device can't have of its own shown read only.
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
import GeneralPanel from './GeneralPanel.svelte';
import { store } from './store.svelte';

let app: ReturnType<typeof mount> | null = null;

async function open(device: boolean, own: Record<string, unknown> | null) {
  store.appSettings = parseSettings(JSON.stringify({ version: 2, homepage: { report: 'list' } }));
  store.keymapDefaults = null;
  store.deviceId = own ? 'phone' : null;
  store.deviceRaw = own;
  app = mount(GeneralPanel, { target: document.body, props: { device } });
  await tick();
}

const field = () => document.querySelector('.own') as HTMLElement;
const select = () => field().querySelector('select') as HTMLSelectElement;
const button = (label: RegExp) =>
  [...document.querySelectorAll('button')].find((b) => label.test(b.textContent ?? '')) as HTMLButtonElement;

beforeEach(() => {
  api.putDeviceSettings.mockResolvedValue({ ok: true });
  api.listDevices.mockResolvedValue([]);
});
afterEach(() => {
  if (app) unmount(app);
  app = null;
  store.deviceRaw = null;
  store.deviceId = null;
  vi.clearAllMocks();
});

describe('the home page in This device', () => {
  it('is framed when it is the device’s own, and put back to everyone’s', async () => {
    await open(true, { deviceId: 'phone', homepage: { report: 'next' }, reminders: { enabled: true } });
    expect(field().classList.contains('mine')).toBe(true);
    expect(select().value).toBe('report:next');

    button(/Use everyone/).click();
    await vi.waitFor(() => expect(api.putDeviceSettings).toHaveBeenCalled());
    const [id, text] = api.putDeviceSettings.mock.calls[0];
    expect(id).toBe('phone');
    expect(JSON.parse(text)).toEqual({ deviceId: 'phone', version: 2, reminders: { enabled: true } });
    await vi.waitFor(() => expect(field().classList.contains('mine')).toBe(false));
  });

  it('is everyone’s, not framed, while the device has nothing of its own', async () => {
    await open(true, null);
    expect(field().classList.contains('mine')).toBe(false);
    expect(select().value).toBe('report:list');
    expect(document.body.textContent).toContain('no settings of its own yet');
  });

  it('says so under Everyone when this device uses its own', async () => {
    await open(false, { deviceId: 'phone', homepage: { board: 'week' } });
    expect(select().value).toBe('report:list');
    expect(document.body.textContent).toContain('This device uses its own');
    expect(document.querySelector('a[href="/settings/device"]')).not.toBeNull();
  });
});

describe('Settings → This device (spec 0028)', () => {
  it('is a tab of its own, and no other tab is read only', async () => {
    const { default: SettingsPage } = await import('./SettingsPage.svelte');
    store.settingsTab = 'device';
    app = mount(SettingsPage, { target: document.body });
    await vi.waitFor(() =>
      expect(document.querySelector('#settings-panel-device h2')?.textContent).toBe('This device'),
    );
    expect(document.querySelector('#settings-tab-device')?.getAttribute('aria-selected')).toBe('true');
    expect(document.querySelector('fieldset:disabled')).toBeNull();
    expect(document.body.textContent).not.toContain('Everyone');
  });
});
