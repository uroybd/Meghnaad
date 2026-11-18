import { describe, expect, it } from 'vitest';
import { urgencyBreakdown } from './urgencyBreakdown';
import type { Row } from './types';

describe('urgencyBreakdown', () => {
  it('gives the terms task info prints, from the engine', () => {
    const task = {
      uuid: '11111111-2222-3333-4444-555555555555',
      status: 'pending',
      description: 'Pay rent',
      entry: 1_700_000_000,
      tags: ['home'],
      priority: 'H',
      urgency: 6.8,
    } as unknown as Row;
    const b = urgencyBreakdown(task, null);
    const labels = b.terms.map((t) => t[0]);
    // In Taskwarrior 3, priority is a UDA, and `task info` names its term so.
    expect(labels).toEqual(expect.arrayContaining(['tags', 'age', 'UDA priority.H']));
    expect(b.terms.find((t) => t[0] === 'UDA priority.H')).toEqual(['UDA priority.H', '1', '6', '6']);
    expect(Number(b.total)).toBeCloseTo(6.8, 1);
  });
});
