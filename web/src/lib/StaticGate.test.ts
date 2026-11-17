// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, tick, unmount } from 'svelte';
import { WrongPassword } from '../engine/pgp';

const session = vi.hoisted(() => ({
  askOtherTabs: vi.fn(),
  connect: vi.fn(),
  forget: vi.fn(),
  unlockWithSecret: vi.fn(),
}));
vi.mock('../engine/session', () => session);
const vault = vi.hoisted(() => ({ sealedAccess: vi.fn() }));
vi.mock('../engine/vault', () => vault);

import StaticGate from './StaticGate.svelte';

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
  for (const f of [...Object.values(session), ...Object.values(vault)]) f.mockReset();
  session.askOtherTabs.mockResolvedValue(false);
});

afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

describe('a device that was never connected', () => {
  it('asks for the branch, the token and the secret, filled from config.json', async () => {
    vault.sealedAccess.mockResolvedValue(undefined);
    await open({ repo: 'alice/tasks', branch: 'main' });
    expect(heading()).toBe('Connect this device');
    expect(input('Repository').value).toBe('alice/tasks');
    expect(input('Branch').value).toBe('main');
    expect(session.askOtherTabs).not.toHaveBeenCalled();
  });

  it('connects with what was typed, and goes on', async () => {
    vault.sealedAccess.mockResolvedValue(undefined);
    session.connect.mockResolvedValue(undefined);
    await open({ repo: 'alice/tasks' });
    type('Token', 'ghp_x');
    type('Encryption secret', 's3cret');
    await submit('Connect');
    expect(session.connect).toHaveBeenCalledWith({
      api: '',
      repo: 'alice/tasks',
      branch: '',
      token: 'ghp_x',
      secret: 's3cret',
    });
    expect(unlocked).toBe(1);
  });

  it('marks the token as the username and the secret as the password, for password managers', async () => {
    vault.sealedAccess.mockResolvedValue(undefined);
    await open();
    expect(input('Token').autocomplete).toBe('username');
    expect(input('Token').type).toBe('text');
    expect(input('Encryption secret').autocomplete).toBe('current-password');
    expect(input('Encryption secret').type).toBe('password');
  });

  it('asks the browser to save the token and the secret once connected, where it can', async () => {
    vault.sealedAccess.mockResolvedValue(undefined);
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
    vault.sealedAccess.mockResolvedValue(undefined);
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
  it('asks only for the secret', async () => {
    vault.sealedAccess.mockResolvedValue(new Uint8Array([1]));
    await open();
    expect(heading()).toBe('Unlock');
    expect(document.querySelectorAll('label input').length).toBe(1);
  });

  it('has a hidden username, so a password manager fills its saved login here', async () => {
    vault.sealedAccess.mockResolvedValue(new Uint8Array([1]));
    await open();
    expect(document.querySelector('input[autocomplete=username]')?.getAttribute('aria-hidden')).toBe('true');
    expect(input('Encryption secret').autocomplete).toBe('current-password');
  });

  it('is unlocked by another open tab without asking', async () => {
    vault.sealedAccess.mockResolvedValue(new Uint8Array([1]));
    session.askOtherTabs.mockResolvedValue(true);
    app = mount(StaticGate, { target: document.body, props: { defaults: {}, onunlocked: () => unlocked++ } });
    await vi.waitFor(() => expect(unlocked).toBe(1));
    // It never asked: no form was drawn.
    expect(document.querySelector('input')).toBeNull();
  });

  it('refuses a wrong secret and changes nothing', async () => {
    vault.sealedAccess.mockResolvedValue(new Uint8Array([1]));
    session.unlockWithSecret.mockRejectedValue(new WrongPassword());
    await open();
    type('Encryption secret', 'wrong');
    await submit('Unlock');
    expect(alert()).toContain('not the encryption secret');
    expect(unlocked).toBe(0);
    expect(session.forget).not.toHaveBeenCalled();
  });

  it('unlocks with the right one', async () => {
    vault.sealedAccess.mockResolvedValue(new Uint8Array([1]));
    session.unlockWithSecret.mockResolvedValue(undefined);
    await open();
    type('Encryption secret', 'right');
    await submit('Unlock');
    expect(session.unlockWithSecret).toHaveBeenCalledWith('right');
    expect(unlocked).toBe(1);
  });

  it('forgets the device only when that is confirmed', async () => {
    vault.sealedAccess.mockResolvedValue(new Uint8Array([1]));
    await open();
    const ask = vi.spyOn(window, 'confirm').mockReturnValueOnce(false).mockReturnValueOnce(true);
    button('Forget this device').click();
    await tick();
    expect(session.forget).not.toHaveBeenCalled();
    button('Forget this device').click();
    await tick();
    expect(session.forget).toHaveBeenCalledTimes(1);
    ask.mockRestore();
  });
});
