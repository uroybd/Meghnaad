import { describe, expect, it } from 'vitest';
import { orList, priorityValues } from './priority';
import type { UdaDef } from './types';

const priority = (values: string[]): Record<string, UdaDef> => ({
  priority: { name: 'priority', type: 'string', label: 'Priority', values, default: null, indicator: null },
});

describe('the values of priority', () => {
  it('are H, M and L until the taskrc says otherwise', () => {
    expect(priorityValues(undefined)).toEqual(['H', 'M', 'L']);
    expect(priorityValues({})).toEqual(['H', 'M', 'L']);
    expect(priorityValues(priority([]))).toEqual(['H', 'M', 'L']);
  });

  it('follow uda.priority.values, without the blank one', () => {
    expect(priorityValues(priority(['Critical', 'Important', '']))).toEqual(['Critical', 'Important']);
    expect(priorityValues(priority(['H', 'M', '', 'L']))).toEqual(['H', 'M', 'L']);
  });

  it('read as a sentence', () => {
    expect(orList(['H', 'M', 'L'])).toBe('H, M or L');
    expect(orList(['Critical', 'Important'])).toBe('Critical or Important');
    expect(orList(['Only'])).toBe('Only');
  });
});
