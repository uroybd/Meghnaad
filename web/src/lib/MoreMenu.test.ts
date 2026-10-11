// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import type { TaskCommand } from './appsettings';
import MoreMenu from './MoreMenu.svelte';

const CMDS: TaskCommand[] = [
  { name: 'fail', command: 'done outcome:fail', icon: 'circle-x' },
  { name: 'later', command: 'modify wait:2d' },
];

let app: ReturnType<typeof mount> | null = null;
// The icon of a command is fetched from the CDN: not from a test.
beforeEach(() => {
  vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('offline')));
});
afterEach(() => {
  vi.unstubAllGlobals();
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

const trigger = () => document.querySelector('button[aria-haspopup=menu]') as HTMLButtonElement | null;
const items = () => [...document.querySelectorAll<HTMLButtonElement>('[role=menuitem]')];
const tick = async () => {
  for (let i = 0; i < 4; i++) await Promise.resolve();
  flushSync();
};

describe('the more-actions menu', () => {
  it('draws nothing when there are no commands', () => {
    app = mount(MoreMenu, { target: document.body, props: { commands: [], onrun: vi.fn() } });
    flushSync();
    expect(trigger()).toBeNull();
  });

  it('opens on the button and lists each command with what it does', async () => {
    app = mount(MoreMenu, { target: document.body, props: { commands: CMDS, onrun: vi.fn() } });
    flushSync();
    expect(trigger()?.getAttribute('aria-expanded')).toBe('false');
    expect(items()).toHaveLength(0);
    trigger()!.click();
    await tick();
    expect(trigger()?.getAttribute('aria-expanded')).toBe('true');
    expect(items().map((i) => i.querySelector('.name')?.textContent)).toEqual(['fail', 'later']);
    expect(items()[0].textContent).toContain('done outcome:fail');
    // The first item is focused, for the keyboard.
    expect(document.activeElement).toBe(items()[0]);
  });

  it('runs the one chosen and closes', async () => {
    const onrun = vi.fn();
    app = mount(MoreMenu, { target: document.body, props: { commands: CMDS, onrun } });
    flushSync();
    trigger()!.click();
    await tick();
    items()[1].click();
    await tick();
    expect(onrun).toHaveBeenCalledWith(CMDS[1]);
    expect(items()).toHaveLength(0);
  });

  it('moves with the arrow keys, and Escape closes it and gives the focus back', async () => {
    app = mount(MoreMenu, { target: document.body, props: { commands: CMDS, onrun: vi.fn() } });
    flushSync();
    trigger()!.click();
    await tick();
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    expect(document.activeElement).toBe(items()[1]);
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    expect(document.activeElement).toBe(items()[0]);
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await tick();
    expect(items()).toHaveLength(0);
    expect(document.activeElement).toBe(trigger());
  });

  it('closes when something else is clicked', async () => {
    app = mount(MoreMenu, { target: document.body, props: { commands: CMDS, onrun: vi.fn() } });
    flushSync();
    trigger()!.click();
    await tick();
    document.body.dispatchEvent(new Event('pointerdown', { bubbles: true }));
    await tick();
    expect(items()).toHaveLength(0);
  });

  it('closes when the page or a table scrolls, but not when its own list does', async () => {
    app = mount(MoreMenu, { target: document.body, props: { commands: CMDS, onrun: vi.fn() } });
    flushSync();
    trigger()!.click();
    await tick();
    document.querySelector('[role=menu]')!.dispatchEvent(new Event('scroll'));
    await tick();
    expect(items()).toHaveLength(2);
    const table = document.body.appendChild(document.createElement('div'));
    table.dispatchEvent(new Event('scroll'));
    await tick();
    expect(items()).toHaveLength(0);
  });
});
