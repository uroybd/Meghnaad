// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import ReportTable from './ReportTable.svelte';
import { store } from './store.svelte';
import type { ConfigResponse, ReportResult, Row } from './types';

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  store.config = null;
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

function draw(rows: Row[], colors: Record<string, { fg?: number; bold?: boolean }>) {
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
  app = mount(ReportTable, { target: document.body, props: { result } });
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
