import { describe, expect, it } from 'vitest';
import { formatCells, projectSegments, rowClass, urgencyLevel, type Ctx } from './format';
import type { Column, Row } from './types';

const NOW = 1_791_376_200; // 2026-10-07T12:30:00Z
const DAY = 86400;

function row(over: Partial<Row> = {}): Row {
  return {
    uuid: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
    status: 'pending',
    description: 'Buy milk',
    project: null,
    priority: null,
    tags: [],
    annotations: [],
    entry: NOW - 3 * DAY,
    modified: null,
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
    urgency: 4.5,
    id: 3,
    virtual_tags: [],
    orphans: [],
    active_seconds: null,
    sessions: [],
    ...over,
  };
}

function col(spec: string, kind: Column['kind'] = 'string'): Column {
  const [name, format] = spec.split('.');
  return { spec, name, format: format ?? null, label: name, kind };
}

const ctx: Ctx = { now: NOW, tz: 0 };

// What each format shows is the engine's (`tc_core::cells`, tested there); these check that the page hands it the table and
// gets the cells back as it draws them.
const cells = (cols: Column[], rows: Row[], c = ctx) => formatCells(cols, rows, c);
const cell = (c: Column, r: Row, x = ctx) => cells([c], [r], x)[0][0];

describe('formatCells', () => {
  it('answers a cell for every row and column, in order', () => {
    const out = cells([col('id'), col('description')], [row(), row({ id: null, description: 'Other' })]);
    expect(out.map((r) => r.map((c) => c.text))).toEqual([
      ['3', 'Buy milk'],
      ['aaaaaaaa', 'Other'],
    ]);
    expect(cells([col('id')], [])).toEqual([]);
  });

  it('keeps what a cell carries besides its text', () => {
    expect(cell(col('id'), row({ id: null }))).toEqual({ text: 'aaaaaaaa', cls: 'dim', uuid: row().uuid });
    expect(cell(col('project'), row({ project: 'Home.Kitchen' })).segments).toEqual(projectSegments('Home.Kitchen'));
    expect(cell(col('tags'), row({ tags: ['a', 'b'] })).chips).toEqual(['a', 'b']);
    expect(cell(col('description'), row({ annotations: [{ entry: NOW, text: 'n' }] })).notes).toEqual([
      { at: '2026-10-07 12:30', text: 'n', entry: NOW },
    ]);
  });

  it('carries the Markdown of a note and of a string UDA, and only when there is any', () => {
    const udas = {
      plan: { name: 'plan', type: 'string' as const, label: null, values: [], default: null, indicator: null },
    };
    const r = row({
      annotations: [
        { entry: NOW, text: '- [ ] milk' },
        { entry: NOW + 1, text: 'plain words' },
      ],
      extra: { plan: 'do **this**' },
    });
    const notes = cell(col('description'), r).notes!;
    expect(notes[0].blocks?.[0].t).toBe('ul');
    expect(notes[1].blocks).toBeUndefined();
    const plan = cell(col('plan'), r, { ...ctx, udas });
    expect(plan.text).toBe('do **this**');
    expect(plan.blocks?.[0].c?.map((n) => n.t)).toEqual(['text', 'b']);
    expect(cell(col('plan'), row({ extra: { plan: 'words' } }), { ...ctx, udas }).blocks).toBeUndefined();
  });

  it('writes dates in the pattern and the zone it is given', () => {
    const due = NOW + 2 * DAY;
    expect(cell(col('due'), row({ due })).text).toBe('2026-10-09 12:30');
    const dates = { report: { pattern: 'D/M/Y H:N', weekstart: 1 as const } };
    expect(cell(col('due'), row({ due }), { ...ctx, tz: 3600, dates }).text).toBe('09/10/2026 13:30');
    expect(cell(col('due.relative'), row({ due })).text).toBe('2d');
  });

  it('takes the udas a cell needs', () => {
    const udas = {
      ship: { name: 'ship', type: 'date' as const, label: null, values: [], default: null, indicator: null },
    };
    expect(cell(col('ship', 'date'), row({ extra: { ship: String(NOW) } }), { ...ctx, udas }).text).toBe(
      '2026-10-07 12:30',
    );
  });

  it('uses the browser zone, at the offset each moment had, when none is given', () => {
    const jan = Date.UTC(2026, 0, 15, 12) / 1000;
    const jul = Date.UTC(2026, 6, 15, 12) / 1000;
    const local = (t: number) => {
      const d = new Date(t * 1000);
      const p = (n: number) => String(n).padStart(2, '0');
      return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
    };
    const out = cells([col('due')], [row({ due: jan }), row({ due: jul })], { now: NOW });
    expect(out.map((r) => r[0].text)).toEqual([local(jan), local(jul)].map((s) => s.replace(/ 00:00$/, '')));
  });
});

describe('rowClass', () => {
  it('flags state', () => {
    expect(rowClass(row({ status: 'completed' }))).toBe('done completed');
    expect(rowClass(row({ status: 'deleted' }))).toBe('done deleted');
    expect(rowClass(row({ start: NOW, blocked: true }))).toBe('blocked active');
    expect(rowClass(row())).toBe('');
  });
});

describe('urgencyLevel', () => {
  it('has thresholds at 5, 10 and 15', () => {
    expect([4.99, 5, 10, 15].map(urgencyLevel)).toEqual(['low', 'mid', 'high', 'crit']);
  });
});
