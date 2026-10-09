// Taskwarrior's `dateformat` patterns (ported from libshared's Datetime::toString), so dates show
// the way the user's taskrc says. Letters are tokens, everything else is copied as is:
//   Y 2026   y 26    M 07 / m 7 (month)   D 05 / d 5 (day)    H 09 / h 9 (hour)
//   N 05 / n 5 (minute)   S 05 / s 5 (second)   A Wednesday / a Wed   B October / b Oct
//   V 05 / v 5 (week)   J 007 / j 7 (day of year)   w 3 (weekday, Sunday = 0)

const DAYS = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];
const MONTHS = [
  'January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December',
];

export interface DateFmt {
  pattern: string;
  /** 0 = weeks start on Sunday, 1 = Monday (ISO weeks), from the `weekstart` setting. */
  weekstart: 0 | 1;
}

const pad = (n: number, w = 2) => String(n).padStart(w, '0');

/** The ISO-8601 week number (Monday first, week 1 holds the first Thursday). */
function isoWeek(y: number, m0: number, d: number): number {
  const t = new Date(Date.UTC(y, m0, d));
  const dow = t.getUTCDay() || 7;
  t.setUTCDate(t.getUTCDate() + 4 - dow);
  const jan1 = Date.UTC(t.getUTCFullYear(), 0, 1);
  return Math.floor((t.getTime() - jan1) / 86_400_000 / 7) + 1;
}

/** strftime's %U: Sunday-first weeks, days before the first Sunday are week 0. */
function sundayWeek(yday0: number, wday: number): number {
  return Math.floor((yday0 + 7 - wday) / 7);
}

/** Format `epoch` (seconds) in the zone `tzOffsetSec` east of UTC (default: the browser's). */
export function formatPattern(epoch: number, fmt: DateFmt, tzOffsetSec?: number): string {
  const tz = tzOffsetSec ?? -new Date(epoch * 1000).getTimezoneOffset() * 60;
  const d = new Date((epoch + tz) * 1000);
  const y = d.getUTCFullYear();
  const m0 = d.getUTCMonth();
  const day = d.getUTCDate();
  const wday = d.getUTCDay();
  const yday0 = Math.round((Date.UTC(y, m0, day) - Date.UTC(y, 0, 1)) / 86_400_000);
  const week = fmt.weekstart === 0 ? sundayWeek(yday0, wday) : isoWeek(y, m0, day);
  const h = d.getUTCHours();
  const mi = d.getUTCMinutes();
  const s = d.getUTCSeconds();

  let out = '';
  for (const c of fmt.pattern) {
    switch (c) {
      case 'm': out += m0 + 1; break;
      case 'M': out += pad(m0 + 1); break;
      case 'd': out += day; break;
      case 'D': out += pad(day); break;
      case 'y': out += pad(y % 100); break;
      case 'Y': out += y; break;
      case 'a': out += DAYS[wday].slice(0, 3); break;
      case 'A': out += DAYS[wday]; break;
      case 'b': out += MONTHS[m0].slice(0, 3); break;
      case 'B': out += MONTHS[m0]; break;
      case 'v': out += week; break;
      case 'V': out += pad(week); break;
      case 'h': out += h; break;
      case 'H': out += pad(h); break;
      case 'n': out += mi; break;
      case 'N': out += pad(mi); break;
      case 's': out += s; break;
      case 'S': out += pad(s); break;
      case 'j': out += yday0 + 1; break;
      case 'J': out += pad(yday0 + 1, 3); break;
      case 'w': out += wday; break;
      default: out += c;
    }
  }
  return out;
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
    kind === 'report' ? first(reportDateformat, s['dateformat.report'], s.dateformat)
    : kind === 'info' ? infoFmt
    : kind === 'annotation' ? first(s['dateformat.annotation'], s.dateformat)
    : first(s['dateformat.annotation'], infoFmt);
  return pattern ? { pattern, weekstart: weekStartOf(settings) } : undefined;
}
