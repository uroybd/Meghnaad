package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals

// The buildTags cases of web/src/lib/tags.test.ts (toggleTag and activeTags are in CommandLineTest).
class TagsTest {
    private fun row(uuid: String, tags: List<String>, urgency: Double = 0.0, status: String = "pending", due: Long? = null) =
        Row(uuid = uuid, status = status, description = "d", tags = tags, urgency = urgency, due = due)

    @Test
    fun tasksAreGroupedUnderEachOfTheirTagsByNameMostUrgentFirstFinishedOnesIncluded() {
        val rows = listOf(
            row("1", listOf("b", "a"), urgency = 1.0),
            row("2", listOf("a"), urgency = 5.0, due = 10),
            row("3", listOf("a"), status = "completed"),
        )
        val t = buildTags(rows, 100)
        assertEquals(listOf("a", "b"), t.map { it.name })
        assertEquals(listOf("2", "1", "3"), t[0].tasks.map { it.uuid })
        assertEquals(1, t[0].overdue)
        assertEquals(1, t[1].tasks.size)
    }

    @Test
    fun anExtraTagIsListedEvenWhenNothingPendingHasIt() {
        val t = buildTags(emptyList(), 0, "done-only")
        assertEquals(listOf("done-only"), t.map { it.name })
        assertEquals(0, t[0].tasks.size)
        assertEquals(0, t[0].overdue)
    }
}
