// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import RepeatMark from './RepeatMark.svelte';

let app: ReturnType<typeof mount> | null = null;
function clear() {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
}
afterEach(clear);

const draw = (row: { status: string; recur: string | null; iter?: string; iter_count?: string }) => {
  app = mount(RepeatMark, { target: document.body, props: { row } });
  flushSync();
  return document.querySelector<HTMLElement>('.repeat-mark');
};

describe('the mark after a repeating task’s title', () => {
  it('is the repeat icon for a recurring task, its template or an instance', () => {
    const m = draw({ status: 'recurring', recur: 'weekly' });
    expect(m?.dataset.kind).toBe('recurring');
    expect(m?.title).toMatch(/^Recurring task: repeats/);
    clear();
    expect(draw({ status: 'pending', recur: 'weekly' })?.title).toMatch(/^Repeats/);
  });

  it('is the iteration icon for an iterative task, with its rule and number', () => {
    const m = draw({ status: 'iterative', recur: null, iter: 'monthly', iter_count: '3' });
    expect(m?.dataset.kind).toBe('iterative');
    expect(m?.title).toBe('Iterative: done, it makes the next (monthly); number 3');
    expect(m?.querySelector('svg')).not.toBeNull();
  });

  it('is nothing for a task that doesn’t repeat, or a completed one of a series', () => {
    expect(draw({ status: 'pending', recur: null })).toBeNull();
    clear();
    expect(draw({ status: 'completed', recur: null, iter_count: '2' })).toBeNull();
  });
});
