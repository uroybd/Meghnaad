// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import AppearancePanel from './AppearancePanel.svelte';
import { parseSettings, type Appearance } from './appsettings';
import { store } from './store.svelte';
import type { ConfigResponse } from './types';

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  store.config = null;
  store.appSettings = null;
  vi.restoreAllMocks();
});

const jira = { match: '\\b(BOR-\\d+)\\b', replace: '[$1](https://j.example/$1)', fields: ['description'] };

function draw(transformations: unknown[] = []) {
  store.appSettings = parseSettings(JSON.stringify({ appearance: { transformations } }));
  store.config = {
    colors: {},
    config: {
      udas: { jira: { name: 'jira', type: 'string', label: 'Jira', values: [], default: null } },
      settings: {},
      reports: {},
    },
  } as unknown as ConfigResponse;
  app = mount(AppearancePanel, { target: document.body, props: {} });
  flushSync();
}

const type = (el: HTMLInputElement, value: string) => {
  el.value = value;
  el.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
};
const button = (name: string) =>
  [...document.querySelectorAll('button')].find((b) => b.textContent?.includes(name) || b.ariaLabel === name)!;
const apply = () => button('Apply');

describe('the Transformations section', () => {
  it('shows the saved rules with the text fields to tick, string UDAs among them', () => {
    draw([jira]);
    expect(document.querySelector<HTMLInputElement>('input[aria-label="Match"]')!.value).toBe(jira.match);
    const fields = [...document.querySelectorAll('[aria-label="Fields for transformation 1"] label')].map((l) => [
      l.textContent?.trim(),
      l.querySelector('input')!.checked,
    ]);
    expect(fields).toEqual([
      ['description', true],
      ['annotations', false],
      ['jira', false],
    ]);
  });

  it('adds a rule, says what the engine finds wrong with it, and saves it once it is right', async () => {
    draw();
    const save = vi.spyOn(store, 'saveAppearance').mockResolvedValue();
    button('Add a transformation').click();
    flushSync();
    type(document.querySelector<HTMLInputElement>('input[aria-label="Match"]')!, '(');
    expect(document.querySelector('ul.err')?.textContent).toContain('transformation 1:');
    expect(apply().disabled).toBe(true);
    type(document.querySelector<HTMLInputElement>('input[aria-label="Match"]')!, jira.match);
    type(document.querySelector<HTMLInputElement>('input[aria-label="Replace"]')!, jira.replace);
    expect(document.querySelector('ul.err')).toBeNull();
    apply().click();
    await vi.waitFor(() => expect(save).toHaveBeenCalled());
    expect((save.mock.calls[0][0] as Appearance).transformations).toEqual([jira]);
  });

  it('reorders and removes rules', () => {
    draw([jira, { ...jira, match: 'two' }]);
    const matches = () =>
      [...document.querySelectorAll<HTMLInputElement>('input[aria-label="Match"]')].map((i) => i.value);
    document.querySelectorAll<HTMLButtonElement>('button[aria-label="Move down"]')[0].click();
    flushSync();
    expect(matches()).toEqual(['two', jira.match]);
    button('Remove transformation 1').click();
    flushSync();
    expect(matches()).toEqual([jira.match]);
  });
});
