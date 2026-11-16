import { describe, expect, it } from 'vitest';
import { remindUdaLines, remindUdaStatus } from './remindUda';
import type { ConfigResponse } from './types';

const config = (udas: Record<string, { type: string; label?: string }>) =>
  ({
    config: { udas: Object.fromEntries(Object.entries(udas).map(([n, u]) => [n, { name: n, ...u }])) },
  }) as never as ConfigResponse;

describe('the remind_in UDA', () => {
  it('is written the first time with a label', () => {
    expect(remindUdaStatus(config({}))).toBe('missing');
    expect(remindUdaLines(config({}))).toEqual([
      ['uda.remind_in.type', 'string'],
      ['uda.remind_in.label', 'Remind in'],
    ]);
    expect(remindUdaLines(null)).toHaveLength(2);
  });

  it('is left alone when it is a string, and only its type is changed otherwise', () => {
    expect(remindUdaStatus(config({ remind_in: { type: 'string' } }))).toBe('ok');
    expect(remindUdaLines(config({ remind_in: { type: 'string' } }))).toEqual([]);
    expect(remindUdaStatus(config({ remind_in: { type: 'numeric', label: 'Mine' } }))).toBe('different');
    expect(remindUdaLines(config({ remind_in: { type: 'numeric', label: 'Mine' } }))).toEqual([
      ['uda.remind_in.type', 'string'],
    ]);
  });
});
