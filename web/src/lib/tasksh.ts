// What tasksh's `review` needs in the taskrc: a `reviewed` date UDA and a `_reviewed` report that lists the tasks
// due for review. tasksh offers to write both the first time it runs; the same lines are written from here.
// (src/review.cpp in https://github.com/GothenburgBitFactory/taskshell)

import type { ConfigResponse } from './types';

export const REVIEW_REPORT = '_reviewed';
export const REVIEW_DESCRIPTION = 'Tasksh review report.  Adjust the filter to your needs.';
export const REVIEW_SORT = 'reviewed+,modified+';
export const REVIEW_FILTER = '( reviewed.none: or reviewed.before:now-6days ) and ( +PENDING or +WAITING )';

/** `ok`: as tasksh defines it. `different`: there, but not the way tasksh needs it. */
export type Part = 'ok' | 'missing' | 'different';

export interface TaskshStatus {
  uda: Part;
  /** The `reviewed` UDA's type, when it is defined. */
  udaType: string | null;
  report: Part;
  /** The columns the `_reviewed` report has now (tasksh reads the uuid of each row). */
  columns: string[];
  /** The saved `_reviewed` filter and sort, when the report is defined. */
  filter: string | null;
  sort: string | null;
  /** Both parts are as tasksh needs them: `review` can run. */
  enabled: boolean;
}

export function taskshStatus(config: ConfigResponse | null): TaskshStatus {
  const uda = config?.config.udas?.reviewed;
  const report = config?.reports.find((r) => r.name === REVIEW_REPORT);
  const columns = report?.columns.map((c) => c.spec) ?? [];
  const udaPart: Part = !uda ? 'missing' : uda.type === 'date' ? 'ok' : 'different';
  const reportPart: Part = !report ? 'missing' : columns.length === 1 && columns[0] === 'uuid' ? 'ok' : 'different';
  return {
    uda: udaPart,
    udaType: uda?.type ?? null,
    report: reportPart,
    columns,
    filter: report?.filter ?? null,
    sort: report?.sort ?? null,
    enabled: udaPart === 'ok' && reportPart === 'ok',
  };
}

/** The part of the report you can adjust: which tasks are due, and in what order. */
export interface ReviewWanted {
  filter: string;
  sort: string;
}

/**
 * The `config` lines (name, value) that bring the taskrc to what is wanted, leaving alone what is already so.
 * `columns` goes before the rest of the report: a report without columns is not kept, so a description sent first
 * would be lost.
 */
export function setupLines(config: ConfigResponse | null, want: ReviewWanted): [string, string][] {
  const s = taskshStatus(config);
  const lines: [string, string][] = [];
  if (s.uda !== 'ok') lines.push(['uda.reviewed.type', 'date']);
  if (s.uda === 'missing' || !config?.config.udas?.reviewed?.label) lines.push(['uda.reviewed.label', 'Reviewed']);
  if (s.report !== 'ok') lines.push([`report.${REVIEW_REPORT}.columns`, 'uuid']);
  if (s.report === 'missing') lines.push([`report.${REVIEW_REPORT}.description`, REVIEW_DESCRIPTION]);
  if (s.report !== 'ok' || s.sort !== want.sort) lines.push([`report.${REVIEW_REPORT}.sort`, want.sort]);
  if (s.report !== 'ok' || s.filter !== want.filter) lines.push([`report.${REVIEW_REPORT}.filter`, want.filter]);
  return lines;
}

/** One `config` line as the arguments to send: no questions asked, as tasksh runs it. */
export function configArgs([name, value]: [string, string]): string[] {
  return ['rc.confirmation:no', 'rc.verbose:nothing', 'config', name, value];
}
