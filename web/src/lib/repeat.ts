// How a task repeats, as the task form offers it: not at all, as an iterative task (Taskwarrior 3.6.0's `iter`), or as
// a recurring one (`recur`). One mode at a time: switching clears the other mode's fields in the same `modify`, which
// `task` takes (`iter: iter_type: recur:weekly` ends a series and makes the task the template of a recurring one).
// Who wants to keep them has the command line.

export type RepeatMode = 'none' | 'iterative' | 'recurring';

export const REPEAT_MODES: { value: RepeatMode; label: string }[] = [
  { value: 'none', label: "Doesn't repeat" },
  { value: 'iterative', label: 'Iterative' },
  { value: 'recurring', label: 'Recurring' },
];

/** The fields of both modes. */
export interface RepeatFields {
  recur: string;
  iter: string;
  iterType: string;
}

/** The mode a task's fields say it is in: a `recur` is recurring (a task can't be both), an `iter` iterative. */
export function modeOf(f: RepeatFields): RepeatMode {
  if (f.recur.trim() !== '') return 'recurring';
  if (f.iter.trim() !== '') return 'iterative';
  return 'none';
}

/**
 * The words of an `add` or `modify` that leave the task in `mode` with the fields `now`, given what it had (`was`; all
 * empty for a new task): the chosen mode's fields where they changed, and the other modes' fields cleared where it had
 * them. A mode chosen and not filled in yet (no period, no rule) changes nothing, the others' fields included.
 */
export function repeatChanges(mode: RepeatMode, now: RepeatFields, was: RepeatFields): string[] {
  if (mode === 'iterative' && now.iter.trim() === '') return [];
  if (mode === 'recurring' && now.recur.trim() === '') return [];
  const out: string[] = [];
  const clear = (name: string, had: string) => {
    if (had.trim() !== '') out.push(`${name}:`);
  };
  if (mode !== 'iterative') {
    clear('iter', was.iter);
    clear('iter_type', was.iterType);
  }
  if (mode !== 'recurring') clear('recur', was.recur);
  if (mode === 'iterative') {
    const iter = now.iter.trim();
    if (iter !== '' && iter !== was.iter.trim()) out.push(`iter:${iter}`);
    if (iter !== '' && now.iterType !== '' && now.iterType !== was.iterType) out.push(`iter_type:${now.iterType}`);
  }
  if (mode === 'recurring') {
    const recur = now.recur.trim();
    if (recur !== '' && recur !== was.recur.trim()) out.push(`recur:${recur}`);
  }
  return out;
}
