// `journal.time` writes an annotation when a task is started or stopped ("Started task",
// "Stopped task"). Those are bookkeeping, not notes: the detail view hides them and shows the
// sessions they describe instead.

import type { Note } from './types';

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
