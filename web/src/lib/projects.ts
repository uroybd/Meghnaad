// Grouping tasks into the project tree. `Home.Kitchen` is a child of `Home`, as in Taskwarrior.
import type { Row } from './types';

export interface ProjectNode {
  /** Full dotted name; '' is the group of tasks that have no project. */
  name: string;
  /** Last segment, for display. */
  label: string;
  /** Tasks filed exactly here, most urgent first. */
  own: Row[];
  children: ProjectNode[];
  /** Pending tasks here and in every sub-project. */
  total: number;
  /** Of those, how many are past due. */
  overdue: number;
}

export const NO_PROJECT = '';

const byUrgency = (a: Row, b: Row) => b.urgency - a.urgency;

/** Build the tree from pending tasks. Sub-projects with no tasks of their own still get a node. */
export function buildTree(rows: Row[], now: number): ProjectNode[] {
  const nodes = new Map<string, ProjectNode>();
  const node = (name: string): ProjectNode => {
    let n = nodes.get(name);
    if (!n) {
      n = { name, label: name.slice(name.lastIndexOf('.') + 1), own: [], children: [], total: 0, overdue: 0 };
      nodes.set(name, n);
      const cut = name.lastIndexOf('.');
      if (cut > 0) node(name.slice(0, cut)).children.push(n);
    }
    return n;
  };

  for (const r of rows) {
    if (r.status !== 'pending') continue;
    const n = node(r.project ?? NO_PROJECT);
    n.own.push(r);
    const late = r.due != null && r.due < now;
    // Count on the node and on each ancestor.
    for (let name = n.name; ; name = name.slice(0, name.lastIndexOf('.'))) {
      const m = nodes.get(name)!;
      m.total++;
      if (late) m.overdue++;
      if (!name.includes('.')) break;
    }
  }

  const sort = (n: ProjectNode) => {
    n.own.sort(byUrgency);
    n.children.sort((a, b) => a.name.localeCompare(b.name));
    n.children.forEach(sort);
  };
  const roots = [...nodes.values()].filter((n) => !n.name.includes('.') && n.name !== NO_PROJECT);
  roots.sort((a, b) => a.name.localeCompare(b.name));
  roots.forEach(sort);
  const none = nodes.get(NO_PROJECT);
  if (none) {
    none.own.sort(byUrgency);
    none.label = '(no project)';
    roots.push(none);
  }
  return roots;
}

/** Every node that has something to expand, for "expand all". */
export function expandable(tree: ProjectNode[]): string[] {
  return tree.flatMap((n) => [...(n.children.length || n.own.length ? [n.name] : []), ...expandable(n.children)]);
}
