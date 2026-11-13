// @vitest-environment jsdom
// The page's keys against the fields of a page: no page takes the keyboard when it opens, Escape gives it back, and
// `a` adds from any page.
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, tick, unmount } from 'svelte';
import { readFileSync } from 'node:fs';
import App from './App.svelte';
import { store } from './lib/store.svelte';

let app: ReturnType<typeof mount> | null = null;
const DEFAULTS = readFileSync('public/keymaps/default.json', 'utf8');
const CONFIG = { reports: [], config: { settings: {}, contexts: {}, active_context: null } };

beforeAll(() => {
  // Set up already (no answer about setup starts the app), and every other request answered with nothing.
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string) =>
      url === '/api/setup'
        ? Promise.reject(new Error('offline'))
        : new Response(
            url === '/api/config'
              ? JSON.stringify(CONFIG)
              : url.startsWith('/keymaps/default.json?v=')
                ? DEFAULTS
                : '{}',
            { status: 200 },
          ),
    ),
  );
  window.matchMedia = ((q: string) => ({
    matches: false,
    media: q,
    addEventListener() {},
    removeEventListener() {},
  })) as never;
  // jsdom has no modal dialogs.
  HTMLDialogElement.prototype.showModal = function (this: HTMLDialogElement) {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function (this: HTMLDialogElement) {
    this.open = false;
  };
});

afterEach(() => {
  if (app) unmount(app);
  app = null;
  store.adding = null;
  store.view = 'tasks';
});

async function start(view: typeof store.view) {
  app = mount(App, { target: document.body });
  // Let it start (the settings, the keys, the address) before going to the page.
  await vi.waitFor(() => expect(store.keymap.contexts.app?.a).toBe('app.add'));
  store.view = view;
  flushSync();
  await tick();
}

const press = (key: string, target: EventTarget = document.body) => {
  target.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
  flushSync();
};

describe("the page's keys and its fields", () => {
  it('leaves the keyboard to the keys when a page with a filter opens', async () => {
    await start('projects');
    expect(document.activeElement?.tagName).not.toBe('INPUT');
  });

  it('adds a task from a page other than Tasks', async () => {
    await start('projects');
    press('a');
    expect(store.adding?.quick).toBe(true);
  });

  it("puts the cursor in the page's filter with f, and Escape takes it out again", async () => {
    await start('projects');
    press('f');
    await new Promise((r) => setTimeout(r, 0)); // the field is focused once the page is drawn
    const field = document.activeElement as HTMLInputElement;
    expect(field.tagName).toBe('INPUT');
    press('a', field); // typed, not a key
    expect(store.adding).toBeNull();
    press('Escape', field);
    expect(document.activeElement).not.toBe(field);
    press('a');
    expect(store.adding?.quick).toBe(true);
  });
});
