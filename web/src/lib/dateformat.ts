// Taskwarrior's `dateformat` patterns, so dates show the way the user's taskrc says. The writing of a date is the
// engine's (`tc_core::helpers::format_date`, through src/engine/helpers.ts), which every front end shares; what is here is
// which pattern applies where, and the zone.

import { formatDate } from '../engine/helpers';

export interface DateFmt {
  pattern: string;
  /** 0 = weeks start on Sunday, 1 = Monday (ISO weeks), from the `weekstart` setting. */
  weekstart: 0 | 1;
}

/** Format `epoch` (seconds) in the zone `tzOffsetSec` east of UTC (default: the browser's). */
export function formatPattern(epoch: number, fmt: DateFmt, tzOffsetSec?: number): string {
  const tz = tzOffsetSec ?? -new Date(epoch * 1000).getTimezoneOffset() * 60;
  return formatDate(fmt.pattern, epoch, tz, fmt.weekstart === 1);
}

/** Where a date is shown, which decides which `dateformat*` setting applies. */
export type FormatKind =
  | 'report' // table cells: report.<name>.dateformat, dateformat.report, dateformat
  | 'info' // the detail view: dateformat.info, dateformat
  | 'annotation' // notes under a report's description: dateformat.annotation, dateformat
  | 'infoNote'; // notes in the detail view: dateformat.annotation, else the detail view's format

export const weekStartOf = (settings: Record<string, string> | undefined): 0 | 1 =>
  /^mon/i.test(settings?.weekstart ?? '') ? 1 : 0;

/** The pattern for a place, following Taskwarrior's fallbacks; undefined = none configured. */
export function formatFor(
  kind: FormatKind,
  settings: Record<string, string> | undefined,
  reportDateformat?: string | null,
): DateFmt | undefined {
  const s = settings ?? {};
  const first = (...v: (string | null | undefined)[]) => v.find((x) => !!x) ?? '';
  const infoFmt = first(s['dateformat.info'], s.dateformat);
  const pattern =
    kind === 'report'
      ? first(reportDateformat, s['dateformat.report'], s.dateformat)
      : kind === 'info'
        ? infoFmt
        : kind === 'annotation'
          ? first(s['dateformat.annotation'], s.dateformat)
          : first(s['dateformat.annotation'], infoFmt);
  return pattern ? { pattern, weekstart: weekStartOf(settings) } : undefined;
}
