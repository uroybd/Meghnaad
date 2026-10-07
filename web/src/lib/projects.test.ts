import { describe, expect, it } from 'vitest';
import { buildTree, expandable } from './projects';
import type { Row } from './types';

const row = (description: string, project: string | null, extra: Partial<Row> = {}): Row =>
  ({ uuid: description, description, project, status: 'pending', urgency: 1, due: null, tags: [], ...extra }) as Row;

describe('buildTree', () => {
  const rows = [
    row('a', 'Home'),
    row('b', 'Home.Kitchen', { due: 50 }),
    row('c', 'Home.Kitchen', { urgency: 9 }),
    row('d', 'Work.Deep.Nest'),
    row('e', null),
    row('done', 'Home', { status: 'completed' }),
  ];
  const tree = buildTree(rows, 100);

  it('nests dotted names, creating parents that hold no tasks themselves', () => {
    expect(tree.map((n) => n.name)).toEqual(['Home', 'Work', '']);
    const work = tree[1];
    expect(work.own).toEqual([]);
    expect(work.children[0].name).toBe('Work.Deep');
    expect(work.children[0].children[0].name).toBe('Work.Deep.Nest');
  });

  it('counts pending tasks including sub-projects, and ignores finished ones', () => {
    expect(tree[0].total).toBe(3);
    expect(tree[0].own.map((r) => r.description)).toEqual(['a']);
    expect(tree[1].total).toBe(1);
  });

  it('counts overdue tasks up the tree', () => {
    expect(tree[0].overdue).toBe(1);
    expect(tree[0].children[0].overdue).toBe(1);
    expect(tree[1].overdue).toBe(0);
  });

  it('puts the most urgent task first and the no-project group last', () => {
    expect(tree[0].children[0].own.map((r) => r.description)).toEqual(['c', 'b']);
    expect(tree.at(-1)?.label).toBe('(no project)');
  });

  it('lists what can be expanded', () => {
    expect(expandable(tree)).toContain('Work.Deep');
    expect(expandable([])).toEqual([]);
  });
});
