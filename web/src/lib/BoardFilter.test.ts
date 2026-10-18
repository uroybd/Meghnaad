// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import type { Board } from './boards';
import BoardFilter from './BoardFilter.svelte';
import { store } from './store.svelte';

const board: Board = { id: 'wk', name: 'Week', filter: 'rc.context=this_week', sections: [] };

let app: ReturnType<typeof mount> | null = null;
beforeEach(() => (store.boardFilters = {}));
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

const draw = () => {
  app = mount(BoardFilter, { target: document.body, props: { board } });
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
