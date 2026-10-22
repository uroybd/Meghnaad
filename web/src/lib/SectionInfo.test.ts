// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import type { Chart, Section, SectionData } from './boards';
import SectionInfo from './SectionInfo.svelte';
import { store } from './store.svelte';

const sec = (extra: Partial<Section>): Section => ({
  id: 's',
  title: 'Points',
  x: 0,
  y: 0,
  w: 4,
  h: 3,
  type: 'number',
  measure: { fn: 'count' },
  ...extra,
});
const tasks = (n: number) =>
  JSON.stringify(
    Array.from({ length: n }, (_, i) => ({
      id: i + 1,
      uuid: `00000000-0000-0000-0000-${String(i + 1).padStart(12, '0')}`,
      description: `Task ${i + 1}`,
      project: 'Home',
      status: 'pending',
      tags: ['a', 'b'],
      points: i,
      due: '20261007T123000Z',
    })),
  );

let app: ReturnType<typeof mount> | null = null;
const closed = vi.fn();
beforeEach(() => {
  closed.mockClear();
  // jsdom has no modal dialogs.
  HTMLDialogElement.prototype.showModal = function () {
    this.setAttribute('open', '');
  };
  HTMLDialogElement.prototype.close = function () {
    this.removeAttribute('open');
    this.dispatchEvent(new Event('close'));
  };
  store.view = 'boards';
  store.filter = '';
});
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

const draw = (section: Section, data: SectionData) => {
  app = mount(SectionInfo, { target: document.body, props: { section, data, onclose: closed } });
  flushSync();
};
const text = () => (document.querySelector('dialog')!.textContent ?? '').replace(/\s+/g, ' ');
const rows = () => [...document.querySelectorAll('.tasks tbody tr')];

describe('the context a card ran in', () => {
  const data = {
    kind: 'answer',
    answer: { total: 1, tasks: 1, unit: 'number' },
    sources: [],
  } as unknown as SectionData;
  it('is the console’s unless the filter names one', () => {
    store.config = { config: { settings: {}, active_context: 'work', contexts: {} } } as never;
    draw(sec({}), { ...data, sources: [{ label: 'x', filter: '( project:Home )', text: '[]' }] } as SectionData);
    expect(text()).toContain('Context work (the console’s)');
    unmount(app!);
    draw(sec({}), {
      ...data,
      sources: [{ label: 'x', filter: '( rc.context=this_week ) ( project:Home )', text: '[]' }],
    } as SectionData);
    expect(text()).toContain('Context this_week (set in the filter)');
    unmount(app!);
    draw(sec({}), { ...data, sources: [{ label: 'x', filter: 'rc.context=none', text: '[]' }] } as SectionData);
    expect(text()).toContain('Context none (set in the filter)');
    store.config = null;
  });
});

describe('the info of a number', () => {
  const data: SectionData = {
    kind: 'answer',
    answer: { unit: 'number', total: 6, tasks: 4, groups: [] },
    sources: [{ label: 'Points', filter: '( project:Home )', text: tasks(4) }],
  };

  it('opens as a dialog and says what it measures, its filter and its figure', () => {
    draw(sec({ measure: { fn: 'sum', field: 'points' } }), data);
    expect(document.querySelector('dialog')?.hasAttribute('open')).toBe(true);
    expect(text()).toContain('Points · Number');
    expect(text()).toContain('Sum of points');
    expect(document.querySelector('.src code')?.textContent).toBe('( project:Home )');
    expect(document.querySelector('.big')?.textContent).toBe('6');
    expect(text()).toContain('from 4 tasks');
  });

  it('lists the tasks the filter got, with the columns it measures, and dates as the page writes them', () => {
    draw(sec({ measure: { fn: 'sum', field: 'points' } }), data);
    expect(rows()).toHaveLength(4);
    const heads = [...document.querySelectorAll('.tasks thead th')].map((h) => h.textContent);
    expect(heads).toEqual(['id', 'description', 'project', 'status', 'tags', 'points']);
    const first = [...rows()[0].querySelectorAll('td')].map((c) => c.textContent);
    expect(first).toEqual(['1', 'Task 1', 'Home', 'pending', 'a, b', '0']);
    expect(text()).toContain('4 tasks');
  });

  it('opens a task from its description, closing the panel so the drawer is not hidden behind it', () => {
    store.detail = null;
    draw(sec({}), data);
    const link = rows()[1].querySelector<HTMLButtonElement>('td button.link')!;
    expect(link.textContent).toBe('Task 2');
    link.click();
    flushSync();
    expect((store.detail as { uuid: string } | null)?.uuid).toBe('00000000-0000-0000-0000-000000000002');
    expect(document.querySelector('dialog')!.hasAttribute('open')).toBe(false);
    expect(closed).toHaveBeenCalled();
  });

  it('shows a date column when the section groups by one', () => {
    draw(sec({ by: { time: { field: 'due', step: 'day' } } }), data);
    const cells = [...rows()[0].querySelectorAll('td')].map((c) => c.textContent);
    expect(cells.at(-1)).toMatch(/2026-10-07/);
  });

  it('lists at most 200 and says how many more', () => {
    draw(sec({}), { ...data, sources: [{ label: 'x', filter: '', text: tasks(230) }] } as SectionData);
    expect(rows()).toHaveLength(200);
    expect(text()).toContain('and 30 more');
    expect(document.querySelector('.src code')?.textContent).toBe('(every task)');
  });

  it('opens the tasks on the Tasks page with the filter, and closes', () => {
    draw(sec({}), data);
    document.querySelector<HTMLButtonElement>('.src button')!.click();
    expect(store.filter).toBe('( project:Home )');
    expect(store.view).toBe('tasks');
    expect(closed).toHaveBeenCalled();
  });

  it('closes with the close button', () => {
    draw(sec({}), data);
    document.querySelector<HTMLButtonElement>('button[aria-label=Close]')!.click();
    expect(closed).toHaveBeenCalledTimes(1);
  });
});

describe('the info of a chart', () => {
  const chart: Chart = {
    unit: 'number',
    labels: [
      { key: '1', label: 'Home' },
      { key: '2', label: 'Work' },
    ],
    datasets: [
      { label: 'Succeeded', data: [3, 1], counts: [3, 1], total: 4, tasks: 4 },
      { label: 'Failed', data: [0, null], counts: [0, 0], total: 0, tasks: 0 },
    ],
  };
  const section = sec({
    type: 'bar',
    stacked: true,
    by: { field: 'project' },
    series: [
      { label: 'Succeeded', filter: 'outcome:success' },
      { label: 'Failed', filter: 'outcome:failed', span: { from: 'end' } },
    ],
  });
  const data: SectionData = {
    kind: 'chart',
    chart,
    limit: null,
    sources: [
      { label: 'Succeeded', filter: '( outcome:success )', text: tasks(4) },
      { label: 'Failed', filter: '( outcome:failed )', text: '[]' },
    ],
  };

  it('shows the axis: each label with each series’ value and the tasks behind it, and the totals', () => {
    draw(section, data);
    const table = document.querySelector('.scroll:not(.tasks) table')!;
    expect([...table.querySelectorAll('thead th')].map((h) => h.textContent)).toEqual([
      'project',
      'Succeeded',
      'Failed',
    ]);
    const body = [...table.querySelectorAll('tbody tr')].map((r) =>
      [...r.querySelectorAll('td')].map((c) => c.textContent?.replace(/\s+/g, '')),
    );
    // Largest first, as the chart has it; nothing to measure is a dash.
    expect(body).toEqual([
      ['Home', '3(3)', '0(0)'],
      ['Work', '1(1)', '–(0)'],
    ]);
    expect(table.querySelector('tfoot')?.textContent?.replace(/\s+/g, '')).toBe('Overallthetasks4(4)0(0)');
    expect(text()).toContain('stacked');
  });

  it('gives each series its filter and its span, and its own list of tasks', () => {
    draw(section, data);
    const lines = [...document.querySelectorAll('.sources .src')].map((l) =>
      l.textContent?.replace(/\s+/g, ' ').trim(),
    );
    expect(lines[0]).toContain('Succeeded ( outcome:success )');
    expect(lines[1]).toContain('Failed ( outcome:failed )');
    expect(lines[1]).toContain('open at the end of each period, from end');
    expect(document.querySelectorAll('h5').length).toBe(2);
    expect(rows()).toHaveLength(4);
  });
});

describe('the info of a table and of a card that failed', () => {
  it('says the report, the filter and how many tasks it matched', () => {
    const result = { kind: 'report', matched: 7, rows: [], columns: [] } as unknown as SectionData & object;
    draw(sec({ type: 'table', report: 'list' }), { kind: 'table', result: result as never, filter: '( +work )' });
    expect(text()).toContain('list');
    expect(document.querySelector('.src code')?.textContent).toBe('( +work )');
    expect(text()).toContain('matched 7 tasks');
  });

  it('says what went wrong instead of a filter', () => {
    draw(sec({}), { kind: 'error', message: "'x' is not a number" });
    expect(document.querySelector('[role=alert]')?.textContent).toBe("'x' is not a number");
    expect(text()).not.toContain('Filter');
  });
});
