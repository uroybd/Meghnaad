package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class BulkTest {
    private val u1 = "11111111-1111-1111-1111-111111111111"
    private val u2 = "22222222-2222-2222-2222-222222222222"

    @Test
    fun listsTaskNumbersAsShortRanges() {
        assertEquals("1,3-5,9", idList(listOf(1, 3, 4, 5, 9)))
        assertEquals("3-5", idList(listOf(5, 3, 4)))
        assertEquals("1,2", idList(listOf(1, 2)))
        assertEquals("7", idList(listOf(7, 7, 7)))
        assertEquals("", idList(emptyList()))
    }

    @Test
    fun givesTheNumbersAsOneListAndTheWholeUuidOfATaskThatHasNone() {
        val tasks = listOf(Pick(u1, 3), Pick(u2, null), Pick("x", 1))
        assertEquals(listOf("1,3", u2), namedWords(tasks))
        assertEquals("1,3 $u2 ", promptFor(tasks))
        assertEquals("4 ", promptFor(listOf(Pick(u1, 4))))
    }

    @Test
    fun namesTasksToAButtonsCommandByTheirWholeUuids() {
        assertEquals(listOf(u1, u2), filterWords(listOf(Pick(u1, 3), Pick(u2, null))))
    }

    @Test
    fun splitsALongSelectionIntoCommands() {
        val rows = List(250) { it }
        val parts = batches(rows)
        assertEquals(listOf(120, 120, 10), parts.map { it.size })
        assertEquals(rows, parts.flatten())
        assertEquals(emptyList(), batches(emptyList<Int>()))
    }

    private val fields = fieldsFor(
        mapOf(
            "estimate" to UdaDef(name = "estimate", type = "numeric", label = "Estimate"),
            "size" to UdaDef(name = "size", type = "string", values = listOf("S", "M")),
        ),
    )

    @Test
    fun offersTheUsualFieldsTagsToAddAndRemoveAndTheTaskrcsUdas() {
        assertEquals(
            listOf("project", "priority", "due", "wait", "scheduled", "until", "recur", "+", "-", "depends", "estimate", "size"),
            fields.map { it.key },
        )
        assertEquals("Estimate", fields.first { it.key == "estimate" }.label)
        assertEquals(listOf("S", "M"), fields.first { it.key == "size" }.values)
    }

    @Test
    fun writesTheWordsModifyTakes() {
        assertEquals(
            listOf("project:Home", "due:", "+a", "+b", "+c", "-old", "estimate:3"),
            modifyWords(
                listOf(
                    ModifyEntry("project", " Home "), ModifyEntry("due", ""), ModifyEntry("+", "a, +b  c"),
                    ModifyEntry("-", "old"), ModifyEntry("estimate", "3"),
                ),
            ),
        )
    }

    @Test
    fun saysWhatItWillDo() {
        assertEquals(
            "set project to Home, clear due, add +a +b, remove +old",
            describe(listOf(ModifyEntry("project", "Home"), ModifyEntry("due", ""), ModifyEntry("+", "a b"), ModifyEntry("-", "old")), fields),
        )
    }

    @Test
    fun refusesAnEntryThatCannotBeApplied() {
        assertNull(problem(ModifyEntry("project", ""), fields)) // empty clears
        assertTrue(problem(ModifyEntry("+", "  "), fields)!!.contains("Add tags"))
        assertNotNull(problem(ModifyEntry("nope", "x"), fields))
        assertTrue(problem(ModifyEntry("nope", "x"), fields)!!.contains("Choose a field"))
    }
}
