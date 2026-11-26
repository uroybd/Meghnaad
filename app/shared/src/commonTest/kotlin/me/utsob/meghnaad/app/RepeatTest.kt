package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals

class RepeatTest {
    private val none = RepeatFields("", "", "")
    private val iterative = RepeatFields("", "monthly", "fixed")
    private val recurring = RepeatFields("weekly", "", "")

    @Test
    fun theModeIsReadFromTheFields() {
        assertEquals(RepeatMode.None, modeOf(none))
        assertEquals(RepeatMode.Iterative, modeOf(iterative))
        assertEquals(RepeatMode.Recurring, modeOf(recurring))
        assertEquals(RepeatMode.None, modeOf(none.copy(iterType = "fixed")))
    }

    @Test
    fun onlyWhatChangedInTheChosenModeIsSent() {
        assertEquals(emptyList(), repeatChanges(RepeatMode.None, none, none))
        assertEquals(listOf("iter:monthly", "iter_type:fixed"), repeatChanges(RepeatMode.Iterative, iterative, none))
        assertEquals(listOf("iter_type:chained"), repeatChanges(RepeatMode.Iterative, iterative.copy(iterType = "chained"), iterative))
        assertEquals(listOf("recur:weekly"), repeatChanges(RepeatMode.Recurring, recurring, none))
    }

    @Test
    fun theOtherModesFieldsAreClearedInTheSameCommand() {
        assertEquals(
            listOf("iter:", "iter_type:", "recur:weekly"),
            repeatChanges(RepeatMode.Recurring, iterative.copy(recur = "weekly"), iterative),
        )
        assertEquals(listOf("iter:", "iter_type:"), repeatChanges(RepeatMode.None, iterative, iterative))
        assertEquals(listOf("iter_type:"), repeatChanges(RepeatMode.None, none, none.copy(iterType = "fixed")))
        assertEquals(listOf("recur:"), repeatChanges(RepeatMode.None, none, recurring))
    }

    @Test
    fun aModeChosenButNotFilledInChangesNothing() {
        assertEquals(emptyList(), repeatChanges(RepeatMode.Iterative, none, none))
        assertEquals(emptyList(), repeatChanges(RepeatMode.Recurring, none, none))
        assertEquals(emptyList(), repeatChanges(RepeatMode.Recurring, iterative, iterative), "no period yet")
        assertEquals(emptyList(), repeatChanges(RepeatMode.Iterative, none.copy(iterType = "fixed"), none))
    }

    @Test
    fun aRepeatingTaskIsMarkedByHowItRepeats() {
        val plain = Row(uuid = "u", status = "pending", description = "x")
        assertEquals("repeat", repeatMark(plain.copy(status = "recurring", recur = "weekly"))?.icon)
        assertEquals("Repeats weekly", repeatMark(plain.copy(recur = "weekly"))?.words)
        val it = repeatMark(plain.copy(status = "iterative", iter = "monthly", iterCount = "3"))
        assertEquals("iteration-cw", it?.icon)
        assertEquals("Iterative: done, it makes the next (monthly); number 3", it?.words)
        assertEquals(null, repeatMark(plain))
        assertEquals(null, repeatMark(plain.copy(status = "completed", iterCount = "2")))
    }
}
