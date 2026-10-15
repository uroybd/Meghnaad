// Table-header sorting. A sort is a Taskwarrior sort spec (`due+,priority-,project+/`); the UI
// applies it through the command line as `rc.report.<name>.sort:<spec>`, so the console and the
// table always agree. What a header click or a grouping does to the keys is the engine's
// (`tc_core::sortspec`, through src/engine/helpers.ts), which every front end shares; the cases are tested there.

import { callHelper } from '../engine/helpers';

export interface SortKey {
  column: string;
  desc: boolean;
  /** Trailing `/`: a visual break when this column's value changes. */
  brk: boolean;
}

/** Columns a header click can sort by (UDAs are added by the caller): the engine's list. */
export const SORTABLE = new Set(callHelper<string[]>('sortable_columns'));

/** `due.relative` sorts by `due`. */
export function baseColumn(spec: string): string {
  return spec.split('.')[0];
}

/** The keys of a spec: none for `none`, `random` or nothing, and a part that is no key is left out. */
export function parseSort(spec: string | null | undefined): SortKey[] {
  return callHelper<SortKey[]>('parse_sort', spec ?? null);
}

export function serializeSort(keys: SortKey[]): string {
  return callHelper<string>('serialize_sort', keys);
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
  return callHelper<ClickResult>('click_sort', current, column, shift);
}

/** The sort override for `report` in a filter string, if any. */
export function sortOverride(filter: string, report: string): string | null {
  return callHelper<string | null>('sort_override', filter, report);
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
  return callHelper<string>('with_sort_override', filter, report, spec, defaultSpec);
}

/** For a header: its place in the active sort (and whether it groups the table), or null. */
export function sortState(
  keys: SortKey[],
  column: string,
): { desc: boolean; rank: number | null; group: boolean } | null {
  return callHelper('sort_state', keys, column);
}

/**
 * Group the table by `column`, or stop grouping by it: its sort key gains or loses the trailing `/`. A column
 * the sort does not use yet becomes the first key, ascending, since a group is only whole when the grouping
 * column is what the table is sorted by first.
 */
export function toggleGroup(current: SortKey[], column: string, defaults: SortKey[] = []): SortKey[] {
  return callHelper<SortKey[]>('toggle_group', current, column, defaults);
}

/** The same sort with no grouping at all. */
export function clearGroups(current: SortKey[]): SortKey[] {
  return callHelper<SortKey[]>('clear_groups', current);
}

/** The column the table is grouped by first, if any. */
export function groupColumn(keys: SortKey[]): string | null {
  return callHelper<string | null>('group_column', keys);
}
