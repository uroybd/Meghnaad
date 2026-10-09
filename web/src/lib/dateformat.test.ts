import { describe, expect, it } from 'vitest';
import { formatFor, formatPattern, weekStartOf } from './dateformat';
import { formatMoment } from './dates';
import { cell, type Ctx } from './format';
import type { Column, Row } from './types';

const WED = 1_791_376_200; // 2026-10-07 12:30:00 UTC, a Wednesday (reference values from strftime)
const JAN1_2026 = 1_767_225_600; // Thursday
const JAN1_2027 = 1_798_761_600; // Friday, ISO week 53 of 2026
const f = (pattern: string, ws: 0 | 1 = 1) => ({ pattern, weekstart: ws });

describe('formatPattern', () => {
  it('formats Taskwarrior tokens, padded and unpadded', () => {
    expect(formatPattern(WED, f('Y-M-D'), 0)).toBe('2026-10-07');
    expect(formatPattern(WED, f('d/m/y'), 0)).toBe('7/10/26');
    expect(formatPattern(WED, f('Y-M-D H:N:S'), 0)).toBe('2026-10-07 12:30:00');
    expect(formatPattern(WED, f('h:n:s'), 0)).toBe('12:30:0');
  });
  it('names days and months', () => {
    expect(formatPattern(WED, f('a A b B'), 0)).toBe('Wed Wednesday Oct October');
    expect(formatPattern(WED, f('w'), 0)).toBe('3');
  });
  it('copies anything that is not a token, and tokens are case sensitive', () => {
    expect(formatPattern(WED, f('[Y] @ H.N'), 0)).toBe('[2026] @ 12.30');
    expect(formatPattern(WED, f('Z-q'), 0)).toBe('Z-q');
  });
  it('counts days and weeks like strftime', () => {
    expect(formatPattern(WED, f('j J'), 0)).toBe('280 280');
    expect(formatPattern(JAN1_2026, f('j J'), 0)).toBe('1 001');
    expect(formatPattern(WED, f('V v', 1), 0)).toBe('41 41'); // %V
    expect(formatPattern(WED, f('V', 0), 0)).toBe('40'); // %U
    expect(formatPattern(JAN1_2026, f('V', 0), 0)).toBe('00'); // before the first Sunday
    expect(formatPattern(JAN1_2026, f('V', 1), 0)).toBe('01');
    expect(formatPattern(JAN1_2027, f('V', 1), 0)).toBe('53'); // ISO week of the old year
  });
  it('uses the viewer zone', () => {
    // 23:30 UTC on the 7th is already the 8th at UTC+2.
    expect(formatPattern(WED + 11 * 3600, f('Y-M-D H:N'), 2 * 3600)).toBe('2026-10-08 01:30');
  });
});

describe('formatFor (Taskwarrior fallbacks)', () => {
  const s = {
    dateformat: 'D/M/Y',
    'dateformat.report': 'Y-M-D',
    'dateformat.info': 'Y-M-D H:N',
    'dateformat.annotation': 'D.M.',
  };
  it('report: the report own, then dateformat.report, then dateformat', () => {
    expect(formatFor('report', s, 'Y')?.pattern).toBe('Y');
    expect(formatFor('report', s)?.pattern).toBe('Y-M-D');
    expect(formatFor('report', { dateformat: 'D/M/Y' })?.pattern).toBe('D/M/Y');
  });
  it('info and annotations', () => {
    expect(formatFor('info', s)?.pattern).toBe('Y-M-D H:N');
    expect(formatFor('annotation', s)?.pattern).toBe('D.M.');
    expect(formatFor('annotation', { dateformat: 'D/M/Y' })?.pattern).toBe('D/M/Y');
    // In the detail view an unset annotation format follows the detail view's own.
    expect(formatFor('infoNote', { 'dateformat.info': 'Y-M-D H:N' })?.pattern).toBe('Y-M-D H:N');
    expect(formatFor('infoNote', s)?.pattern).toBe('D.M.');
  });
  it('nothing configured means the built-in display', () => {
    expect(formatFor('report', {})).toBeUndefined();
    expect(formatFor('info', undefined)).toBeUndefined();
  });
  it('weekstart', () => {
    expect(weekStartOf({ weekstart: 'Sunday' })).toBe(0);
    expect(weekStartOf({ weekstart: 'monday' })).toBe(1);
    expect(weekStartOf({})).toBe(0); // Sunday, as in Taskwarrior, unless the taskrc says otherwise
    expect(formatFor('report', { dateformat: 'V', weekstart: 'sunday' })?.weekstart).toBe(0);
  });
});

describe('where it is used', () => {
  it('formatMoment keeps the built-in display without a pattern', () => {
    expect(formatMoment(WED, 0)).toBe('2026-10-07 12:30');
    expect(formatMoment(WED, 0, f('D/M/Y'))).toBe('07/10/2026');
    expect(formatMoment(null, 0, f('D/M/Y'))).toBe('');
  });

  const col = (name: string, format: string | null = null): Column => ({
    spec: name,
    name,
    format,
    label: name,
    kind: 'date',
  });
  const row = {
    due: WED,
    recur: 'weekly',
    annotations: [{ entry: WED, text: 'called' }],
    virtual_tags: [],
    extra: {},
  } as unknown as Row;
  const ctx: Ctx = { now: WED, tz: 0, dates: { report: f('D.M.Y'), annotation: f('M/D') } };

  it('table date cells, and the notes under a description', () => {
    expect(cell(col('due'), row, ctx).text).toBe('07.10.2026');
    const d = cell({ ...col('description'), kind: 'description' }, { ...row, description: 'x' } as Row, ctx);
    expect(d.lines).toEqual(['10/07 called']);
  });
  it('other column formats ignore the pattern', () => {
    expect(cell(col('due', 'epoch'), row, ctx).text).toBe(String(WED));
    expect(cell(col('due', 'iso'), row, ctx).text).toBe('2026-10-07T12:30:00Z');
  });
  it('recurrence.indicator', () => {
    const r = { ...col('recur', 'indicator'), kind: 'string' as const };
    expect(cell(r, row, ctx).text).toBe('R');
    expect(cell(r, row, { ...ctx, recurIndicator: '↻' }).text).toBe('↻');
    expect(cell(r, { ...row, recur: null } as Row, { ...ctx, recurIndicator: '↻' }).text).toBe('');
  });
});
