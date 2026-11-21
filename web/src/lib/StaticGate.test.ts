// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, tick, unmount } from 'svelte';
import { WrongPassword } from '../engine/pgp';

import { PasskeyCancelled } from '../engine/passkey';
import { SETTINGS_KEY } from './lockedReminders';

const session = vi.hoisted(() => ({
  openDevice: vi.fn(),
  connect: vi.fn(),
  forget: vi.fn(),
  unlockWithSecret: vi.fn(),
  unlockWithPasskey: vi.fn(),
  passkeyKind: vi.fn(),
}));
vi.mock('../engine/session', () => session);

import StaticGate from './StaticGate.svelte';
import { asker } from './ask.svelte';

let app: ReturnType<typeof mount> | null = null;
let unlocked = 0;

async function open(defaults = {}) {
  app = mount(StaticGate, { target: document.body, props: { defaults, onunlocked: () => unlocked++ } });
  await vi.waitFor(() => expect(document.querySelector('h1')?.textContent).toBeTruthy());
  await tick();
}

const heading = () => document.querySelector('h1')?.textContent ?? '';
const input = (label: string) =>
  [...document.querySelectorAll('label')].find((l) => l.textContent?.trim().startsWith(label))!.querySelector('input')!;
const button = (label: string) =>
  [...document.querySelectorAll('button')].find((b) => b.textContent?.trim() === label)!;
/** Tick the checkbox whose label starts with `label`. */
function tick_(label: string) {
  input(label).click();
  flushSync();
}
const alert = () => document.querySelector('[role=alert]')?.textContent?.trim() ?? '';

function type(label: string, value: string) {
  const el = input(label);
  el.value = value;
  el.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
}

async function submit(label: string) {
  button(label).click();
  await tick();
  await vi.waitFor(() => expect(button(label).disabled).toBe(false));
}

beforeEach(() => {
  unlocked = 0;
  for (const f of Object.values(session)) f.mockReset();
  session.passkeyKind.mockResolvedValue(undefined);
  localStorage.clear();
  sessionStorage.clear();
});

afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

describe('a device that was never connected', () => {
  it('asks for the branch, the token and the secret, filled from config.json', async () => {
    session.openDevice.mockResolvedValue('connect');
    await open({ repo: 'alice/tasks', branch: 'main' });
    expect(heading()).toBe('Connect this device');
    expect(input('Repository').value).toBe('alice/tasks');
    expect(input('Branch').value).toBe('main');
  });

  it('connects with what was typed, and goes on', async () => {
    session.openDevice.mockResolvedValue('connect');
    session.connect.mockResolvedValue(undefined);
    await open({ repo: 'alice/tasks' });
    type('Token', 'ghp_x');
    type('Encryption secret', 's3cret');
    await submit('Connect');
    expect(session.connect).toHaveBeenCalledWith(
      { api: '', repo: 'alice/tasks', branch: '', token: 'ghp_x', secret: 's3cret' },
      false,
    );
    expect(unlocked).toBe(1);
  });

  it('trusts the device when that is ticked', async () => {
    session.openDevice.mockResolvedValue('connect');
    session.connect.mockResolvedValue(undefined);
    await open({ repo: 'alice/tasks' });
    type('Token', 'ghp_x');
    type('Encryption secret', 's3cret');
    tick_('Trust this device');
    await submit('Connect');
    expect(session.connect.mock.calls[0][1]).toBe(true);
  });

  it('marks the token as the username and the secret as the password, for password managers', async () => {
    session.openDevice.mockResolvedValue('connect');
    await open();
    expect(input('Token').autocomplete).toBe('username');
    expect(input('Token').type).toBe('text');
    expect(input('Encryption secret').autocomplete).toBe('current-password');
    expect(input('Encryption secret').type).toBe('password');
  });

  it('asks the browser to save the token and the secret once connected, where it can', async () => {
    session.openDevice.mockResolvedValue('connect');
    session.connect.mockResolvedValue(undefined);
    const store = vi.fn().mockResolvedValue(undefined);
    class FakePasswordCredential {
      constructor(public data: { id: string; password: string }) {}
    }
    vi.stubGlobal('PasswordCredential', FakePasswordCredential);
    Object.defineProperty(navigator, 'credentials', { value: { store }, configurable: true });
    try {
      await open({ repo: 'alice/tasks' });
      type('Token', 'ghp_x');
      type('Encryption secret', 's3cret');
      await submit('Connect');
      expect(store).toHaveBeenCalledWith(new FakePasswordCredential({ id: 'ghp_x', password: 's3cret' }));
      expect(unlocked).toBe(1);
    } finally {
      vi.unstubAllGlobals();
      Reflect.deleteProperty(navigator, 'credentials');
    }
  });

  it('says why it could not connect, and stays', async () => {
    session.openDevice.mockResolvedValue('connect');
    session.connect.mockRejectedValue(new Error('The tasks on the branch could not be opened: decryption failed.'));
    await open({ repo: 'alice/tasks' });
    type('Token', 'ghp_x');
    type('Encryption secret', 'wrong');
    await submit('Connect');
    expect(alert()).toContain('decryption failed');
    expect(unlocked).toBe(0);
    expect(heading()).toBe('Connect this device');
  });
});

describe('a connected device', () => {
  it('asks only for the secret, and whether to trust the device', async () => {
    session.openDevice.mockResolvedValue('unlock');
    await open();
    expect(heading()).toBe('Unlock');
    expect([...document.querySelectorAll('label input')].map((i) => (i as HTMLInputElement).type)).toEqual([
      'password',
      'checkbox',
    ]);
    expect(document.body.textContent).toContain('Anyone who can use this browser can open your tasks.');
    expect(button('Unlock with passkey')).toBeUndefined();
  });

  it('has a hidden username, so a password manager fills its saved login here', async () => {
    session.openDevice.mockResolvedValue('unlock');
    await open();
    expect(document.querySelector('input[autocomplete=username]')?.getAttribute('aria-hidden')).toBe('true');
    expect(input('Encryption secret').autocomplete).toBe('current-password');
  });

  it('is unlocked without asking (another open tab, or a trusted device)', async () => {
    session.openDevice.mockResolvedValue('unlocked');
    app = mount(StaticGate, { target: document.body, props: { defaults: {}, onunlocked: () => unlocked++ } });
    await vi.waitFor(() => expect(unlocked).toBe(1));
    // It never asked: no form was drawn.
    expect(document.querySelector('input')).toBeNull();
  });

  it('refuses a wrong secret and changes nothing', async () => {
    session.openDevice.mockResolvedValue('unlock');
    session.unlockWithSecret.mockRejectedValue(new WrongPassword());
    await open();
    type('Encryption secret', 'wrong');
    await submit('Unlock');
    expect(alert()).toContain('not the encryption secret');
    expect(unlocked).toBe(0);
    expect(session.forget).not.toHaveBeenCalled();
  });

  it('unlocks with the right one', async () => {
    session.openDevice.mockResolvedValue('unlock');
    session.unlockWithSecret.mockResolvedValue(undefined);
    await open();
    type('Encryption secret', 'right');
    await submit('Unlock');
    expect(session.unlockWithSecret).toHaveBeenCalledWith('right', false);
    expect(unlocked).toBe(1);
  });

  it('trusts the device when that is ticked', async () => {
    session.openDevice.mockResolvedValue('unlock');
    session.unlockWithSecret.mockResolvedValue(undefined);
    await open();
    type('Encryption secret', 'right');
    tick_('Trust this device');
    await submit('Unlock');
    expect(session.unlockWithSecret).toHaveBeenCalledWith('right', true);
  });

  it('forgets the device only when that is confirmed', async () => {
    session.openDevice.mockResolvedValue('unlock');
    await open();
    // The page's own question, answered no and then yes.
    button('Forget this device').click();
    await vi.waitFor(() => expect(asker.current?.message).toMatch(/^Forget this device\?/));
    asker.answer(false);
    await tick();
    expect(session.forget).not.toHaveBeenCalled();
    button('Forget this device').click();
    await vi.waitFor(() => expect(asker.current).not.toBeNull());
    asker.answer(true);
    await vi.waitFor(() => expect(session.forget).toHaveBeenCalledTimes(1));
  });
});

describe('a device with a passkey (spec 0019)', () => {
  beforeEach(() => {
    session.openDevice.mockResolvedValue('unlock');
    session.passkeyKind.mockResolvedValue('prf');
  });

  it('offers it first, and unlocks with it', async () => {
    session.unlockWithPasskey.mockResolvedValue(undefined);
    await open();
    button('Unlock with passkey').click();
    await vi.waitFor(() => expect(unlocked).toBe(1));
    expect(session.unlockWithSecret).not.toHaveBeenCalled();
  });

  it('cancelled, leaves the form as it was, ready for the secret', async () => {
    session.unlockWithPasskey.mockRejectedValue(new PasskeyCancelled());
    await open();
    await submit('Unlock with passkey');
    expect(alert()).toBe('');
    expect(unlocked).toBe(0);
    expect(document.activeElement).toBe(input('Encryption secret'));
  });

  it('failing, says why and keeps the secret ready', async () => {
    session.unlockWithPasskey.mockRejectedValue(new Error("That passkey's signature didn't check out."));
    await open();
    await submit('Unlock with passkey');
    expect(alert()).toContain("didn't check out");
    expect(unlocked).toBe(0);
    expect(document.activeElement).toBe(input('Encryption secret'));
  });
});

describe('a tab that locked with reminders on (spec 0017)', () => {
  it('says it keeps reminding, and of how many tasks', async () => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ enabled: true }));
    const later = Math.floor(Date.now() / 1000) + 86_400 * 3 + 3600;
    const task = (uuid: string) => ({
      uuid,
      status: 'pending',
      description: uuid,
      due: later,
      wait: null,
      scheduled: null,
    });
    sessionStorage.setItem('meghnaad-locked-reminders', JSON.stringify([task('a'), task('b')]));
    session.openDevice.mockResolvedValue('unlock');
    await open();
    expect(document.body.textContent).toContain(
      'Reminders keep coming for 2 tasks, as they were when this tab locked.',
    );
  });

  it('says nothing of reminders when it kept none', async () => {
    session.openDevice.mockResolvedValue('unlock');
    await open();
    expect(document.body.textContent).not.toContain('Reminders keep coming');
  });
});
