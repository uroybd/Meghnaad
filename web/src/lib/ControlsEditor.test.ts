// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import type { Control } from './boards';
import { reactive } from '../reactive.svelte';
import ControlsEditor from './ControlsEditor.svelte';

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

const draw = (controls: Control[]) => {
  const props = reactive({ controls });
  app = mount(ControlsEditor, { target: document.body, props });
  flushSync();
  return props;
};
const button = (text: string) =>
  [...document.querySelectorAll('.foot button')].find((b) => b.textContent?.trim() === text) as HTMLButtonElement;
const type = (selector: string, value: string) => {
  const input = document.querySelector<HTMLInputElement>(selector)!;
  input.value = value;
  input.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
};

describe('the controls of a board, in its settings', () => {
  it('adds a control of each kind, named for its kind and differently the second time', () => {
    const p = draw([]);
    for (const kind of ['Date range', 'Date range', 'Number', 'Selection', 'Switch']) {
      button(kind).click();
      flushSync();
    }
    expect(p.controls).toEqual([
      { name: 'daterange', type: 'daterange', start: 'sow', end: 'eow' },
      { name: 'daterange2', type: 'daterange', start: 'sow', end: 'eow' },
      { name: 'number', type: 'number', value: '0' },
      { name: 'select', type: 'select', source: 'project' },
      { name: 'boolean', type: 'boolean', on: '', off: '' },
    ]);
  });

  it('renames a control in lower case, labels it with any text, and removes it', () => {
    const p = draw([{ name: 'a', type: 'daterange', start: 'som', end: 'eom' }]);
    type('input[aria-label="Name of control 1"]', 'Month');
    type('input[aria-label="Label of control 1"]', 'This month, so far');
    expect(p.controls[0]).toEqual({
      name: 'month',
      label: 'This month, so far',
      type: 'daterange',
      start: 'som',
      end: 'eom',
    });
    type('input[aria-label="Label of control 1"]', '');
    expect(p.controls[0].label).toBeUndefined();
    document.querySelector<HTMLButtonElement>('button[aria-label="Remove month"]')!.click();
    flushSync();
    expect(p.controls).toEqual([]);
  });

  it('sets what a switch writes, and what a selection picks from', () => {
    const p = draw([
      { name: 'next', type: 'boolean' },
      { name: 'tags', type: 'select', source: 'project' },
    ]);
    type('input[aria-label="Filter of next when on"]', '+next');
    const from = document.querySelector<HTMLSelectElement>('select[aria-label="What tags picks from"]')!;
    from.value = 'tags';
    from.dispatchEvent(new Event('change', { bubbles: true }));
    flushSync();
    const several = [...document.querySelectorAll<HTMLLabelElement>('label.check')].find((l) =>
      l.textContent?.includes('Several'),
    )!;
    several.querySelector('input')!.click();
    flushSync();
    type('input[aria-label="What joins the values of tags"]', 'and');
    expect(p.controls).toEqual([
      { name: 'next', type: 'boolean', on: '+next' },
      { name: 'tags', type: 'select', source: 'tags', multiple: true, join: 'and' },
    ]);
  });

  it('swaps a selection’s choices for a field to type in, starting from what is picked', () => {
    const p = draw([{ name: 'tags', type: 'select', source: 'tags', multiple: true, values: ['a', 'My Home'] }]);
    const swap = () => document.querySelector<HTMLButtonElement>('button.swap')!;
    expect(document.querySelector('input[aria-label="Expression of Tags"]')).toBeNull();
    swap().click();
    flushSync();
    expect(p.controls[0]).toMatchObject({ expr: "a or 'My Home'" });
    expect(document.querySelector<HTMLInputElement>('input[aria-label="Expression of Tags"]')!.value).toBe(
      "a or 'My Home'",
    );
    swap().click();
    flushSync();
    expect(p.controls[0]).toMatchObject({ values: ['a', 'My Home'], expr: undefined });
    expect(document.querySelector('input[aria-label="Expression of Tags"]')).toBeNull();
    // A board can open it typed.
    const typed = [...document.querySelectorAll<HTMLLabelElement>('label.check')].find((l) =>
      l.textContent?.includes('Typed in at first'),
    )!;
    typed.querySelector('input')!.click();
    flushSync();
    expect(p.controls[0]).toMatchObject({ typed: true });
  });
});
