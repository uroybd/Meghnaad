// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import ContextPicker from './ContextPicker.svelte';
import { store } from './store.svelte';
import type { ConfigResponse } from './types';

const config = (active: string | null, names = ['home', 'work']) =>
  ({
    config: {
      settings: {},
      active_context: active,
      contexts: Object.fromEntries(names.map((n) => [n, { name: n, read: `+${n}`, write: null, rc: {} }])),
    },
  }) as unknown as ConfigResponse;

let app: ReturnType<typeof mount> | null = null;
beforeEach(() => {
  localStorage.clear();
  store.contextChoices = {};
  store.config = config('work');
});
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  store.config = null;
});

const draw = (scope: Parameters<typeof store.setContextChoice>[0] = 'tasks') => {
  app = mount(ContextPicker, { target: document.body, props: { scope } });
  flushSync();
  return document.querySelector('select') as HTMLSelectElement | null;
};

describe('a page’s context', () => {
  it('follows the active one until it is told otherwise, and says which that is', () => {
    const select = draw()!;
    expect(select.value).toBe('');
    expect([...select.options].map((o) => o.textContent?.trim())).toEqual([
      'Context: work (same as console)',
      'Context: none',
      'Context: home',
      'Context: work',
    ]);
    expect(store.contextIn('tasks')).toBe('work');
  });

  it('keeps its own choice, which does not change the active context or another page', () => {
    const select = draw()!;
    select.value = 'home';
    select.dispatchEvent(new Event('change', { bubbles: true }));
    flushSync();
    expect(store.contextChoice('tasks')).toBe('home');
    expect(store.contextIn('tasks')).toBe('home');
    expect(store.contextIn('summary')).toBe('work');
    expect(store.activeContext).toBe('work');
    expect(JSON.parse(localStorage.getItem('tw-web-page-contexts')!)).toEqual({ tasks: 'home' });
    expect(select.classList.contains('own')).toBe(true);
  });

  it('can say none, whatever is active, and go back to following', () => {
    const select = draw()!;
    select.value = 'none';
    select.dispatchEvent(new Event('change', { bubbles: true }));
    flushSync();
    expect(store.contextIn('tasks')).toBeNull();
    select.value = '';
    select.dispatchEvent(new Event('change', { bubbles: true }));
    flushSync();
    expect(store.contextChoice('tasks')).toBeUndefined();
    expect(store.contextIn('tasks')).toBe('work');
    expect(localStorage.getItem('tw-web-page-contexts')).toBe('{}');
  });

  it('forgets a context that was deleted, and is not drawn when there is nothing to choose', () => {
    store.setContextChoice('tasks', 'gone');
    expect(store.contextChoice('tasks')).toBeUndefined();
    store.setContextChoice('tasks', undefined);
    store.config = config(null, []);
    expect(draw()).toBeNull();
  });
});
