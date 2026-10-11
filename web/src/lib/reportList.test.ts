import { describe, expect, it } from 'vitest';
import { isHiddenReport, offeredReports } from './reportList';
import type { ReportMeta } from './types';

const r = (name: string): ReportMeta => ({ name, description: null, columns: [], filter: null, sort: null });
const ALL = [r('next'), r('_reviewed'), r('list'), r('_unique'), r('my_report')];
const names = (l: ReportMeta[]) => l.map((x) => x.name);

describe('hidden reports', () => {
  it('are the ones whose names start with an underscore', () => {
    expect(isHiddenReport('_reviewed')).toBe(true);
    for (const n of ['next', 'my_report', 'a_', '']) expect(isHiddenReport(n), n).toBe(false);
  });

  it('are left out of what is offered, unless all are asked for', () => {
    expect(names(offeredReports(ALL, false, 'next'))).toEqual(['next', 'list', 'my_report']);
    expect(names(offeredReports(ALL, true, 'next'))).toEqual(['next', '_reviewed', 'list', '_unique', 'my_report']);
  });

  it('stay offered while one of them is the report in use', () => {
    expect(names(offeredReports(ALL, false, '_reviewed'))).toEqual(['next', '_reviewed', 'list', 'my_report']);
  });

  it('leave the list as it is when there are none', () => {
    const plain = [r('next'), r('list')];
    expect(offeredReports(plain, false, 'next')).toEqual(plain);
  });
});
