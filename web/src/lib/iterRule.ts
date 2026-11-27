// An iterative task's rule (`iter`) as the Iter picker shows it. Its Taskwarrior mode takes the periods and words `task`
// takes (`monthly`, `3d`, `P2W`, "every other day"); its Calendar mode builds an RFC 5545 rule (RRULE, `FREQ=…`) the way
// calendar apps do. The rule is read and written by the engine (`tc_core::iter_rule`), as on the phone and the terminal.

import { callHelper } from '../engine/helpers';

export type Freq = 'daily' | 'weekly' | 'monthly' | 'yearly';

export interface CalendarRule {
  freq: Freq;
  interval: number;
  /** Weekly: `MO` … `SU`. */
  days: string[];
  /** Monthly: on this day of the month, or on the `nth` (1 to 4, -1 the last) `weekday`. */
  month_day: number | null;
  nth: number | null;
  weekday: string | null;
  /** Ends on this day (`2027-12-31`), or after `count` times; neither is never. */
  until: string | null;
  count: number | null;
}

export interface MonthChoices {
  day: number;
  nth: number;
  weekday: string;
  last: boolean;
}

export const WEEKDAYS = ['MO', 'TU', 'WE', 'TH', 'FR', 'SA', 'SU'];
export const DAY_NAMES = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'];
const ORDINALS = ['first', 'second', 'third', 'fourth'];

/** The rule `value` is, when the Calendar mode can show it; null for anything else (it stays in the Taskwarrior mode). */
export const parseRule = (value: string): CalendarRule | null =>
  callHelper<CalendarRule | null>('iter_rule_parse', value);
export const buildRule = (r: CalendarRule): string => callHelper<string>('iter_rule_build', r);
/** Every week on the weekday of `date` (`2026-12-25`). */
export const defaultRule = (date: string): CalendarRule => callHelper<CalendarRule>('iter_rule_default', date);
export const monthChoices = (date: string): MonthChoices | null =>
  callHelper<MonthChoices | null>('iter_month_choices', date);
/** Any `iter` in words: "Every 2 weeks on Friday", "every 3 days", or as typed. */
export const describeIter = (value: string): string => callHelper<string>('describe_iter', value ?? '');

/** "the fourth Friday", "the last Friday". */
export function nthWords(nth: number, weekday: string): string {
  const day = DAY_NAMES[WEEKDAYS.indexOf(weekday)] ?? weekday;
  return `the ${nth === -1 ? 'last' : (ORDINALS[nth - 1] ?? 'first')} ${day}`;
}

/** The day a new rule counts from: the first of `dates` that is a day (`2026-12-25…`), else today. */
export function anchorOf(dates: string[], today = new Date()): string {
  const day = dates.map((d) => d.trim().slice(0, 10)).find((d) => /^\d{4}-\d{2}-\d{2}$/.test(d));
  if (day) return day;
  const p = (n: number) => String(n).padStart(2, '0');
  return `${today.getFullYear()}-${p(today.getMonth() + 1)}-${p(today.getDate())}`;
}
