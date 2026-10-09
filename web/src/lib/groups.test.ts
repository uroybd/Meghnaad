import { describe, expect, it } from 'vitest';
import { groupHeads } from './groups';
import type { Column, ReportResult, Row, UdaDef } from './types';

const col = (name: string, label: string, kind: Column['kind'] = 'string'): Column => ({
  spec: name,
  name,
  format: null,
  label,
  kind,
});

const row = (project: string | null, outcome?: string): Row =>
  ({
    uuid: project ?? 'none',
    status: 'pending',
    description: 'd',
    project,
    priority: null,
    tags: [],
    annotations: [],
    virtual_tags: [],
    extra: outcome ? { outcome } : {},
  }) as unknown as Row;

const report = (sort: string | null, columns: Column[], rows: Row[], breaks: boolean[]): ReportResult =>
  ({ kind: 'report', report: 'g', description: null, columns, rows, breaks, sort }) as unknown as ReportResult;

const ctx = {
  now: 0,
  udas: { outcome: { name: 'outcome', type: 'string', label: 'Result' } as UdaDef },
};

describe('groupHeads', () => {
  const rows = [row('X', 'pass'), row('X', 'pass'), row('Y', 'fail'), row('Y')];
  const breaks = [false, false, true, true];

  it('puts a header above the first row of each group and none inside it', () => {
    const r = report('project+/', [col('project', 'Proj', 'project')], rows, [false, false, true, false]);
    expect(groupHeads(r, ctx)).toEqual(['Proj: X', null, 'Proj: Y', null]);
  });

  it('names a UDA by its label even when the report does not show it, and says (none) for no value', () => {
    const r = report('outcome+/', [col('project', 'Proj', 'project')], rows, breaks);
    expect(groupHeads(r, ctx)).toEqual(['Result: pass', null, 'Result: fail', 'Result: (none)']);
  });

  it('joins every column that breaks the table', () => {
    const r = report('project+/,outcome+/', [col('project', 'Proj', 'project')], rows, breaks);
    expect(groupHeads(r, ctx)[2]).toBe('Proj: Y · Result: fail');
  });

  it('draws nothing when no sort key has a slash', () => {
    expect(groupHeads(report('project+,outcome-', [], rows, breaks), ctx)).toEqual([]);
    expect(groupHeads(report(null, [], rows, breaks), ctx)).toEqual([]);
  });
});
