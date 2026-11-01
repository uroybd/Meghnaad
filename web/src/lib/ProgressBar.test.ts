// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import ProgressBar from './ProgressBar.svelte';

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

const draw = (value: number, range: { min: number; max: number }) => {
  app = mount(ProgressBar, { target: document.body, props: { value, text: String(value), range, label: 'Progress' } });
  flushSync();
  return document.body;
};

describe('ProgressBar', () => {
  it('fills to how far the value is between min and max, and says so to a screen reader', () => {
    const body = draw(30, { min: 10, max: 50 });
    const bar = body.querySelector('[role=progressbar]')!;
    expect(bar.getAttribute('aria-label')).toBe('Progress');
    expect(bar.getAttribute('aria-valuemin')).toBe('10');
    expect(bar.getAttribute('aria-valuemax')).toBe('50');
    expect(bar.getAttribute('aria-valuenow')).toBe('30');
    expect(bar.getAttribute('aria-valuetext')).toBe('30');
    expect(body.querySelector<HTMLElement>('.fill')?.style.width).toBe('50%');
  });

  it('holds a value outside the range at the end of the bar', () => {
    const body = draw(250, { min: 0, max: 100 });
    expect(body.querySelector<HTMLElement>('.fill')?.style.width).toBe('100%');
    expect(body.querySelector('.fill.full')).not.toBeNull();
    expect(body.querySelector('[role=progressbar]')?.getAttribute('aria-valuenow')).toBe('100');
  });
});
