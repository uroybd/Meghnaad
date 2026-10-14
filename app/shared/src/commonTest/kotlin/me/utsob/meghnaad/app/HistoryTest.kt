package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals

class HistoryTest {
    private val show: (Long) -> String = { "<$it>" }

    private fun c(kind: String, prop: String, old: String? = null, value: String? = null, date: Boolean = false, duration: String? = null) =
        HistoryChange(kind, prop, old, value, date, duration)

    private fun say(x: HistoryChange) = describeChange(x, show)

    // The sentences are the ones the real `task` 3.5.0 printed for the same changes.
    @Test
    fun saysWhatWasSetWithDatesInTheDateFormat() {
        assertEquals("Description set to 'Alpha'.", say(c("set", "description", value = "Alpha")))
        assertEquals("Due set to '<1760000000>'.", say(c("set", "due", value = "1760000000", date = true)))
        assertEquals("Priority set to 'H'.", say(c("set", "priority", value = "H")))
    }

    @Test
    fun saysWhatAPropertyChangedFromAndTo() {
        assertEquals("Project changed from 'home' to 'work'.", say(c("changed", "project", old = "home", value = "work")))
        assertEquals("Due changed from '<100>' to '<200>'.", say(c("changed", "due", old = "100", value = "200", date = true)))
        assertEquals("Status changed from 'pending' to 'completed'.", say(c("changed", "status", old = "pending", value = "completed")))
    }

    @Test
    fun showsADateValueThatIsNotANumberAsItIsLikeTaskwarrior() {
        assertEquals("Due set to 'soon'.", say(c("set", "due", value = "soon", date = true)))
        assertEquals("Due set to '0'.", say(c("set", "due", value = "0", date = true)))
    }

    @Test
    fun saysWhenAPropertyWasDeletedWithHowLongAStartHadRun() {
        assertEquals("Wait deleted.", say(c("deleted", "wait", old = "5", date = true)))
        assertEquals("Start deleted (duration: 0:00:03).", say(c("deleted", "start", old = "1", date = true, duration = "0:00:03")))
        assertEquals("Start deleted (duration: -3:10:28).", say(c("deleted", "start", old = "1", date = true, duration = "-3:10:28")))
        // No beginning in the history to measure from: say no more than is known.
        assertEquals("Start deleted.", say(c("deleted", "start", old = "1", date = true)))
    }

    @Test
    fun namesTagsAnnotationsAndDependencies() {
        assertEquals("Tag 'y' added.", say(c("tag_added", "y")))
        assertEquals("Tag 'x' deleted.", say(c("tag_deleted", "x")))
        assertEquals("Annotation of 'a note' added.", say(c("note_added", "annotation_1", value = "a note")))
        assertEquals("Annotation changed to 'b'.", say(c("note_changed", "annotation_1", old = "a", value = "b")))
        assertEquals("Annotation 'gone' deleted.", say(c("note_deleted", "annotation_1", old = "gone")))
        assertEquals("Dependency on 'abc-123' added.", say(c("dep_added", "abc-123")))
        assertEquals("Dependency on 'abc-123' deleted.", say(c("dep_deleted", "abc-123")))
    }
}
