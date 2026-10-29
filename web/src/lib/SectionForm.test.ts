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

  it('each can be moved up or down, the first being at the bottom of a stack, and the ends cannot go further', () => {
    const changed = vi.fn();
    app = mount(SectionForm, { target: document.body, props: { section, onchange: changed } });
    flushSync();
    const button = (label: string) => document.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`)!;
    expect(button('Move series 1 up').disabled).toBe(true);
    expect(button('Move series 3 down').disabled).toBe(true);
    button('Move series 2 down').click();
    expect(changed.mock.calls[0][0].series.map((x: { label: string }) => x.label)).toEqual(['One', 'Three', 'Two']);
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

describe('the rows of a heatmap', () => {
  const heat: Section = {
    ...section,
    type: 'heatmap',
    series: undefined,
    by: { time: { field: 'end', step: 'week' } },
    rows: { time: { field: 'end', part: 'weekday' } },
  };
  const rowsSelect = () =>
    [...document.querySelectorAll<HTMLSelectElement>('select')].find((x) =>
      [...x.options].some((o) => o.value === 'monthday'),
    )!;

  it('are a part of a date or the value of an attribute, and none is a row for each series', () => {
    const changed = vi.fn();
    app = mount(SectionForm, { target: document.body, props: { section: heat, onchange: changed } });
    flushSync();
    const select = rowsSelect();
    expect(select.value).toBe('weekday');
    expect([...select.options].map((o) => o.value)).toEqual(['', 'weekday', 'hour', 'monthday', 'month', 'field']);
    const pick = (value: string) => {
      select.value = value;
      select.dispatchEvent(new Event('change', { bubbles: true }));
    };
    pick('hour');
    expect(changed.mock.calls[0][0].rows).toEqual({ time: { field: 'end', part: 'hour' } });
    pick('field');
    expect(changed.mock.calls[1][0].rows).toEqual({ field: 'project' });
    pick('');
    expect(changed.mock.calls[2][0].rows).toBeUndefined();
  });

  it('can be stacked with a row for each series too: the series are then one row', () => {
    const changed = vi.fn();
    const older = { ...heat, rows: undefined, series: [{ label: 'A' }, { label: 'B' }] };
    app = mount(SectionForm, { target: document.body, props: { section: older, onchange: changed } });
    flushSync();
    const box = [...document.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')].find((b) =>
      b.parentElement?.textContent?.includes('Stacked'),
    )!;
    expect(box.disabled).toBe(false);
    box.checked = true;
    box.dispatchEvent(new Event('change', { bubbles: true }));
    expect(changed.mock.calls[0][0].stacked).toBe(true);
  });

  it('can be stacked now, and a section that is not a heatmap has none', () => {
    const changed = vi.fn();
    const two = { ...heat, series: [{ label: 'A' }, { label: 'B' }] };
    app = mount(SectionForm, { target: document.body, props: { section: two, onchange: changed } });
    flushSync();
    const box = [...document.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')].find((b) =>
      b.parentElement?.textContent?.includes('Stacked'),
    )!;
    box.checked = true;
    box.dispatchEvent(new Event('change', { bubbles: true }));
    expect(changed.mock.calls[0][0].stacked).toBe(true);
    // Switching to another kind of chart takes the rows away, which only a heatmap may have.
    const type = [...document.querySelectorAll<HTMLSelectElement>('select')].find((x) => x.value === 'heatmap')!;
    type.value = 'bar';
    type.dispatchEvent(new Event('change', { bubbles: true }));
    expect(changed.mock.calls[1][0]).toMatchObject({ type: 'bar', rows: undefined });
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

  it('covers a range of the board or dates of its own, and keeps the range when the date or step changes', () => {
    const changed = vi.fn();
    const ranges = [{ name: 'sprint', start: 'sow', end: 'eow' }];
    app = mount(SectionForm, { target: document.body, props: { section: dated, onchange: changed, ranges } });
    flushSync();
    const over = [...document.querySelectorAll<HTMLSelectElement>('select')].find((x) => x.value === '')!;
    expect([...over.options].map((o) => o.value)).toEqual(['', 'range:sprint', 'own']);
    expect(document.querySelector('input[placeholder="sow, 2026-10-01"]')).toBeNull();
    const pick = (value: string) => {
      over.value = value;
      over.dispatchEvent(new Event('change', { bubbles: true }));
    };
    pick('range:sprint');
    expect(changed.mock.calls[0][0].by).toEqual({ time: { field: 'end', step: 'week', range: 'sprint' } });
    pick('own');
    expect(changed.mock.calls[1][0].by.time.range).toEqual({ start: 'sow', end: 'eow' });
    pick('');
    expect(changed.mock.calls[2][0].by).toEqual({ time: { field: 'end', step: 'week' } });
  });

  it('has the dates of an own range typed in, and changing the step leaves the range', () => {
    const changed = vi.fn();
    const own: Section = {
      ...dated,
      by: { time: { field: 'end', step: 'week', range: { start: '2026-10-01', end: 'eom' } } },
    };
    app = mount(SectionForm, { target: document.body, props: { section: own, onchange: changed } });
    flushSync();
    const to = document.querySelector<HTMLInputElement>('input[placeholder="eow, 2026-10-31"]')!;
    expect(to.value).toBe('eom');
    to.value = 'eoy';
    to.dispatchEvent(new Event('input', { bubbles: true }));
    expect(changed.mock.calls[0][0].by.time.range).toEqual({ start: '2026-10-01', end: 'eoy' });
    const select = per();
    select.value = 'month';
    select.dispatchEvent(new Event('change', { bubbles: true }));
    expect(changed.mock.calls[1][0].by.time).toEqual({
      field: 'end',
      step: 'month',
      range: { start: '2026-10-01', end: 'eom' },
    });
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
