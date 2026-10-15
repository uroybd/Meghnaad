// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import ReportTable from './ReportTable.svelte';
import { selection } from './selection.svelte';
import { store } from './store.svelte';
import type { ConfigResponse, ReportResult, Row } from './types';

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  store.config = null;
  store.detail = null;
  selection.clear();
});

const row = (description: string, priority: string | null, style?: Row['style']): Row =>
  ({
    uuid: description,
    status: 'pending',
    description,
    project: null,
    priority,
    tags: [],
    annotations: [],
    entry: 1,
    modified: 1,
    start: null,
    end: null,
    due: null,
    wait: null,
    scheduled: null,
    until: null,
    depends: [],
    blocked: false,
    blocking: false,
    recur: null,
    parent: null,
    mask: null,
    imask: null,
    extra: {},
    urgency: 1,
    id: 1,
    virtual_tags: [],
    orphans: [],
    active_seconds: null,
    sessions: [],
    style,
  }) as Row;

function draw(rows: Row[], colors: Record<string, { fg?: number; bold?: boolean }> = {}, selectable = false) {
  store.config = { colors, config: { udas: {}, settings: {}, reports: {} } } as unknown as ConfigResponse;
  const result: ReportResult = {
    kind: 'report',
    report: 'next',
    description: null,
    columns: [
      { spec: 'description', name: 'description', format: null, label: 'Description', kind: 'description' },
      { spec: 'priority', name: 'priority', format: null, label: 'P', kind: 'priority' },
    ],
    rows,
    breaks: rows.map(() => false),
    matched: rows.length,
    sort: null,
  };
  app = mount(ReportTable, { target: document.body, props: { result, selectable } });
  flushSync();
}
const cellOf = (description: string, column: string) =>
  [...document.querySelectorAll('tbody tr')]
    .find((tr) => tr.textContent?.includes(description))!
    .querySelector<HTMLElement>(`td.${column}`)!;

describe('the priority cell', () => {
  it('has the colour of its own priority, whatever the rest of the row looks like', () => {
    // A row that is due soon has the white (7) row colour; the priority cell keeps its own.
    draw([row('soon', 'H', { fg: 7 }), row('plain', 'M'), row('none', null, { fg: 7 })], {
      'uda.priority.H': { fg: 1, bold: true },
      'uda.priority.M': { fg: 3 },
    });
    const h = cellOf('soon', 'priority');
    expect(h.style.color).not.toBe('');
    expect(h.style.fontWeight).toBe('700');
    // The rest of that row is not touched by it.
    expect(cellOf('soon', 'description').style.color).toBe('');
    // Medium has a colour, not bold; the colours differ between priorities.
    const m = cellOf('plain', 'priority');
    expect(m.style.color).not.toBe('');
    expect(m.style.fontWeight).toBe('');
    expect(m.style.color).not.toBe(h.style.color);
  });

  it('is left alone when there is no colour for the priority, or no priority', () => {
    draw([row('low', 'L'), row('none', null)], { 'uda.priority.H': { fg: 1 } });
    expect(cellOf('low', 'priority').style.color).toBe('');
    expect(cellOf('none', 'priority').style.color).toBe('');
  });
});

describe('selecting tasks', () => {
  const boxes = () => [...document.querySelectorAll<HTMLInputElement>('tbody td.sel input')];
  const all = () => document.querySelector<HTMLInputElement>('th.sel input')!;

  it('has no checkboxes unless asked', () => {
    draw([row('a', null), row('b', null)]);
    expect(document.querySelector('input[type="checkbox"]')).toBeNull();
  });

  it('has a checkbox on each row and one for all, and ticking selects the task', () => {
    draw([row('a', null), row('b', null), row('c', null)], {}, true);
    expect(boxes()).toHaveLength(3);
    boxes()[1].click();
    flushSync();
    expect(selection.uuids).toEqual(['b']);
    expect(document.querySelectorAll('tr.picked')).toHaveLength(1);
    // Some are ticked: the all-box is half ticked, not ticked.
    expect(all().indeterminate).toBe(true);
    expect(all().checked).toBe(false);
    expect(store.detail).toBeNull(); // the click did not also open the task
    all().click();
    flushSync();
    expect(selection.uuids.sort()).toEqual(['a', 'b', 'c']);
    expect(all().checked).toBe(true);
    expect(all().indeterminate).toBe(false);
    all().click();
    flushSync();
    expect(selection.count).toBe(0);
  });

  it('opens a task from its row as before', () => {
    draw([row('a', null), row('b', null)], {}, true);
    (document.querySelector('tbody tr td.description') as HTMLElement).click();
    expect(store.detail?.uuid).toBe('a');
    expect(selection.count).toBe(0);
  });
});

describe('what a card on a phone is made from', () => {
  const now = 1_791_799_200;

  it('names the state of each row and the column of each cell, so the card can be styled by them', () => {
    store.now = now;
    const rows = [row('overdue one', 'H'), row('later one', 'M'), row('started one', null), row('finished one', 'L')];
    rows[0].due = now - 86_400;
    rows[1].due = now + 86_400;
    rows[2].start = now - 60;
    rows[3].due = now - 86_400;
    rows[3].status = 'completed';
    draw(rows);
    const tr = (name: string) => [...document.querySelectorAll('tbody tr')].find((r) => r.textContent?.includes(name))!;
    expect(tr('overdue one').classList.contains('overdue')).toBe(true);
    expect(tr('overdue one').getAttribute('data-pri')).toBe('H');
    expect(tr('later one').classList.contains('overdue')).toBe(false);
    expect(tr('later one').getAttribute('data-pri')).toBe('M');
    expect(tr('started one').classList.contains('started')).toBe(true);
    expect(tr('started one').hasAttribute('data-pri')).toBe(false);
    // Finished tasks are not "overdue", whatever their due date was.
    expect(tr('finished one').classList.contains('overdue')).toBe(false);
    expect(document.querySelector('td.description')!.classList.contains('col-description')).toBe(true);
  });
});

describe('a column with nothing in it', () => {
  const drawWith = (rows: Row[], printEmpty?: boolean) => {
    store.config = { colors: {}, config: { udas: {}, settings: {}, reports: {} } } as unknown as ConfigResponse;
    const result: ReportResult = {
      kind: 'report',
      report: 'next',
      description: null,
      columns: [
        { spec: 'description', name: 'description', format: null, label: 'Description', kind: 'description' },
        { spec: 'priority', name: 'priority', format: null, label: 'P', kind: 'priority' },
      ],
      rows,
      breaks: rows.map(() => false),
      matched: rows.length,
      sort: null,
      ...(printEmpty ? { print_empty_columns: true } : {}),
    };
    app = mount(ReportTable, { target: document.body, props: { result } });
    flushSync();
    return [...document.querySelectorAll('thead th')]
      .map((th) => th.textContent?.trim())
      .filter((t) => t && t !== 'Actions');
  };

  it('is left out, as Taskwarrior leaves it out by default', () => {
    expect(drawWith([row('a', null), row('b', null)])).toEqual(['Description']);
    expect(document.querySelectorAll('tbody tr:first-child td.priority')).toHaveLength(0);
  });

  it('stays when any row has something in it', () => {
    expect(drawWith([row('a', null), row('b', 'H')])).toEqual(['Description', 'P']);
  });

  it('stays with print.empty.columns', () => {
    expect(drawWith([row('a', null)], true)).toEqual(['Description', 'P']);
  });
});
