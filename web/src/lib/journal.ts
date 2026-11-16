// `journal.time` writes an annotation when a task is started or stopped ("Started task",
// "Stopped task"). Those are bookkeeping, not notes: the detail view hides them and shows the
// sessions they describe instead.

import { callHelper } from '../engine/helpers';
import type { Note, Session } from './types';

export interface Markers {
  start: string;
  stop: string;
}

/** Notes you wrote, without the journal's start/stop markers. */
export function visibleNotes(notes: Note[], journal: Markers | null | undefined): Note[] {
  if (!journal) return notes;
  return notes.filter((n) => n.text !== journal.start && n.text !== journal.stop);
}

/** How many notes are journal markers (for "n hidden" hints). */
export function hiddenCount(notes: Note[], journal: Markers | null | undefined): number {
  return notes.length - visibleNotes(notes, journal).length;
}

/** A task's tracked work: its sessions (the running one last, with no end) and what they add up to, in seconds. */
export interface TimeLog {
  sessions: Session[];
  total: number;
}

/**
 * The time log of a task, from the start and stop notes `journal.time` wrote, as of `now` (seconds): the engine's own
 * pairing of them (`tc_core::helpers::time_log`), which the phone and the terminal app draw from too. None without the
 * journal.
 */
export function timeLog(
  task: { annotations: Note[]; start: number | null },
  journal: Markers | null | undefined,
  now = Math.floor(Date.now() / 1000),
): TimeLog {
  return callHelper<TimeLog>('time_log', { annotations: task.annotations, start: task.start }, journal ?? null, now);
}
