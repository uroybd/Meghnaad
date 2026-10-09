// Grouping tasks by tag for the Tags page. A task with two tags appears under both.
import type { Row } from './types';

export interface TagEntry {
  name: string;
  /** The tasks with this tag, most urgent first. */
  tasks: Row[];
  /** Of those, how many are pending and past due. */
  overdue: number;
}

/**
 * One entry per tag, by name, from the tasks given (the page asks for those its filter matches, finished ones
 * included when the filter allows them, so a tag stays listed after its last task is done). `extra` is a tag to
 * list even when no task has it (a chip clicked in a report of finished tasks), so that showing "its entry" always
 * has an entry to show.
 */
export function buildTags(rows: Row[], now: number, extra?: string): TagEntry[] {
  const by = new Map<string, TagEntry>();
  const entry = (name: string): TagEntry => {
    let e = by.get(name);
    if (!e) by.set(name, (e = { name, tasks: [], overdue: 0 }));
    return e;
  };
  for (const r of rows) {
    for (const t of r.tags) {
      const e = entry(t);
      e.tasks.push(r);
      if (r.status === 'pending' && r.due != null && r.due < now) e.overdue++;
    }
  }
  if (extra) entry(extra);
  for (const e of by.values()) e.tasks.sort((a, b) => b.urgency - a.urgency);
  return [...by.values()].sort((a, b) => a.name.localeCompare(b.name));
}
