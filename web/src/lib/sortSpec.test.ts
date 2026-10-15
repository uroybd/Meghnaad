import { describe, expect, it } from 'vitest';
import {
  clearGroups,
  clickSort,
  groupColumn,
  parseSort,
  serializeSort,
  sortOverride,
  sortState,
  toggleGroup,
  withSortOverride,
} from './sortSpec';

// The behaviour is the engine's (tc_core::sortspec, tested there with the cases that were here). These check that the
// page's side carries keys and answers across: the shapes, the nulls and the defaults.
const k = (column: string, desc = false, brk = false) => ({ column, desc, brk });

describe('parse/serialize', () => {
  it('round-trips a spec, with break markers and a column format stripped', () => {
    const spec = 'start-,due+,project+/,urgency-';
    expect(serializeSort(parseSort(spec))).toBe(spec);
    expect(parseSort(spec)[2]).toEqual(k('project', false, true));
    expect(parseSort('due.relative+')).toEqual([k('due')]);
  });
  it('none, random, junk and no spec are no keys', () => {
    for (const s of ['none', 'random', 'due', null, undefined]) expect(parseSort(s)).toEqual([]);
  });
});

describe('clickSort', () => {
  it('answers keys and whether to go back to the report default', () => {
    const asc = clickSort([k('due'), k('urgency', true)], 'due');
    expect(serializeSort(asc.keys)).toBe('due-,urgency-');
    expect(asc.reset).toBe(false);
    expect(clickSort(asc.keys, 'due').reset).toBe(true);
    expect(serializeSort(clickSort([k('due')], 'project', true).keys)).toBe('due+,project+');
  });
});

describe('the sort token in a filter string', () => {
  it('adds, finds and removes it, and takes nulls', () => {
    let f = withSortOverride('project:Work', 'next', 'due-', 'urgency-');
    expect(f).toBe('project:Work rc.report.next.sort:due-');
    expect(sortOverride(f, 'next')).toBe('due-');
    expect(sortOverride(f, 'list')).toBeNull();
    f = withSortOverride(f, 'next', null, null);
    expect(f).toBe('project:Work');
    expect(withSortOverride('', 'next', 'urgency-', 'urgency-')).toBe('');
  });
});

describe('a header and grouping', () => {
  it('reports direction, rank and grouping, or null', () => {
    const keys = parseSort('due+,project-');
    expect(sortState(keys, 'due.relative')).toEqual({ desc: false, rank: 1, group: false });
    expect(sortState(parseSort('due+'), 'due')).toEqual({ desc: false, rank: null, group: false });
    expect(sortState(keys, 'tags')).toBeNull();
  });
  it('groups and ungroups, with and without the report defaults', () => {
    const own = parseSort('urgency-,project-');
    expect(serializeSort(toggleGroup(own, 'project', own))).toBe('urgency-,project-/');
    expect(serializeSort(toggleGroup(parseSort('urgency-'), 'project'))).toBe('project+/,urgency-');
    const keys = parseSort('project+/,outcome+/,due+');
    expect(groupColumn(keys)).toBe('project');
    expect(serializeSort(clearGroups(keys))).toBe('project+,outcome+,due+');
    expect(groupColumn(clearGroups(keys))).toBeNull();
  });
});
