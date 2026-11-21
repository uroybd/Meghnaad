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
  store.settingsDevice = device;
  app = mount(GeneralPanel, { target: document.body });
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
  store.settingsDevice = false;
  store.deviceRaw = null;
  store.deviceId = null;
  vi.clearAllMocks();
});

describe('the home page under This device', () => {
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
    expect(document.querySelector('a[href="/settings/device/general"]')).not.toBeNull();
  });
});

describe('the Settings page under This device', () => {
  it('shows what a device can’t have of its own read only, and its own tabs as they are', async () => {
    const { default: SettingsPage } = await import('./SettingsPage.svelte');
    store.settingsTab = 'appearance';
    store.settingsDevice = true;
    app = mount(SettingsPage, { target: document.body });
    await tick();
    const set = (tab: string) => document.querySelector(`#settings-panel-${tab} fieldset`) as HTMLFieldSetElement;
    expect(set('appearance').disabled).toBe(true);
    expect(set('general').disabled).toBe(false);
    expect(set('keys').disabled).toBe(false);
    expect(document.querySelector('#settings-panel-appearance .shared')?.textContent).toContain('Everyone');
    // Under Everyone, nothing is read only.
    store.settingsDevice = false;
    await tick();
    expect(set('appearance').disabled).toBe(false);
  });
});
