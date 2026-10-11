// Reports whose names start with `_` are Taskwarrior's hidden helpers (`_reviewed`, `_unique`): they are for scripts and
// other commands, so lists a person picks from leave them out until asked, as `task reports` does. Typing one still works.

import type { ReportMeta } from './types';

export const isHiddenReport = (name: string): boolean => name.startsWith('_');

/**
 * The reports to offer: all of them when `showAll`, otherwise those not hidden, plus the one in use (a hidden report
 * that was opened by name must not vanish from under the person looking at it).
 */
export function offeredReports(reports: ReportMeta[], showAll: boolean, current: string): ReportMeta[] {
  return showAll ? reports : reports.filter((r) => !isHiddenReport(r.name) || r.name === current);
}
