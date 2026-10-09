import { describe, expect, it } from 'vitest';
import { cell, rowClass, urgencyLevel } from './format';
import type { Column, Row, UdaDef } from './types';

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

const ctx = { now: NOW, tz: 0 };

describe('cell', () => {
  it('shows the id, or a dimmed short uuid when there is none', () => {
    expect(cell(col('id'), row(), ctx).text).toBe('3');
    expect(cell(col('id'), row({ id: null }), ctx)).toEqual({ text: 'aaaaaaaa', cls: 'dim', uuid: row().uuid });
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

  it('start.active is a star', () => {
    expect(cell(col('start.active'), row({ start: NOW }), ctx).text).toBe('*');
    expect(cell(col('start.active'), row(), ctx).text).toBe('');
  });

  it('start.active shows only while the task is started and not ended', () => {
    expect(cell(col('start.active'), row({ start: NOW, end: NOW + 5 }), ctx).text).toBe('');
  });

  it('the indicator settings change what the indicator columns show', () => {
    const custom = { ...ctx, indicators: { active: '>>', tag: '#', dependency: 'DEP' } };
    expect(cell(col('start.active'), row({ start: NOW }), custom).text).toBe('>>');
    expect(cell(col('tags.indicator'), row({ tags: ['x'] }), custom).text).toBe('#');
    expect(cell(col('tags.indicator'), row(), custom).text).toBe('');
    expect(cell(col('depends.indicator'), row({ pending_deps: 1 }), custom).text).toBe('DEP');
    // Unset, they are Taskwarrior's: *, + and D.
    expect(cell(col('tags.indicator'), row({ tags: ['x'] }), ctx).text).toBe('+');
  });

  it('only dependencies still open count for the dependency columns', () => {
    // It depends on a task that has since been finished: nothing is holding it up.
    const done = row({ depends: ['u'] });
    expect(cell(col('depends.indicator'), done, ctx).text).toBe('');
    expect(cell(col('depends.count'), done, ctx).text).toBe('');
    const two = row({ depends: ['u', 'v', 'w'], pending_deps: 2 });
    expect(cell(col('depends.count'), two, ctx).text).toBe('[2]');
  });

  it('an id carries its uuid, for the copy tooltip', () => {
    const c = cell(col('id'), row(), ctx);
    expect(c.text).toBe('3');
    expect(c.uuid).toBe('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee');
  });

  it('a project is split into its parts', () => {
    const c = cell(col('project'), row({ project: 'Home.Kitchen.Sink' }), ctx);
    expect(c.text).toBe('Home.Kitchen.Sink');
    expect(c.segments?.map((s) => s.text)).toEqual(['Home', 'Kitchen', 'Sink']);
    // The parent format shows the path above the project, still in parts; no project, no parts.
    expect(cell(col('project.parent'), row({ project: 'Home.Kitchen' }), ctx).segments?.map((s) => s.text)).toEqual([
      'Home',
    ]);
    expect(cell(col('project'), row({ project: null }), ctx).segments).toBeUndefined();
  });

  it('tags are pills, except in the compact formats', () => {
    const r = row({ tags: ['errand', 'next'] });
    expect(cell(col('tags'), r, ctx).chips).toEqual(['errand', 'next']);
    expect(cell(col('tags'), r, ctx).text).toBe('errand next');
    expect(cell(col('tags'), row(), ctx).chips).toBeUndefined();
    expect(cell(col('tags.count'), r, ctx)).toEqual({ text: '[2]' });
    expect(cell(col('tags.indicator'), r, ctx)).toEqual({ text: '+' });
    expect(cell(col('tags.list'), r, ctx)).toEqual({ text: 'errand,next' });
  });

  it('urgency is coloured by how pressing it is', () => {
    expect([0, 4.99, 5, 9.99, 10, 14.99, 15, 40].map(urgencyLevel)).toEqual([
      'low',
      'low',
      'mid',
      'mid',
      'high',
      'high',
      'crit',
      'crit',
    ]);
    expect(urgencyLevel(-5)).toBe('low'); // blocked tasks go negative
    expect(cell(col('urgency', 'number'), row({ urgency: 16.2 }), ctx)).toEqual({ text: '16.20', cls: 'urg-crit' });
    expect(cell(col('urgency.integer', 'number'), row({ urgency: 6.4 }), ctx)).toEqual({ text: '6', cls: 'urg-mid' });
  });

  it('a multi-line string UDA keeps its lines in the table', () => {
    const udas: Record<string, UdaDef> = {
      notes: { name: 'notes', type: 'string', label: null, values: [], default: null, indicator: null },
    };
    const c = (value: string) => cell(col('notes'), row({ extra: { notes: value } }), { ...ctx, udas });
    expect(c('one\ntwo')).toEqual({ text: 'one\ntwo', cls: 'multiline' });
    expect(c('just one line')).toEqual({ text: 'just one line' });
  });

  it('description formats', () => {
    const r = row({ description: 'x'.repeat(60), annotations: [{ entry: NOW, text: 'note' }] });
    expect(cell(col('description'), r, ctx).lines).toEqual(['2026-10-07 12:30 note']);
    expect(cell(col('description.desc'), r, ctx).lines).toBeUndefined();
    expect(cell(col('description.truncated'), r, ctx).text).toHaveLength(40);
    expect(cell(col('description.count'), r, ctx).text.endsWith(' [1]')).toBe(true);
    expect(cell(col('description.oneline'), r, ctx).text).toBe(`${r.description} 2026-10-07 12:30 note`);
  });

  it('project, tags, depends, status, priority, urgency', () => {
    expect(cell(col('project.parent'), row({ project: 'a.b.c' }), ctx).text).toBe('a.b');
    expect(cell(col('project.indented'), row({ project: 'a.b' }), ctx).text).toBe('  b');
    expect(cell(col('tags'), row({ tags: ['x', 'y'] }), ctx).text).toBe('x y');
    expect(cell(col('tags.count'), row({ tags: ['x', 'y'] }), ctx).text).toBe('[2]');
    expect(cell(col('depends.indicator'), row({ depends: ['u'], pending_deps: 1 }), ctx).text).toBe('D');
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
    expect(rowClass(row({ status: 'completed' }))).toBe('done completed');
    expect(rowClass(row({ status: 'deleted' }))).toBe('done deleted');
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
    expect(cell(col('description'), withNotes, { ...ctx, journal: J }).lines).toEqual([
      '2026-10-07 12:30 call Sam first',
    ]);
    // Without journalling they are ordinary notes and show.
    expect(cell(col('description'), withNotes, ctx).lines).toHaveLength(3);
  });

  it('do not inflate the count or the one-line form', () => {
    expect(cell(col('description.count'), withNotes, { ...ctx, journal: J }).text).toBe('Buy milk [1]');
    expect(cell(col('description.oneline'), withNotes, { ...ctx, journal: J }).text).toBe(
      'Buy milk 2026-10-07 12:30 call Sam first',
    );
  });

  it('a task with only markers shows no notes at all', () => {
    const only = row({
      annotations: [
        { entry: NOW, text: 'Started task' },
        { entry: NOW + 5, text: 'Stopped task' },
      ],
    });
    expect(cell(col('description'), only, { ...ctx, journal: J }).lines).toBeUndefined();
  });
});
