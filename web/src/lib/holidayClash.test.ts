// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { clashMarks, clashOf } from './holidayClash';
import { highlightPieces } from './highlight';
import { store } from './store.svelte';

const HOLIDAYS = {
  'holiday.xmas.name': 'Christmas Day',
  'holiday.xmas.date': '20261225',
  'holiday.ny.name': 'New Year break',
  'holiday.ny.start': '20261231',
  'holiday.ny.end': '20270102',
};
const commands = new Set(['add', 'modify', 'list', 'done']);

beforeEach(() => {
  vi.useFakeTimers({ toFake: ['Date'] });
  vi.setSystemTime(new Date(2026, 9, 18, 12, 0, 0));
  store.config = { config: { settings: HOLIDAYS } } as never;
});
afterEach(() => {
  vi.useRealTimers();
  store.config = null;
});

describe('a date on a holiday', () => {
  it('is found for a date, a date with a time, and a stretch', () => {
    expect(clashOf('due', '2026-12-25')?.names).toEqual(['Christmas Day']);
    expect(clashOf('scheduled', '2026-12-25T10:30')?.message).toContain('Christmas Day');
    expect(clashOf('due', '2027-01-01')?.names).toEqual(['New Year break']);
    expect(clashOf('due', '2026-12-26')).toBeNull();
    expect(clashOf('due', '')).toBeNull();
  });

  it('is resolved from words first', () => {
    store.config = {
      config: { settings: { ...HOLIDAYS, 'holiday.today.name': 'Today fest', 'holiday.today.date': '20261018' } },
    } as never;
    const c = clashOf('due', 'eod');
    expect(c?.names).toEqual(['Today fest']);
    expect(c?.message).toBe('due (eod) is 2026-10-18, a holiday: Today fest');
    expect(clashOf('due', 'tomorrow')).toBeNull();
  });

  it('is not asked about when no holiday is defined', () => {
    store.config = { config: { settings: {} } } as never;
    expect(clashOf('due', '2026-12-25')).toBeNull();
    expect(clashMarks('add x due:2026-12-25', commands)).toEqual([]);
  });

  it('marks the date word of a command line that sets it, and a piece of the drawn line carries the warning', () => {
    const line = 'add Party due:2026-12-25 +fun';
    const marks = clashMarks(line, commands);
    expect(marks.map((m) => line.slice(m.start, m.end))).toEqual(['2026-12-25']);
    const pieces = highlightPieces(line, commands, marks);
    expect(pieces.filter((p) => p.warn).map((p) => p.text)).toEqual(['2026-12-25']);
    expect(pieces.map((p) => p.text).join('')).toBe(line);
    // A filter is not a plan.
    expect(clashMarks('list due:2026-12-25', commands)).toEqual([]);
    expect(clashMarks('Party due:2026-12-25', commands, true)).toHaveLength(1);
  });
});
