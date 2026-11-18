// @vitest-environment jsdom
// Settings → Device in the static flavour: trusting the device (spec 0018) and its passkey (spec 0019).
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, tick, unmount } from 'svelte';

const session = vi.hoisted(() => ({
  connectedTo: vi.fn(),
  DEFAULT_IDLE_MINUTES: 30,
  forget: vi.fn(),
  idleMinutes: vi.fn(),
  isTrusted: vi.fn(),
  lock: vi.fn(),
  passkeyKind: vi.fn(),
  passkeyPossible: vi.fn(),
  removePasskey: vi.fn(),
  setIdleMinutes: vi.fn(),
  setTrusted: vi.fn(),
  setUpPasskey: vi.fn(),
}));
vi.mock('../engine/session', () => session);

import DevicePanel from './DevicePanel.svelte';

let app: ReturnType<typeof mount> | null = null;

async function open() {
  app = mount(DevicePanel, { target: document.body });
  await vi.waitFor(() => expect(session.passkeyPossible).toHaveBeenCalled());
  await tick();
  flushSync();
}

const field = (label: string) =>
  [...document.querySelectorAll('label')].find((l) => l.textContent?.trim().startsWith(label))!.querySelector('input')!;
const button = (label: string) => [...document.querySelectorAll('button')].find((b) => b.textContent?.trim() === label);
const text = () => document.body.textContent ?? '';

beforeEach(() => {
  for (const f of Object.values(session)) if (typeof f === 'function') f.mockReset();
  session.connectedTo.mockReturnValue({ repo: 'alice/tasks', branch: '', api: '' });
  session.idleMinutes.mockReturnValue(30);
  session.isTrusted.mockResolvedValue(false);
  session.passkeyKind.mockResolvedValue(undefined);
  session.passkeyPossible.mockResolvedValue(true);
  for (const f of [session.setTrusted, session.setUpPasskey, session.removePasskey]) f.mockResolvedValue(undefined);
});

afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

describe('this device', () => {
  it('not trusted: the switch is off, the idle lock applies, and a passkey can be set up', async () => {
    await open();
    expect(field('Trust this device').checked).toBe(false);
    expect(field('Lock after').disabled).toBe(false);
    expect(text()).toContain('Anyone who can use this browser can open your tasks.');
    button('Unlock with a passkey on this device')!.click();
    await vi.waitFor(() => expect(session.setUpPasskey).toHaveBeenCalled());
  });

  it('trusted: the idle lock is shown as off', async () => {
    session.isTrusted.mockResolvedValue(true);
    await open();
    expect(field('Trust this device').checked).toBe(true);
    expect(field('Lock after').disabled).toBe(true);
    expect(text()).toContain('Off while this device is trusted.');
  });

  it('turns trust on and off', async () => {
    await open();
    field('Trust this device').click();
    await vi.waitFor(() => expect(session.setTrusted).toHaveBeenCalledWith(true));
  });

  it('with a passkey, says where it is kept, and can stop using it', async () => {
    session.passkeyKind.mockResolvedValue('prf');
    await open();
    expect(text()).toContain('A passkey unlocks this device.');
    expect(text()).toContain('delete it there');
    expect(button('Unlock with a passkey on this device')).toBeUndefined();
    button('Stop unlocking with a passkey')!.click();
    await vi.waitFor(() => expect(session.removePasskey).toHaveBeenCalled());
  });

  it('offers no passkey where it cannot be set up', async () => {
    session.passkeyPossible.mockResolvedValue(false);
    await open();
    expect(button('Unlock with a passkey on this device')).toBeUndefined();
  });
});
