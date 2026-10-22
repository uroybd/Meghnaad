// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import type { Board } from './boards';
import BoardFilter from './BoardFilter.svelte';
import { store } from './store.svelte';

const board: Board = { id: 'wk', name: 'Week', filter: 'rc.context=this_week', sections: [] };

let app: ReturnType<typeof mount> | null = null;
beforeEach(() => {
  store.boardFilters = {};
  store.boardRanges = {};
});
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

const draw = (b: Board = board) => {
  app = mount(BoardFilter, { target: document.body, props: { board: b } });
  flushSync();
};
const toggle = () => document.querySelector<HTMLButtonElement>('button.toggle')!;
const field = () => document.querySelector<HTMLTextAreaElement | HTMLInputElement>('#board-view-filter-input');

describe('the filter above a board', () => {
  it('is collapsed, and opens to the board’s own filter', () => {
    draw();
    expect(toggle().getAttribute('aria-expanded')).toBe('false');
    expect(field()).toBeNull();
    toggle().click();
    flushSync();
    expect(toggle().getAttribute('aria-expanded')).toBe('true');
    expect(field()!.value).toBe('rc.context=this_week');
    expect(document.body.textContent).toContain("The board's own filter is rc.context=this_week");
  });

  it('keeps what is typed for the board, says so while collapsed, and goes back to the board’s own', () => {
    draw();
    toggle().click();
    flushSync();
    const input = field()!;
    input.value = 'rc.context=none project:Home';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    expect(store.boardFilters).toEqual({ wk: 'rc.context=none project:Home' });
    expect(document.querySelector('.changed')).not.toBeNull();
    document.querySelector<HTMLButtonElement>('button[aria-label="Use the board\'s own filter"]')!.click();
    flushSync();
    expect(store.boardFilters).toEqual({});
    expect(document.querySelector('.changed')).toBeNull();
  });

  it('can be emptied, which is no filter and not the board’s own', () => {
    store.boardFilters = { wk: '' };
    draw();
    expect(document.querySelector('.changed')).not.toBeNull();
  });
});

describe('the date ranges above a board', () => {
  const withRange: Board = {
    ...board,
    ranges: [{ name: 'sprint_week', start: 'sow', end: 'eow' }],
  };
  const start = () => document.querySelector<HTMLButtonElement>('button[aria-label^="Start of"]')!;

  it('shows each range by a label, with its own dates and what they are now', () => {
    draw(withRange);
    expect(document.querySelector('.range .name')!.textContent).toBe('Sprint week');
    expect(start().textContent).toContain('sow');
    expect(document.querySelector('.resolved')!.textContent).toMatch(/\$sprint_week .+ – /);
  });

  it('draws nothing for a board without ranges', () => {
    draw();
    expect(document.querySelector('.range')).toBeNull();
    expect(document.querySelector('.resolved')).toBeNull();
  });

  it('keeps the dates chosen for the board, says what is wrong with one, and goes back to the board’s own', () => {
    draw(withRange);
    start().click();
    flushSync();
    const input = document.querySelector<HTMLInputElement>('#datefield-input')!;
    input.value = 'som';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    expect(store.boardRanges).toEqual({ wk: { sprint_week: { start: 'som', end: 'eow' } } });
    input.value = 'soonish';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    expect(document.querySelector('.resolved .err')!.textContent).toContain("'soonish' is not a date");
    document.querySelector<HTMLButtonElement>('button[aria-label^="Use the board\'s own dates"]')!.click();
    flushSync();
    expect(store.boardRanges).toEqual({});
  });
});
