import { describe, expect, it } from 'vitest';
import { describeRecur, presetFor, PRESETS } from './recurrence';

// The words are the engine's (tc_core::helpers::describe_recur, tested there with the cases that were here).
describe('describeRecur', () => {
  it('asks the engine', () => {
    expect(describeRecur('3d')).toBe('every 3 days');
    expect(describeRecur('weekly')).toBe('every week');
  });
  it('handles no period', () => {
    expect(describeRecur(null)).toBe('');
    expect(describeRecur('')).toBe('');
    expect(describeRecur('fortnightish')).toBe('fortnightish');
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
