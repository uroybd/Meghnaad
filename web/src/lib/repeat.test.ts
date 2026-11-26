import { describe, expect, it } from 'vitest';
import { modeOf, repeatChanges, type RepeatFields } from './repeat';

const none: RepeatFields = { recur: '', iter: '', iterType: '' };
const iterative: RepeatFields = { recur: '', iter: 'monthly', iterType: 'fixed' };
const recurring: RepeatFields = { recur: 'weekly', iter: '', iterType: '' };

describe('modeOf', () => {
  it('reads the mode from the fields a task has', () => {
    expect(modeOf(none)).toBe('none');
    expect(modeOf(iterative)).toBe('iterative');
    expect(modeOf(recurring)).toBe('recurring');
    expect(modeOf({ ...none, iterType: 'fixed' })).toBe('none');
  });
});

describe('repeatChanges', () => {
  it('sends only what changed in the chosen mode', () => {
    expect(repeatChanges('none', none, none)).toEqual([]);
    expect(repeatChanges('iterative', iterative, none)).toEqual(['iter:monthly', 'iter_type:fixed']);
    expect(repeatChanges('iterative', { ...iterative, iterType: 'chained' }, iterative)).toEqual(['iter_type:chained']);
    expect(repeatChanges('recurring', recurring, none)).toEqual(['recur:weekly']);
    expect(repeatChanges('recurring', { ...recurring, recur: 'daily' }, recurring)).toEqual(['recur:daily']);
  });

  it("clears the other mode's fields the task had, in the same command", () => {
    // Iterative to recurring: the series ends and the task becomes a template, as `task` 3.6.0 takes it.
    expect(repeatChanges('recurring', { ...iterative, recur: 'weekly' }, iterative)).toEqual([
      'iter:',
      'iter_type:',
      'recur:weekly',
    ]);
    expect(repeatChanges('none', iterative, iterative)).toEqual(['iter:', 'iter_type:']);
    // A type left behind with no rule goes too.
    expect(repeatChanges('none', none, { ...none, iterType: 'fixed' })).toEqual(['iter_type:']);
    expect(repeatChanges('none', none, recurring)).toEqual(['recur:']);
  });

  it('changes nothing for a mode chosen but not filled in', () => {
    expect(repeatChanges('iterative', none, none)).toEqual([]);
    expect(repeatChanges('recurring', none, none)).toEqual([]);
    // No period yet: the series is not ended for it.
    expect(repeatChanges('recurring', iterative, iterative)).toEqual([]);
    // Its type alone is not enough: `task` would refuse a type without a rule.
    expect(repeatChanges('iterative', { ...none, iterType: 'fixed' }, none)).toEqual([]);
  });
});
