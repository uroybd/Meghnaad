// The sentences of `task info`'s change history, as Taskwarrior words them (CmdInfo.cpp).

import type { HistoryChange } from './types';

const ucFirst = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

/** Date properties hold epoch seconds; anything that isn't a number is shown as it is. */
function value(c: HistoryChange, v: string | null, show: (epoch: number) => string): string {
  if (v == null) return '';
  const n = Number.parseInt(v, 10);
  return c.date && v !== '' && n ? show(n) : v;
}

/** One change as a sentence. `show` renders epoch seconds in the user's date format. */
export function describeChange(c: HistoryChange, show: (epoch: number) => string): string {
  const prop = ucFirst(c.prop);
  switch (c.kind) {
    case 'set': return `${prop} set to '${value(c, c.value, show)}'.`;
    case 'changed': return `${prop} changed from '${value(c, c.old, show)}' to '${value(c, c.value, show)}'.`;
    case 'deleted': return c.duration != null ? `${prop} deleted (duration: ${c.duration}).` : `${prop} deleted.`;
    case 'note_added': return `Annotation of '${c.value}' added.`;
    case 'note_changed': return `Annotation changed to '${c.value}'.`;
    case 'note_deleted': return `Annotation '${c.old}' deleted.`;
    case 'tag_added': return `Tag '${c.prop}' added.`;
    case 'tag_deleted': return `Tag '${c.prop}' deleted.`;
    case 'dep_added': return `Dependency on '${c.prop}' added.`;
    case 'dep_deleted': return `Dependency on '${c.prop}' deleted.`;
  }
}
