import { describe, expect, it } from 'vitest';
import { filterWords, isCalendarWord, shiftMonth, stepWords, withMonths } from './calendarQuery';

describe('which words are months and which are filters', () => {
  it('knows the words calendar reads', () => {
    for (const w of ['due', 'du', 'y', 'Y', '3', '12', '2027', 'march', 'mar', 'DEC', 'sept'])
      expect(isCalendarWord(w), w).toBe(true);
    for (const w of ['project:Work', '+next', '/milk/', '-waiting', 'Work', 'd', 'x', '(', 'and'])
      expect(isCalendarWord(w), w).toBe(false);
  });

  it('splits a query into its filter and its months', () => {
    expect(filterWords('project:Work march 2027 +next')).toEqual(['project:Work', '+next']);
    expect(filterWords('due')).toEqual([]);
    expect(withMonths('project:Work march 2027', ['5', '2028'])).toBe('project:Work 5 2028');
    expect(withMonths('march 2027', [])).toBe('');
    expect(withMonths("'/buy milk/' 3 2027", ['y'])).toBe("'/buy milk/' y");
  });
});

describe('paging', () => {
  it('moves by months across years', () => {
    expect(shiftMonth({ year: 2026, month: 10 }, 3)).toEqual({ year: 2027, month: 1 });
    expect(shiftMonth({ year: 2026, month: 10 }, -10)).toEqual({ year: 2025, month: 12 });
    expect(shiftMonth({ year: 2026, month: 1 }, -1)).toEqual({ year: 2025, month: 12 });
    expect(shiftMonth({ year: 2026, month: 12 }, 1)).toEqual({ year: 2027, month: 1 });
    expect(shiftMonth({ year: 2026, month: 6 }, 0)).toEqual({ year: 2026, month: 6 });
  });

  it('steps by what is shown, and keeps a whole year a whole year', () => {
    expect(stepWords({ year: 2026, month: 10 }, 3, 1)).toEqual(['1', '2027']);
    expect(stepWords({ year: 2026, month: 10 }, 3, -1)).toEqual(['7', '2026']);
    expect(stepWords({ year: 2026, month: 1 }, 12, 1)).toEqual(['1', '2027', 'y']);
    expect(stepWords({ year: 2026, month: 1 }, 12, -1)).toEqual(['1', '2025', 'y']);
    // calendar.monthsperline=2: two months at a time.
    expect(stepWords({ year: 2026, month: 10 }, 2, 1)).toEqual(['12', '2026']);
  });
});
