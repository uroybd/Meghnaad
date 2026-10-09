// Table-header sorting. A sort is a Taskwarrior sort spec (`due+,priority-,project+/`); the UI
// applies it through the command line as `rc.report.<name>.sort:<spec>`, so the console and the
// table always agree.

import { shellQuote, splitWords } from './cmdline';

export interface SortKey {
  column: string;
  desc: boolean;
  /** Trailing `/`: a visual break when this column's value changes. */
  brk: boolean;
}

/** Columns a header click can sort by (UDAs are added by the caller). */
export const SORTABLE = new Set([
  'id',
  'uuid',
  'status',
  'description',
  'project',
  'priority',
  'tags',
  'depends',
  'entry',
  'start',
  'end',
  'due',
  'wait',
  'scheduled',
  'until',
  'modified',
  'urgency',
  'recur',
  'parent',
]);

const MAX_KEYS = 4;

/** `due.relative` sorts by `due`. */
export function baseColumn(spec: string): string {
  return spec.split('.')[0];
}

export function parseSort(spec: string | null | undefined): SortKey[] {
  const s = (spec ?? '').trim();
  if (!s || s === 'none' || s === 'random') return [];
  return s.split(',').flatMap((part) => {
    const m = /^(.+?)([+-])(\/?)$/.exec(part.trim());
    return m ? [{ column: baseColumn(m[1]), desc: m[2] === '-', brk: m[3] === '/' }] : [];
  });
}

export function serializeSort(keys: SortKey[]): string {
  return keys.map((k) => `${k.column}${k.desc ? '-' : '+'}${k.brk ? '/' : ''}`).join(',');
}

export interface ClickResult {
  keys: SortKey[];
  /** The third click on a column: go back to the report's own sort. */
  reset: boolean;
}

/**
 * What a header click does:
 *  - click a new column: it becomes the primary key (ascending); earlier keys stay as tie-breakers;
 *  - click the primary column again: ascending -> descending -> back to the report's default;
 *  - shift-click: add the column as a further tie-breaker, or flip its direction if present.
 */
export function clickSort(current: SortKey[], column: string, shift = false): ClickResult {
  const at = current.findIndex((k) => k.column === column);

  if (shift) {
    if (at >= 0) {
      const keys = current.map((k, i) => (i === at ? { ...k, desc: !k.desc } : k));
      return { keys, reset: false };
    }
    return { keys: [...current, { column, desc: false, brk: false }].slice(0, MAX_KEYS), reset: false };
  }

  if (at === 0) {
    if (!current[0].desc) {
      return { keys: [{ ...current[0], desc: true }, ...current.slice(1)], reset: false };
    }
    return { keys: [], reset: true };
  }
  const rest = current.filter((k) => k.column !== column);
  return { keys: [{ column, desc: false, brk: false }, ...rest].slice(0, MAX_KEYS), reset: false };
}

const tokenFor = (report: string) =>
  new RegExp(`^rc\\.report\\.${report.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\.sort[:=]`);

/** The sort override for `report` in a filter string, if any. */
export function sortOverride(filter: string, report: string): string | null {
  const re = tokenFor(report);
  const w = splitWords(filter).find((x) => re.test(x));
  return w ? w.slice(w.search(/[:=]/) + 1) : null;
}

/**
 * Set (or with `null`, remove) the sort override in a filter string. A spec equal to the report's
 * default needs no override, so it is dropped to keep the command short.
 */
export function withSortOverride(
  filter: string,
  report: string,
  spec: string | null,
  defaultSpec: string | null,
): string {
  const re = tokenFor(report);
  const words = splitWords(filter).filter((w) => !re.test(w));
  if (spec && spec !== (defaultSpec ?? '')) words.push(`rc.report.${report}.sort:${spec}`);
  return words.map(shellQuote).join(' ');
}

/** For a header: its place in the active sort (and whether it groups the table), or null. */
export function sortState(
  keys: SortKey[],
  column: string,
): { desc: boolean; rank: number | null; group: boolean } | null {
  const i = keys.findIndex((k) => k.column === baseColumn(column));
  if (i < 0) return null;
  return { desc: keys[i].desc, rank: keys.length > 1 ? i + 1 : null, group: keys[i].brk };
}

/**
 * Group the table by `column`, or stop grouping by it: its sort key gains or loses the trailing `/`. A column
 * the sort does not use yet becomes the first key, ascending, since a group is only whole when the grouping
 * column is what the table is sorted by first.
 */
export function toggleGroup(current: SortKey[], column: string, defaults: SortKey[] = []): SortKey[] {
  const base = baseColumn(column);
  const key = current.find((k) => k.column === base);
  if (key) {
    // Ending a grouping that put the column into the sort takes it out again; one the report's own sort has
    // keeps its place and loses only the `/`.
    if (key.brk && !defaults.some((d) => d.column === base)) return current.filter((k) => k !== key);
    return current.map((k) => (k === key ? { ...k, brk: !k.brk } : k));
  }
  return [{ column: base, desc: false, brk: true }, ...current].slice(0, MAX_KEYS);
}

/** The same sort with no grouping at all. */
export function clearGroups(current: SortKey[]): SortKey[] {
  return current.map((k) => ({ ...k, brk: false }));
}

/** The column the table is grouped by first, if any. */
export function groupColumn(keys: SortKey[]): string | null {
  return keys.find((k) => k.brk)?.column ?? null;
}
