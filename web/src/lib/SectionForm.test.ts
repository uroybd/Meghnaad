// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import type { Section } from './boards';
import SectionForm from './SectionForm.svelte';

const section: Section = {
  id: 's',
  title: 'Done',
  x: 0,
  y: 0,
  w: 6,
  h: 4,
  type: 'bar',
  measure: { fn: 'count' },
  by: { field: 'project' },
  series: [{ label: 'One' }, { label: 'Two', filter: 'outcome:failed' }, { label: 'Three' }],
};

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

describe('the series of a chart', () => {
  it('each has a labelled button that removes just that series', () => {
    const changed = vi.fn();
    app = mount(SectionForm, { target: document.body, props: { section, onchange: changed } });
    flushSync();
    const buttons = [...document.querySelectorAll<HTMLButtonElement>('button')].filter(
      (b) => b.textContent?.trim() === 'Remove series',
    );
    expect(buttons).toHaveLength(3);
    expect(buttons[1].getAttribute('aria-label')).toBe('Remove series 2');
    buttons[1].click();
    expect(changed).toHaveBeenCalledTimes(1);
    expect(changed.mock.calls[0][0].series.map((x: { label: string }) => x.label)).toEqual(['One', 'Three']);
  });

  it('removing the last one leaves a chart of the one measure', () => {
    const changed = vi.fn();
    const one = { ...section, series: [{ label: 'Only' }] };
    app = mount(SectionForm, { target: document.body, props: { section: one, onchange: changed } });
    flushSync();
    document.querySelector<HTMLButtonElement>('button[aria-label="Remove series 1"]')!.click();
    expect(changed.mock.calls[0][0].series).toBeUndefined();
  });
});
