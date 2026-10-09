import { describe, expect, it } from 'vitest';
import { clickSort, parseSort, serializeSort, sortOverride, sortState, withSortOverride } from './sortSpec';

const k = (column: string, desc = false, brk = false) => ({ column, desc, brk });

describe('parse/serialize', () => {
  it('round-trips a real spec, including break markers', () => {
    const spec = 'start-,due+,project+/,urgency-';
    expect(serializeSort(parseSort(spec))).toBe(spec);
    expect(parseSort(spec)[2]).toEqual(k('project', false, true));
  });
  it('none, random and junk mean no keys', () => {
    expect(parseSort('none')).toEqual([]);
    expect(parseSort('random')).toEqual([]);
    expect(parseSort(null)).toEqual([]);
    expect(parseSort('due')).toEqual([]); // no direction: not a valid key
  });
  it('strips a column format', () => {
    expect(parseSort('due.relative+')).toEqual([k('due')]);
  });
});

describe('clickSort', () => {
  it('a new column becomes primary, keeping the old keys as tie-breakers', () => {
    const r = clickSort([k('due'), k('urgency', true)], 'project');
    expect(serializeSort(r.keys)).toBe('project+,due+,urgency-');
    expect(r.reset).toBe(false);
  });
  it('re-clicking the primary column cycles asc -> desc -> default', () => {
    const asc = clickSort([k('due'), k('urgency', true)], 'due');
    expect(serializeSort(asc.keys)).toBe('due-,urgency-');
    const reset = clickSort(asc.keys, 'due');
    expect(reset.reset).toBe(true);
  });
  it('clicking a secondary column promotes it, ascending, without duplicating it', () => {
    const r = clickSort([k('due'), k('project', true)], 'project');
    expect(serializeSort(r.keys)).toBe('project+,due+');
  });
  it('shift-click appends a tie-breaker, or flips it if already present', () => {
    const added = clickSort([k('due')], 'project', true);
    expect(serializeSort(added.keys)).toBe('due+,project+');
    const flipped = clickSort(added.keys, 'project', true);
    expect(serializeSort(flipped.keys)).toBe('due+,project-');
  });
  it('never keeps more than four keys', () => {
    const r = clickSort([k('a'), k('b'), k('c'), k('d')], 'e');
    expect(r.keys).toHaveLength(4);
    expect(r.keys[0].column).toBe('e');
  });
  it('starting from an unsorted report works', () => {
    expect(serializeSort(clickSort([], 'due').keys)).toBe('due+');
  });
});

describe('the sort token in a filter string', () => {
  it('adds, replaces and removes the override for one report only', () => {
    let f = withSortOverride('project:Work', 'next', 'due-', 'urgency-');
    expect(f).toBe('project:Work rc.report.next.sort:due-');
    expect(sortOverride(f, 'next')).toBe('due-');
    f = withSortOverride(f, 'next', 'description+', 'urgency-');
    expect(f).toBe('project:Work rc.report.next.sort:description+');
    expect(sortOverride(f, 'list')).toBeNull();
    f = withSortOverride(f, 'next', null, 'urgency-');
    expect(f).toBe('project:Work');
  });
  it('drops the override when it equals the report default', () => {
    expect(withSortOverride('', 'next', 'urgency-', 'urgency-')).toBe('');
  });
  it("leaves other reports' overrides alone", () => {
    const f = withSortOverride('rc.report.list.sort:due+', 'next', 'urgency+', 'urgency-');
    expect(f).toBe('rc.report.list.sort:due+ rc.report.next.sort:urgency+');
  });
  it('handles report names with regex characters safely', () => {
    expect(sortOverride('rc.report.a.b.sort:due+', 'a.b')).toBe('due+');
    expect(sortOverride('rc.report.axb.sort:due+', 'a.b')).toBeNull();
  });
});

describe('sortState', () => {
  it('reports direction and rank for a header', () => {
    const keys = parseSort('due+,project-');
    expect(sortState(keys, 'due.relative')).toEqual({ desc: false, rank: 1 });
    expect(sortState(keys, 'project')).toEqual({ desc: true, rank: 2 });
    expect(sortState(keys, 'tags')).toBeNull();
    expect(sortState(parseSort('due+'), 'due')).toEqual({ desc: false, rank: null });
  });
});
