// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { mount, tick, unmount } from 'svelte';

const plugin = vi.hoisted(() => ({
  setConnection: vi.fn(),
  send: vi.fn(),
  clearConnection: vi.fn(),
}));
vi.mock('../engine/native', () => ({ plugin }));

import ConnectView from './ConnectView.svelte';

let app: ReturnType<typeof mount> | null = null;

afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  vi.clearAllMocks();
});

const field = (label: string) =>
  [...document.querySelectorAll('label')]
    .find((l) => l.textContent?.trim().startsWith(label))!
    .querySelector('input') as HTMLInputElement;
const fill = (label: string, value: string) => {
  const el = field(label);
  el.value = value;
  el.dispatchEvent(new Event('input', { bubbles: true }));
};
const connect = () => document.querySelector('button[type=submit]') as HTMLButtonElement;

async function fillAll() {
  fill('S3 endpoint', 'https://acct.r2.cloudflarestorage.com');
  fill('Bucket', 'tasks');
  fill('Access key ID', 'AK');
  fill('Secret access key', 'SK');
  fill('Encryption secret', 'passphrase');
  await tick();
}

describe('ConnectView', () => {
  it('asks for the keys as passwords and needs every field but the region', async () => {
    app = mount(ConnectView, { target: document.body, props: { onready: () => {} } });
    expect(field('Secret access key').type).toBe('password');
    expect(field('Encryption secret').type).toBe('password');
    expect(field('Region').value).toBe('auto');
    expect(connect().disabled).toBe(true);
    await fillAll();
    expect(connect().disabled).toBe(false);
  });

  it('keeps the connection, tries it with a first sync, and goes on', async () => {
    plugin.setConnection.mockResolvedValue(undefined);
    plugin.send.mockResolvedValue({ value: 'true' });
    const ready = vi.fn();
    app = mount(ConnectView, { target: document.body, props: { onready: ready } });
    await fillAll();
    connect().click();
    await vi.waitFor(() => expect(ready).toHaveBeenCalled());
    expect(plugin.setConnection).toHaveBeenCalledWith({
      endpoint: 'https://acct.r2.cloudflarestorage.com',
      bucket: 'tasks',
      region: 'auto',
      accessKey: 'AK',
      secretKey: 'SK',
      encryptionSecret: 'passphrase',
    });
    expect(plugin.send).toHaveBeenCalledWith({ op: 'sync' });
    expect(plugin.clearConnection).not.toHaveBeenCalled();
  });

  it('says what went wrong and forgets the keys when the first sync fails', async () => {
    plugin.setConnection.mockResolvedValue(undefined);
    plugin.send.mockRejectedValue(new Error('Server Error: could not reach the bucket'));
    plugin.clearConnection.mockResolvedValue(undefined);
    const ready = vi.fn();
    app = mount(ConnectView, { target: document.body, props: { onready: ready } });
    await fillAll();
    connect().click();
    await vi.waitFor(() =>
      expect(document.querySelector('[role=alert]')?.textContent).toContain('could not reach the bucket'),
    );
    expect(document.querySelector('[role=alert]')?.textContent).not.toContain('Server Error');
    expect(plugin.clearConnection).toHaveBeenCalled();
    expect(ready).not.toHaveBeenCalled();
    // The form is still filled in, so it can be corrected.
    expect(field('Bucket').value).toBe('tasks');
  });
});
