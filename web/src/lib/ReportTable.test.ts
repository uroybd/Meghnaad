// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
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
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
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

describe('a table with no room for its buttons', () => {
  // jsdom lays nothing out: the table is as wide, and its box as narrow, as the test says.
  function squeeze(scroll: number, box: number) {
    vi.stubGlobal(
      'ResizeObserver',
      class {
        constructor(private cb: ResizeObserverCallback) {}
        observe() {
          this.cb([], this as unknown as ResizeObserver);
        }
        unobserve() {}
        disconnect() {}
      },
    );
    vi.stubGlobal('matchMedia', (q: string) => ({
      matches: q.includes('min-width'),
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }));
    vi.spyOn(HTMLElement.prototype, 'scrollWidth', 'get').mockReturnValue(scroll);
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(box);
    vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(120);
  }
  const buttons = () => [...document.querySelectorAll('td.actions button')].map((b) => b.getAttribute('aria-label'));

  it('shows one ⋯ button, and the others in its menu', async () => {
    squeeze(900, 500);
    draw([row('one', null)]);
    await Promise.resolve();
    flushSync();
    expect(buttons()).toEqual(['More actions']);
    document.querySelector<HTMLButtonElement>('td.actions button')!.click();
    await Promise.resolve();
    await Promise.resolve();
    flushSync();
    expect([...document.querySelectorAll('[role=menuitem]')].map((e) => e.textContent?.trim())).toEqual([
      'Done',
      'Start',
      'Delete',
    ]);
  });

  it('keeps the buttons when the table fits', async () => {
    squeeze(500, 500);
    draw([row('one', null)]);
    await Promise.resolve();
    flushSync();
    expect(buttons()).toEqual(['Done', 'Start', 'Delete']);
  });
});

describe('a task card on a phone', () => {
  const buttons = () => [...document.querySelectorAll('td.actions button')].map((b) => b.getAttribute('aria-label'));

  it('keeps one button, Done, and has the rest in its ⋯ menu', async () => {
    vi.stubGlobal('matchMedia', (q: string) => ({
      matches: q.includes('max-width: 760px'),
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }));
    draw([row('one', null)]);
    await Promise.resolve();
    flushSync();
    expect(buttons()).toEqual(['Done', 'More actions']);
    document.querySelector<HTMLButtonElement>('td.actions button[aria-haspopup=menu]')!.click();
    await Promise.resolve();
    await Promise.resolve();
    flushSync();
    expect([...document.querySelectorAll('[role=menuitem]')].map((e) => e.textContent?.trim())).toEqual([
      'Start',
      'Delete',
    ]);
  });
});

describe('a report of many tasks', () => {
  const many = (n: number) => Array.from({ length: n }, (_, i) => row(`task ${i}`, null));
  const drawn = () => document.querySelectorAll('tbody tr').length;
  const button = (text: RegExp) =>
    [...document.querySelectorAll<HTMLButtonElement>('button.more')].find((b) => text.test(b.textContent ?? ''));

  it('draws the top at once and says how many are shown, when there are more than a screenful of hundreds', () => {
    draw(many(650));
    expect(drawn()).toBe(200);
    expect(document.querySelector('.count')?.textContent).toMatch(/650 tasks\s+· 200 shown so far/);
  });

  it('draws more when asked, a step at a time or all of it', () => {
    draw(many(650));
    button(/Show 300 more/)!.click();
    flushSync();
    expect(drawn()).toBe(500);
    button(/Show all/)!.click();
    flushSync();
    expect(drawn()).toBe(650);
    // Nothing is left to ask for.
    expect(document.querySelector('button.more')).toBeNull();
    expect(document.querySelector('.count')?.textContent).not.toMatch(/shown so far/);
  });

  it('is all there when it is not many, with nothing to ask for', () => {
    draw(many(200));
    expect(drawn()).toBe(200);
    expect(document.querySelector('button.more')).toBeNull();
  });

  it('still selects every task of the report, drawn or not', () => {
    draw(many(650), {}, true);
    const all = document.querySelector<HTMLInputElement>('thead input[type=checkbox]')!;
    expect(all.getAttribute('aria-label')).toBe('Select all 650 tasks');
    all.click();
    flushSync();
    expect(selection.has('task 649')).toBe(true);
    expect(selection.has('task 0')).toBe(true);
  });

  it('draws as far as it asks for when the window is not yet full, as the end comes near', () => {
    // The browser tells when the end of the table is near; here, always, until nothing is left.
    let watched = 0;
    class Near {
      constructor(private fn: (seen: { isIntersecting: boolean }[]) => void) {}
      observe() {
        watched++;
        queueMicrotask(() => this.fn([{ isIntersecting: true }]));
      }
      disconnect() {}
    }
    vi.stubGlobal('IntersectionObserver', Near);
    draw(many(650));
    return Promise.resolve()
      .then(() => new Promise((r) => setTimeout(r, 20)))
      .then(() => {
        flushSync();
        expect(drawn()).toBe(650);
        expect(watched).toBeGreaterThan(1);
      });
  });
});
