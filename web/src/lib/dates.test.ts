import { describe, expect, it } from 'vitest';
import { compactDuration, formatMoment, fromParts, isValidDate, isValidTime, toParts } from './dates';

const IST = 19800; // +05:30
// 2026-12-25T08:30:00Z
const XMAS_0830Z = 1798187400;

describe('date + optional time', () => {
  it('a date-only value stays date-only', () => {
    expect(fromParts({ date: '2026-12-25', time: '' })).toBe('2026-12-25');
  });

  it('adding a time makes it a moment', () => {
    expect(fromParts({ date: '2026-12-25', time: '08:30' })).toBe('2026-12-25T08:30');
  });

  it('no date means no value, even if a time was typed', () => {
    expect(fromParts({ date: '', time: '08:30' })).toBe('');
  });

  it('shows the time only when it is not midnight', () => {
    expect(toParts(XMAS_0830Z, 0)).toEqual({ date: '2026-12-25', time: '08:30' });
    expect(toParts(1798156800, 0)).toEqual({ date: '2026-12-25', time: '' });
  });

  it("renders in the viewer's zone, which can change the date", () => {
    // 08:30Z is 14:00 in IST; 20:00Z is already the next day there.
    expect(toParts(XMAS_0830Z, IST)).toEqual({ date: '2026-12-25', time: '14:00' });
    expect(toParts(XMAS_0830Z + 12 * 3600, IST)).toEqual({ date: '2026-12-26', time: '02:00' });
  });

  it('round-trips a moment through local parts', () => {
    const p = toParts(XMAS_0830Z, IST);
    expect(fromParts(p)).toBe('2026-12-25T14:00');
  });

  it('formats for display', () => {
    expect(formatMoment(XMAS_0830Z, 0)).toBe('2026-12-25 08:30');
    expect(formatMoment(1798156800, 0)).toBe('2026-12-25');
    expect(formatMoment(null)).toBe('');
  });

  it('validates', () => {
    expect(isValidDate('2026-02-29')).toBe(false);
    expect(isValidDate('2028-02-29')).toBe(true);
    expect(isValidDate('2026-13-01')).toBe(false);
    expect(isValidTime('23:59')).toBe(true);
    expect(isValidTime('24:00')).toBe(false);
    expect(isValidTime('9:30')).toBe(false);
  });
});

describe('compactDuration', () => {
  it.each([
    [30, '30s'],
    [90, '1min'],
    [7200, '2h'],
    [3 * 86400, '3d'],
    [20 * 86400, '2w'],
    [90 * 86400, '3mo'],
    [800 * 86400, '2y'],
    [-3 * 86400, '-3d'],
  ])('%i seconds -> %s', (sec, want) => expect(compactDuration(sec)).toBe(want));
});

import { formatSeconds } from './dates';
describe('formatSeconds', () => {
  it.each([
    [0, '0s'],
    [45, '45s'],
    [75, '1m 15s'],
    [3600, '1h 0m'],
    [4325, '1h 12m'],
    [90061, '1d 1h'],
    [-5, '0s'],
  ])('%i -> %s', (sec, want) => expect(formatSeconds(sec)).toBe(want));
});

import { formatStamp } from './dates';
describe('formatStamp', () => {
  it('shows seconds, in the given zone', () => {
    expect(formatStamp(1798187405, 0)).toBe('2026-12-25 08:30:05');
    expect(formatStamp(1798187405, 19800)).toBe('2026-12-25 14:00:05');
    expect(formatStamp(null)).toBe('');
  });
});
