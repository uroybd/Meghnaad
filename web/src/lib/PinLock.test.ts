// @vitest-environment jsdom
// The PIN screen (spec 0022): four keys, Backspace, a wrong PIN, and nothing under it seeing the keys.
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';

const session = vi.hoisted(() => ({
  watcher: null as ((locked: boolean) => void) | null,
  onPin: vi.fn(),
  pinUnlock: vi.fn(),
  lock: vi.fn(),
}));
vi.mock('../engine/session', () => session);
import PinLock from './PinLock.svelte';

const tick = async () => {
  for (let i = 0; i < 5; i++) await new Promise((r) => setTimeout(r, 0));
  flushSync();
};
const key = (k: string) => {
  const e = new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true });
  document.body.dispatchEvent(e);
  return e;
};
const dots = () => document.querySelectorAll('.dots span.on').length;

let app: ReturnType<typeof mount> | null = null;
beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function () {
    this.setAttribute('open', '');
  };
  HTMLDialogElement.prototype.close = function () {
    this.removeAttribute('open');
  };
  session.onPin.mockImplementation((fn: (l: boolean) => void) => {
    session.watcher = fn;
    fn(false);
    return () => undefined;
  });
  session.pinUnlock.mockReset();
  session.lock.mockReset();
});
afterEach(() => {
  if (app) unmount(app);
  app = null;
});

it('comes up when locked, takes four keys, and takes Backspace back', async () => {
  app = mount(PinLock, { target: document.body });
  await tick();
  expect(document.querySelector('dialog')?.hasAttribute('open')).toBe(false);
  // Keys while it is down go to the app.
  expect(key('a').defaultPrevented).toBe(false);

  session.watcher?.(true);
  await tick();
  expect(document.querySelector('dialog')?.hasAttribute('open')).toBe(true);
  session.pinUnlock.mockResolvedValue(true);
  const under = vi.fn();
  window.addEventListener('keydown', under);
  key('1');
  key('*');
  key('Enter');
  key('A');
  await tick();
  expect(dots()).toBe(2);
  key('Backspace');
  await tick();
  expect(dots()).toBe(1);
  key('a');
  key('ArrowUp');
  key('9');
  await tick();
  expect(session.pinUnlock).toHaveBeenCalledWith(['1', 'a', '↑', '9']);
  expect(under).not.toHaveBeenCalled();
  window.removeEventListener('keydown', under);
});

it('says a wrong PIN and clears, and Forgot the PIN? locks fully', async () => {
  app = mount(PinLock, { target: document.body });
  await tick();
  session.watcher?.(true);
  await tick();
  session.pinUnlock.mockResolvedValue(false);
  for (const k of ['1', '2', '3', '4']) key(k);
  await tick();
  expect(dots()).toBe(0);
  expect(document.querySelector('[role=status]')?.textContent).toContain("isn't the PIN");
  (document.querySelector('.forgot') as HTMLButtonElement).click();
  expect(session.lock).toHaveBeenCalled();
});
