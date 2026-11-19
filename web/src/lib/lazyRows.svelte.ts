import { untrack } from 'svelte';

/**
 * A report of thousands of tasks takes over a second to lay out (about 0.4 ms a row), and the page does not answer
 * meanwhile. The top of the table is drawn at once, and more as the end of it nears the window or when asked. It is
 * still the whole report: the select-all box, the counts and the sort are about every row, drawn or not.
 */
export const FIRST_ROWS = 200;
export const MORE_ROWS = 300;

/** What the table scrolls in: the page scrolls inside its main area, and a margin around the window only reaches past it. */
function scroller(el: HTMLElement): HTMLElement | null {
  for (let up = el.parentElement; up; up = up.parentElement) {
    if (/auto|scroll/.test(getComputedStyle(up).overflowY)) return up;
  }
  return null;
}

/**
 * The rows of `all` to draw now. `table` names what is shown: only another one (another report, or another sort) starts
 * over at the top, and the same one drawn again keeps what was drawn. `tail` is the element after the rows: as it nears
 * the window, more are drawn. Called while a component starts, which its effects belong to.
 */
export function lazyRows<T>(all: () => T[], table: () => string, tail: () => HTMLElement | undefined) {
  let drawn = $state(FIRST_ROWS);
  const total = $derived(all().length);
  const rows = $derived(drawn >= total ? all() : all().slice(0, drawn));
  let drawnFor = '';
  $effect.pre(() => {
    const now = table();
    untrack(() => {
      if (now !== drawnFor) {
        drawnFor = now;
        drawn = FIRST_ROWS;
      }
    });
  });
  const more = () => (drawn = Math.min(total, drawn + MORE_ROWS));
  $effect(() => {
    void drawn; // watched again after each step, so a window still not filled asks for the next
    const el = tail();
    if (!el || typeof IntersectionObserver === 'undefined') return;
    const watch = new IntersectionObserver((seen) => seen.some((s) => s.isIntersecting) && more(), {
      root: scroller(el),
      rootMargin: '1200px 0px',
    });
    watch.observe(el);
    return () => watch.disconnect();
  });
  return {
    get drawn() {
      return drawn;
    },
    get total() {
      return total;
    },
    get rows() {
      return rows;
    },
    /** Draw the next {MORE_ROWS}. */
    more,
    /** Draw every row. */
    all: () => (drawn = total),
  };
}
