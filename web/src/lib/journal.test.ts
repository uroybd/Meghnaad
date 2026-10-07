import { describe, expect, it } from 'vitest';
import { hiddenCount, visibleNotes } from './journal';

const J = { start: 'Started task', stop: 'Stopped task' };
const n = (entry: number, text: string) => ({ entry, text });

describe('visibleNotes', () => {
  const notes = [n(1, 'Started task'), n(2, 'call Sam first'), n(3, 'Stopped task'), n(4, 'Started task')];

  it('hides only the journal markers', () => {
    expect(visibleNotes(notes, J)).toEqual([n(2, 'call Sam first')]);
    expect(hiddenCount(notes, J)).toBe(3);
  });

  it('shows everything when journalling is off', () => {
    expect(visibleNotes(notes, null)).toEqual(notes);
    expect(hiddenCount(notes, undefined)).toBe(0);
  });

  it('honours custom marker text, and leaves the default text alone then', () => {
    const custom = { start: 'Clock in', stop: 'Clock out' };
    const mixed = [n(1, 'Clock in'), n(2, 'Started task'), n(3, 'Clock out')];
    expect(visibleNotes(mixed, custom)).toEqual([n(2, 'Started task')]);
  });

  it('matches exactly, not by prefix', () => {
    expect(visibleNotes([n(1, 'Started task early')], J)).toHaveLength(1);
  });
});
