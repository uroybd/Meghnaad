// The Calendar page keeps its months and its task filter in one box, as `task calendar` takes
// them on one line: `due`, `y`, a year, a month and year, or a month name, plus filter words.

import { splitWords, shellQuote } from './cmdline';

const MONTHS = [
  'january',
  'february',
  'march',
  'april',
  'may',
  'june',
  'july',
  'august',
  'september',
  'october',
  'november',
  'december',
];

/** Whether a word is one `calendar` itself reads (the same test the server applies). */
export function isCalendarWord(word: string): boolean {
  const w = word.toLowerCase();
  return (
    (w.length >= 2 && 'due'.startsWith(w)) ||
    w === 'y' ||
    /^\d+$/.test(w) ||
    (w.length >= 2 && MONTHS.some((m) => m.startsWith(w)))
  );
}

/** The filter words of a query, without its months. */
export function filterWords(query: string): string[] {
  return splitWords(query).filter((w) => !isCalendarWord(w));
}

/** The same filter with other months asked for. */
export function withMonths(query: string, months: string[]): string {
  return [...filterWords(query), ...months].map(shellQuote).join(' ');
}

/** `first` moved by `delta` months. */
export function shiftMonth(first: { year: number; month: number }, delta: number): { year: number; month: number } {
  const index = first.year * 12 + (first.month - 1) + delta;
  return { year: Math.floor(index / 12), month: (((index % 12) + 12) % 12) + 1 };
}

/**
 * The words that show the page before or after the one on screen. A year (12 months) steps by a
 * year and keeps showing a whole year; otherwise it steps by as many months as are shown.
 */
export function stepWords(first: { year: number; month: number }, shown: number, direction: 1 | -1): string[] {
  const next = shiftMonth(first, direction * shown);
  const words = [String(next.month), String(next.year)];
  return shown === 12 ? [...words, 'y'] : words;
}
