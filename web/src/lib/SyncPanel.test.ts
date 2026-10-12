// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { mount, tick, unmount } from 'svelte';

const plugin = vi.hoisted(() => ({
  getConnection: vi.fn(),
  clearConnection: vi.fn(),
  setConnection: vi.fn(),
  send: vi.fn(),
}));
vi.mock('../engine/native', () => ({ plugin }));
const engine = vi.hoisted(() => ({ sync: vi.fn(), lastSync: null as number | null, onChanged: vi.fn(() => () => {}) }));
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
});
