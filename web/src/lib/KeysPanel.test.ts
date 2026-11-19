// @vitest-environment jsdom
import { readFileSync } from 'node:fs';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, tick, unmount } from 'svelte';

const api = vi.hoisted(() => ({ putAppSettings: vi.fn(), getAppSettings: vi.fn() }));
vi.mock('./api', async (original) => ({ ...(await original<typeof import('./api')>()), ...api }));

import { parseSettings } from './appsettings';
import KeysPanel from './KeysPanel.svelte';
import { store } from './store.svelte';
import type { CommandsDoc } from './keymap';

const defaults = JSON.parse(readFileSync('public/keymaps/default.json', 'utf8')) as Record<string, unknown>;
const commandsDoc = JSON.parse(readFileSync('public/keymaps/commands.json', 'utf8')) as CommandsDoc;

let app: ReturnType<typeof mount> | null = null;

async function open(settings: Record<string, unknown>) {
  store.appSettings = parseSettings(JSON.stringify(settings));
  store.keymapDefaults = defaults;
  store.commandsDoc = commandsDoc;
  app = mount(KeysPanel, { target: document.body });
  await tick();
}

const section = (label: string) => document.querySelector(`section[aria-label="${label}"]`) as HTMLElement;
const rows = (label: string) =>
  [...section(label).querySelectorAll('tbody tr')].map((r) => r.querySelector('code')?.textContent);
const button = (label: string, within: ParentNode = document) =>
  [...within.querySelectorAll('button')].find((b) => b.textContent?.trim() === label) as HTMLButtonElement;
function type(el: HTMLInputElement, text: string) {
  el.value = text;
  el.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
}

beforeEach(() => {
  localStorage.clear();
  api.putAppSettings.mockResolvedValue({ ok: true });
  api.getAppSettings.mockImplementation(async () => JSON.parse(api.putAppSettings.mock.calls.at(-1)?.[0] ?? '{}'));
});
afterEach(() => {
  if (app) unmount(app);
  app = null;
  vi.clearAllMocks();
});

describe('the Keybindings tab', () => {
  it('shows only your own keys until the defaults are asked for', async () => {
    await open({ version: 2, tui: { keys: { tasks: { x: 'tasks.done' } } } });
    expect(rows('Terminal')).toEqual(['tasks.done']);
    expect(rows('General')).toEqual([]);
    expect(section('General').textContent).toContain('None of your own.');

    const toggle = document.querySelector('.top input[type=checkbox]') as HTMLInputElement;
    toggle.click();
    flushSync();
    expect(rows('Web')).toContain('app.add');
    expect(section('Web').querySelector('tr.default kbd.muted')).not.toBeNull();
    expect(localStorage.getItem('tw-web-keys-defaults')).toBe('1');
  });

  it('searches what is shown by name, what it does, or a key', async () => {
    await open({
      version: 2,
      keys: { app: { 'ctrl+k': 'app.prompt' } },
      tui: { keys: { tasks: { x: 'tasks.done', z: 'tasks.start' } } },
    });
    const search = document.querySelector('input[aria-label="Search keys"]') as HTMLInputElement;
    type(search, 'start');
    expect(rows('Terminal')).toEqual(['tasks.start']);
    type(search, 'ctrl+k');
    expect(rows('General')).toEqual(['app.prompt']);
    expect(rows('Terminal')).toEqual([]);
  });

  it('adds a key from the search, typed, and applies it', async () => {
    await open({ version: 2, taskCommands: [] });
    button('Add', section('Web')).click();
    flushSync();
    type(section('Web').querySelector('input[aria-label="Search actions"]') as HTMLInputElement, 'add');
    const result = [...section('Web').querySelectorAll('.result')].find((b) =>
      b.textContent?.includes('app.add'),
    ) as HTMLButtonElement;
    expect(result.querySelector('kbd')?.textContent).toBe('a');
    result.click();
    flushSync();
    type(section('Web').querySelector('input[aria-label="Key"]') as HTMLInputElement, 'hyper+q');
    expect(section('Web').querySelector('.err')?.textContent).toContain('is not a key');
    type(section('Web').querySelector('input[aria-label="Key"]') as HTMLInputElement, 'Ctrl-N');
    button('Add key').click();
    flushSync();
    expect(rows('Web')).toEqual(['app.add']);
    expect(section('Web').querySelector('kbd')?.textContent).toBe('Ctrl+Shift+N');

    button('Apply').click();
    await vi.waitFor(() => expect(api.putAppSettings).toHaveBeenCalled());
    expect(JSON.parse(api.putAppSettings.mock.calls[0][0])).toEqual({
      version: 2,
      taskCommands: [],
      web: { keys: { app: { 'ctrl+shift+n': 'app.add', a: null } } },
    });
  });

  it('records a key pressed, and leaves an action with none', async () => {
    await open({});
    button('Add', section('Terminal')).click();
    flushSync();
    type(section('Terminal').querySelector('input[aria-label="Search actions"]') as HTMLInputElement, 'tasks.done');
    (section('Terminal').querySelector('.result') as HTMLButtonElement).click();
    flushSync();
    const capture = section('Terminal').querySelector('.capture') as HTMLButtonElement;
    capture.click();
    flushSync();
    const outside = vi.fn();
    window.addEventListener('keydown', outside);
    capture.dispatchEvent(new KeyboardEvent('keydown', { key: '/', bubbles: true, cancelable: true }));
    flushSync();
    window.removeEventListener('keydown', outside);
    expect(outside).not.toHaveBeenCalled();
    expect(capture.textContent).toContain('/');
    button('No key').click();
    flushSync();
    expect(section('Terminal').querySelector('tbody')?.textContent).toContain('no key');
  });

  it('lists the commands a key can run, and a key bound to one with what it does', async () => {
    await open({ version: 2, tui: { keys: { tasks: { 'g h': 'filter +home' } } } });
    const names = () => [...section('Commands').querySelectorAll('kbd')].map((k) => k.textContent);
    expect(names()).toContain(':tabn[ext]');
    expect(section('Commands').textContent).toContain('takes');
    const more = section('Commands').querySelector('a') as HTMLAnchorElement;
    expect(more.href).toBe(`${commandsDoc.docs}#tabnext`);
    expect(rows('Terminal')).toEqual(['filter +home']);
    expect(section('Terminal').querySelector('.err')).toBeNull();
    type(document.querySelector('input[aria-label="Search keys"]') as HTMLInputElement, 'tabn');
    expect(names()).toEqual([':tabn[ext]', ':tabN[ext]']);
    type(document.querySelector('input[aria-label="Search keys"]') as HTMLInputElement, 'denot');
    expect(names()).toEqual([':den[otate]']);
  });

  it('takes out a key of your own', async () => {
    await open({ web: { keys: { 'app.help': 'h' } } });
    // An old file is read as brought up to date, and saved so.
    (section('Web').querySelector('button[aria-label="Take out app.help"]') as HTMLButtonElement).click();
    flushSync();
    expect(rows('Web')).toEqual([]);
    button('Apply').click();
    await vi.waitFor(() => expect(api.putAppSettings).toHaveBeenCalled());
    expect(JSON.parse(api.putAppSettings.mock.calls[0][0])).toEqual({ version: 2 });
  });
});
