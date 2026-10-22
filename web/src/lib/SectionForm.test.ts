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

describe('grouping a chart by a date', () => {
  const dated: Section = { ...section, series: undefined, by: { time: { field: 'end', step: 'week' } } };
  const per = () => [...document.querySelectorAll<HTMLSelectElement>('select')].find((x) => x.value === 'week')!;

  it('offers the largest group of a range, as it is now', () => {
    const changed = vi.fn();
    const ranges = [{ name: 'daterange', start: 'sow', end: 'eow' }];
    app = mount(SectionForm, { target: document.body, props: { section: dated, onchange: changed, ranges } });
    flushSync();
    const option = [...per().options].find((o) => o.value === '$daterange.lg_group.name')!;
    expect(option.textContent).toBe('Largest group (currently day)');
    const select = per();
    select.value = option.value;
    select.dispatchEvent(new Event('change', { bubbles: true }));
    expect(changed.mock.calls[0][0].by).toEqual({ time: { field: 'end', step: '$daterange.lg_group.name' } });
  });

  it('says whose group it is when there are several ranges, and offers none without ranges', () => {
    const ranges = [
      { name: 'sprint_week', start: 'som', end: 'eom' },
      { name: 'later', start: 'soonish', end: 'eom' },
    ];
    app = mount(SectionForm, { target: document.body, props: { section: dated, onchange: vi.fn(), ranges } });
    flushSync();
    const texts = [...per().options].map((o) => o.textContent);
    expect(texts).toContain('Largest group of Sprint week (currently week)');
    expect(texts).toContain('Largest group of Later (currently not a date range yet)');
    unmount(app);
    document.body.innerHTML = '';
    app = mount(SectionForm, { target: document.body, props: { section: dated, onchange: vi.fn() } });
    flushSync();
    expect([...per().options].map((o) => o.value)).toEqual(['day', 'week', 'month', 'quarter', 'year']);
  });

  it('keeps what is typed in Keep: a number as a number, a count as text', () => {
    const changed = vi.fn();
    app = mount(SectionForm, { target: document.body, props: { section: dated, onchange: changed } });
    flushSync();
    const keep = document.querySelector<HTMLInputElement>('input[placeholder^="all"]')!;
    keep.value = '12';
    keep.dispatchEvent(new Event('input', { bubbles: true }));
    expect(changed.mock.calls.at(-1)![0].limit).toBe(12);
    keep.value = ' $daterange.lg_group.count ';
    keep.dispatchEvent(new Event('input', { bubbles: true }));
    expect(changed.mock.calls.at(-1)![0].limit).toBe('$daterange.lg_group.count');
    keep.value = '';
    keep.dispatchEvent(new Event('input', { bubbles: true }));
    expect(changed.mock.calls.at(-1)![0].limit).toBeUndefined();
  });
});
