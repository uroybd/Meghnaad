import { describe, expect, it } from 'vitest';
import { cell, rowClass } from './format';
import type { Column, Row } from './types';

const NOW = 1_791_376_200; // 2026-10-07T12:30:00Z
const DAY = 86400;

function row(over: Partial<Row> = {}): Row {
  return {
    uuid: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee', status: 'pending', description: 'Buy milk',
    project: null, priority: null, tags: [], annotations: [], entry: NOW - 3 * DAY, modified: null,
    start: null, end: null, due: null, wait: null, scheduled: null, until: null, depends: [],
    blocked: false, blocking: false, recur: null, parent: null, extra: {},
    urgency: 4.5, id: 3, virtual_tags: [], orphans: [], active_seconds: null, sessions: [], ...over,
  };
}

function col(spec: string, kind: Column['kind'] = 'string'): Column {
  const [name, format] = spec.split('.');
  return { spec, name, format: format ?? null, label: name, kind };
}

const ctx = { now: NOW, tz: 0 };

describe('cell', () => {
  it('shows the id, or a dimmed short uuid when there is none', () => {
    expect(cell(col('id'), row(), ctx).text).toBe('3');
    expect(cell(col('id'), row({ id: null }), ctx)).toEqual({ text: 'aaaaaaaa', cls: 'dim' });
  });

  it('formats dates: absolute (time only if present), relative, age, remaining', () => {
    const due = NOW + 2 * DAY; // exactly 12:30
    expect(cell(col('due'), row({ due }), ctx).text).toBe('2026-10-09 12:30');
    expect(cell(col('due'), row({ due: due - 12 * 3600 - 30 * 60 }), ctx).text).toBe('2026-10-09');
    expect(cell(col('due.relative'), row({ due }), ctx).text).toBe('2d');
    expect(cell(col('due.relative'), row({ due: NOW - 3 * DAY }), ctx).text).toBe('-3d');
    expect(cell(col('due.remaining'), row({ due: NOW - DAY }), ctx).text).toBe('');
    expect(cell(col('entry.age'), row(), ctx).text).toBe('3d');
    expect(cell(col('due'), row(), ctx).text).toBe('');
  });

  it('marks an overdue due date', () => {
    const r = row({ due: NOW - DAY, virtual_tags: ['OVERDUE'] });
    expect(cell(col('due'), r, ctx).cls).toBe('overdue');
  });

  it('start.active is a star', () => {
    expect(cell(col('start.active'), row({ start: NOW }), ctx).text).toBe('*');
    expect(cell(col('start.active'), row(), ctx).text).toBe('');
  });

  it('description formats', () => {
    const r = row({ description: 'x'.repeat(60), annotations: [{ entry: NOW, text: 'note' }] });
    expect(cell(col('description'), r, ctx).lines).toEqual(['2026-10-07 12:30 note']);
    expect(cell(col('description.desc'), r, ctx).lines).toBeUndefined();
    expect(cell(col('description.truncated'), r, ctx).text).toHaveLength(40);
    expect(cell(col('description.count'), r, ctx).text.endsWith(' [1]')).toBe(true);
    expect(cell(col('description.oneline'), r, ctx).text.endsWith(' note')).toBe(true);
  });

  it('project, tags, depends, status, priority, urgency', () => {
    expect(cell(col('project.parent'), row({ project: 'a.b.c' }), ctx).text).toBe('a.b');
    expect(cell(col('project.indented'), row({ project: 'a.b' }), ctx).text).toBe('  b');
    expect(cell(col('tags'), row({ tags: ['x', 'y'] }), ctx).text).toBe('x y');
    expect(cell(col('tags.count'), row({ tags: ['x', 'y'] }), ctx).text).toBe('[2]');
    expect(cell(col('depends.indicator'), row({ depends: ['u'] }), ctx).text).toBe('D');
    expect(cell(col('status.short'), row(), ctx).text).toBe('P');
    expect(cell(col('status.short'), row({ virtual_tags: ['WAITING'] }), ctx).text).toBe('W');
    expect(cell(col('priority'), row({ priority: 'H' }), ctx).cls).toBe('pri-h');
    expect(cell(col('urgency'), row(), ctx).text).toBe('4.50');
  });

  it('UDAs render from extra, dates as dates, and unset as empty', () => {
    const r = row({ extra: { estimate: 'big', ship: String(NOW + DAY), legacy: 'old' } });
    expect(cell(col('estimate'), r, ctx).text).toBe('big');
    expect(cell(col('ship', 'date'), r, ctx).text).toBe('2026-10-08 12:30');
    expect(cell(col('ship.relative', 'date'), r, ctx).text).toBe('1d');
    expect(cell(col('missing'), r, ctx).text).toBe('');
    // Orphans render too (read-only is enforced where editing happens).
    expect(cell(col('legacy'), r, ctx).text).toBe('old');
  });
});

describe('rowClass', () => {
  it('flags state', () => {
    expect(rowClass(row({ status: 'completed' }))).toBe('done');
    expect(rowClass(row({ start: NOW, blocked: true }))).toBe('blocked active');
    expect(rowClass(row())).toBe('');
  });
});


describe('journal markers in lists', () => {
  const J = { start: 'Started task', stop: 'Stopped task' };
  const withNotes = row({
    annotations: [
      { entry: NOW, text: 'Started task' },
      { entry: NOW + 5, text: 'call Sam first' },
      { entry: NOW + 9, text: 'Stopped task' },
    ],
  });

  it('are left out of the annotation lines under a description', () => {
    expect(cell(col('description'), withNotes, { ...ctx, journal: J }).lines).toEqual(['2026-10-07 12:30 call Sam first']);
    // Without journalling they are ordinary notes and show.
    expect(cell(col('description'), withNotes, ctx).lines).toHaveLength(3);
  });

  it('do not inflate the count or the one-line form', () => {
    expect(cell(col('description.count'), withNotes, { ...ctx, journal: J }).text).toBe('Buy milk [1]');
    expect(cell(col('description.oneline'), withNotes, { ...ctx, journal: J }).text).toBe('Buy milk call Sam first');
  });

  it('a task with only markers shows no notes at all', () => {
    const only = row({ annotations: [{ entry: NOW, text: 'Started task' }, { entry: NOW + 5, text: 'Stopped task' }] });
    expect(cell(col('description'), only, { ...ctx, journal: J }).lines).toBeUndefined();
  });
});
