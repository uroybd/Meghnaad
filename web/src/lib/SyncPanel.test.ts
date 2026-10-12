// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { mount, tick, unmount } from 'svelte';

const plugin = vi.hoisted(() => ({
  getConnection: vi.fn(),
  getSyncOptions: vi.fn(),
  setSyncOptions: vi.fn(),
  clearConnection: vi.fn(),
  setConnection: vi.fn(),
  send: vi.fn(),
}));
vi.mock('../engine/native', () => ({ plugin }));
const engine = vi.hoisted(() => ({
  sync: vi.fn(),
  lastSync: null as number | null,
  onChanged: vi.fn(() => () => {}),
  onSettingsChanged: vi.fn(() => () => {}),
}));
vi.mock('../engine/client.svelte', () => ({ engine }));

import SyncPanel from './SyncPanel.svelte';
import { store } from './store.svelte';

let app: ReturnType<typeof mount> | null = null;
const button = (label: string) =>
  [...document.querySelectorAll('button')].find((b) => b.textContent?.trim() === label) as
    HTMLButtonElement | undefined;

beforeEach(() => {
  vi.spyOn(store, 'notify').mockImplementation(() => {});
  engine.sync.mockResolvedValue(undefined);
  plugin.clearConnection.mockResolvedValue(undefined);
  plugin.getSyncOptions.mockResolvedValue({ settings: false, hooks: false });
  plugin.setSyncOptions.mockResolvedValue(undefined);
});

afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  vi.clearAllMocks();
  vi.restoreAllMocks();
});

describe('SyncPanel', () => {
  it('says syncing is off and offers the form when the phone is not connected', async () => {
    plugin.getConnection.mockResolvedValue({ configured: false, localOnly: true });
    app = mount(SyncPanel, { target: document.body });
    await vi.waitFor(() => expect(document.body.textContent).toContain('Syncing is off'));
    expect(document.querySelector('form')).not.toBeNull();
    expect(button('Sync now')).toBeUndefined();
  });

  it('says syncing is on without showing anything of the bucket, and syncs on request', async () => {
    plugin.getConnection.mockResolvedValue({ configured: true, localOnly: false });
    app = mount(SyncPanel, { target: document.body });
    await vi.waitFor(() => expect(document.body.textContent).toContain('Syncing is on'));
    expect(document.body.textContent).not.toMatch(/https?:|\.com|AK|bucket:/);
    expect(document.querySelector('form')).toBeNull();
    button('Sync now')!.click();
    await vi.waitFor(() => expect(engine.sync).toHaveBeenCalledWith(true));
    await vi.waitFor(() => expect(store.notify).toHaveBeenCalledWith('Synced.', 'ok'));
  });

  it('stops syncing after a confirmation, and keeps the tasks', async () => {
    plugin.getConnection.mockResolvedValueOnce({
      configured: true,
      localOnly: false,
      bucket: 'tasks',
      endpoint: 'https://x',
      accessKey: 'AK',
    });
    app = mount(SyncPanel, { target: document.body });
    await vi.waitFor(() => expect(button('Stop syncing')).toBeDefined());

    vi.spyOn(window, 'confirm').mockReturnValueOnce(false);
    button('Stop syncing')!.click();
    await tick();
    expect(plugin.clearConnection).not.toHaveBeenCalled();

    vi.spyOn(window, 'confirm').mockReturnValueOnce(true);
    plugin.getConnection.mockResolvedValueOnce({ configured: false, localOnly: true });
    button('Stop syncing')!.click();
    await vi.waitFor(() => expect(plugin.clearConnection).toHaveBeenCalled());
    await vi.waitFor(() => expect(document.body.textContent).toContain('Syncing is off'));
  });

  it('keeps the settings and the hooks in step with the bucket when asked to, and syncs at once', async () => {
    plugin.getConnection.mockResolvedValue({ configured: true, localOnly: false });
    plugin.getSyncOptions.mockResolvedValue({ settings: false, hooks: true });
    app = mount(SyncPanel, { target: document.body });
    await vi.waitFor(() => expect(document.body.textContent).toContain('Syncing is on'));
    const box = (name: string) =>
      [...document.querySelectorAll('fieldset input')].find((i) =>
        i.closest('label')?.textContent?.includes(name),
      ) as HTMLInputElement;
    await vi.waitFor(() => expect(box('Hooks').checked).toBe(true));
    expect(box('Settings').checked).toBe(false);
    expect(document.body.textContent?.replace(/\s+/g, ' ')).toContain('secrets for hooks are never synced');

    box('Settings').click();
    await vi.waitFor(() => expect(plugin.setSyncOptions).toHaveBeenCalledWith({ settings: true, hooks: true }));
    await vi.waitFor(() => expect(engine.sync).toHaveBeenCalledWith(true));

    // Turning one off while the other stays on syncs again; turning the last off does not.
    engine.sync.mockClear();
    box('Settings').click();
    box('Hooks').click();
    await vi.waitFor(() => expect(plugin.setSyncOptions).toHaveBeenLastCalledWith({ settings: false, hooks: false }));
    await vi.waitFor(() => expect(engine.sync).toHaveBeenCalledTimes(1));
    await tick();
    expect(engine.sync).toHaveBeenCalledTimes(1);
  });
});
