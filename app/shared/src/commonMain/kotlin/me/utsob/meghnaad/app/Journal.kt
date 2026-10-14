package me.utsob.meghnaad.app

// `journal.time` writes an annotation when a task is started or stopped ("Started task", "Stopped task"). Those are
// bookkeeping, not notes: the detail view hides them and shows the sessions they describe instead (web/src/lib/journal.ts).

/** Notes you wrote, without the journal's start and stop markers. */
fun visibleNotes(notes: List<Note>, journal: Journal?): List<Note> =
    if (journal == null) notes else notes.filter { it.text != journal.start && it.text != journal.stop }

/** How many notes are journal markers (for "n hidden" hints). */
fun hiddenCount(notes: List<Note>, journal: Journal?): Int = notes.size - visibleNotes(notes, journal).size
