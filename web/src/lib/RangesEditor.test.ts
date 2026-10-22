// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import type { DateRange } from './boards';
import { reactive } from '../reactive.svelte';
import RangesEditor from './RangesEditor.svelte';

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

const draw = (ranges: DateRange[]) => {
  const props = reactive({ ranges });
  app = mount(RangesEditor, { target: document.body, props });
  flushSync();
  return props;
};
const button = (text: string) => [...document.querySelectorAll('button')].find((b) => b.textContent?.includes(text))!;

describe('the date ranges of a board, in its settings', () => {
  it('adds a range named for the week, and names the next one differently', () => {
    const p = draw([]);
    button('Date range').click();
    flushSync();
    button('Date range').click();
    flushSync();
    expect(p.ranges).toEqual([
      { name: 'daterange', start: 'sow', end: 'eow' },
      { name: 'daterange2', start: 'sow', end: 'eow' },
    ]);
  });

  it('renames a range in lower case, and removes it', () => {
    const p = draw([{ name: 'a', start: 'som', end: 'eom' }]);
    const name = document.querySelector<HTMLInputElement>('input[aria-label="Name of date range 1"]')!;
    name.value = 'Month';
    name.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    expect(p.ranges[0]).toEqual({ name: 'month', start: 'som', end: 'eom' });
    document.querySelector<HTMLButtonElement>('button[aria-label="Remove month"]')!.click();
    flushSync();
    expect(p.ranges).toEqual([]);
  });
});
