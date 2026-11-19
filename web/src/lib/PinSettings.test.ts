// @vitest-environment jsdom
// Setting the PIN (spec 0022): four keys, the same four again, saved with no button; a mismatch starts again.
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';

vi.mock('../engine/session', () => ({ pinLock: vi.fn() }));
import { checkPin, hasPin } from '../engine/pin';
import PinSettings from './PinSettings.svelte';

const tick = async () => {
  for (let i = 0; i < 5; i++) await new Promise((r) => setTimeout(r, 0));
  flushSync();
};
const button = (name: string) => [...document.querySelectorAll('button')].find((b) => b.textContent?.trim() === name);
const press = async (...keys: string[]) => {
  const box = document.querySelector('.capture') as HTMLElement;
  for (const k of keys) box.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));
  await tick();
};

let app: ReturnType<typeof mount> | null = null;
beforeEach(() => localStorage.clear());
afterEach(() => {
  if (app) unmount(app);
  app = null;
});

it('is saved when the second four match, with no button, and asks again when they differ', async () => {
  app = mount(PinSettings, { target: document.body });
  button('Set a PIN')?.click();
  await tick();
  await press('1', 'a', 'ArrowUp', '9');
  expect(document.querySelector('.capture')?.textContent).toContain('Again');
  await press('1', 'a', 'ArrowUp', '8');
  expect(document.querySelector('[role=status]')?.textContent).toContain('differ');
  expect(hasPin()).toBe(false);
  await press('1', 'a', 'ArrowUp', '9', '1', 'a', 'ArrowUp', '9');
  expect(hasPin()).toBe(true);
  expect(await checkPin(['1', 'a', '↑', '9'])).toBe(true);
  expect(document.querySelector('.capture')).toBeNull();
  expect(button('Lock with PIN now')).toBeTruthy();
  button('Remove PIN')?.click();
  await tick();
  expect(hasPin()).toBe(false);
});
