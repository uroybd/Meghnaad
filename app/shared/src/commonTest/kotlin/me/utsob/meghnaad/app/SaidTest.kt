package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

// web/src/lib/said.test.ts
class SaidTest {
    private val reminder = listOf("Task 3 is blocked by:", "  1 a", "  2 b", "and is blocking:", "  4 d")
    private fun item(note: List<String> = emptyList()) =
        ConfirmItem(key = "dep:u", uuid = "u", id = 3, description = "c", question = "Would you like the dependency chain fixed?", note = note)

    @Test
    fun theReminderComesBeforeTheQuestionAsTaskPrintsIt() {
        assertEquals(reminder.joinToString("\n") + "\nWould you like the dependency chain fixed?", questionText(item(reminder)))
        assertEquals("Would you like the dependency chain fixed?", questionText(item()))
    }

    @Test
    fun oneQuestionAsItIsAndSeveralUnderTheirHeading() {
        val one = ConfirmResult("x", "extras", listOf(item(reminder)))
        assertEquals(questionText(item(reminder)), confirmText(one))
        val two = ConfirmResult("A few more questions about these tasks.", "extras", listOf(item(listOf("n")), item()))
        assertEquals("A few more questions about these tasks.\nn", confirmText(two))
    }

    @Test
    fun eachLineItsOwnLineAndAWarningWhenOneIs() {
        val lines = listOf(HookLine("warn", "Task 3 is blocked by:"), HookLine("warn", "  1 a"))
        assertEquals("Task 3 is blocked by:\n1 a", saidText(lines))
        assertTrue(saidBad(lines))
        assertFalse(saidBad(listOf(HookLine("info", "hi"))))
    }
}
