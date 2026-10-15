package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals

// The same cases as web/src/lib/journal.test.ts.
class JournalTest {
    private val j = Journal(start = "Started task", stop = "Stopped task")
    private fun n(entry: Long, text: String) = Note(entry, text)
    private val notes = listOf(n(1, "Started task"), n(2, "call Sam first"), n(3, "Stopped task"), n(4, "Started task"))

    @Test
    fun onlyTheJournalMarkersAreHidden() {
        assertEquals(listOf(n(2, "call Sam first")), visibleNotes(notes, j))
        assertEquals(3, hiddenCount(notes, j))
    }

    @Test
    fun everythingShowsWhenJournallingIsOff() {
        assertEquals(notes, visibleNotes(notes, null))
        assertEquals(0, hiddenCount(notes, null))
    }

    @Test
    fun customMarkerTextIsHonouredAndTheDefaultTextIsLeftAloneThen() {
        val custom = Journal(start = "Clock in", stop = "Clock out")
        val mixed = listOf(n(1, "Clock in"), n(2, "Started task"), n(3, "Clock out"))
        assertEquals(listOf(n(2, "Started task")), visibleNotes(mixed, custom))
    }

    @Test
    fun aMarkerMatchesExactlyNotByPrefix() {
        assertEquals(1, visibleNotes(listOf(n(1, "Started task early")), j).size)
    }
}
