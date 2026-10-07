import { describe, expect, it } from 'vitest';
import { describeRecur, presetFor, PRESETS } from './recurrence';

describe('describeRecur', () => {
  it('names the common periods', () => {
    expect(describeRecur('daily')).toBe('every day');
    expect(describeRecur('weekdays')).toBe('every weekday');
    expect(describeRecur('biweekly')).toBe('every 2 weeks');
    expect(describeRecur('quarterly')).toBe('every 3 months');
    expect(describeRecur('Annual')).toBe('every year');
  });
  it('reads counts and units', () => {
    expect(describeRecur('3d')).toBe('every 3 days');
    expect(describeRecur('1w')).toBe('every week');
    expect(describeRecur('2 weeks')).toBe('every 2 weeks');
    expect(describeRecur('6mo')).toBe('every 6 months');
    expect(describeRecur('2q')).toBe('every 2 quarters');
  });
  it('reads ISO durations', () => {
    expect(describeRecur('P1M')).toBe('every 1 month');
    expect(describeRecur('P2W')).toBe('every 2 weeks');
    expect(describeRecur('P1Y2M')).toBe('every 1 year 2 months');
  });
  it('leaves unknown text alone and handles empty', () => {
    expect(describeRecur('fortnightish')).toBe('fortnightish');
    expect(describeRecur('P')).toBe('P');
    expect(describeRecur(null)).toBe('');
    expect(describeRecur('')).toBe('');
  });
});

describe('presetFor', () => {
  it('matches only exact presets so anything else opens the custom field', () => {
    for (const p of PRESETS) expect(presetFor(p.value)).toBe(p.value);
    expect(presetFor('3d')).toBe('');
    expect(presetFor('week')).toBe('');
    expect(presetFor(null)).toBe('');
  });
});
