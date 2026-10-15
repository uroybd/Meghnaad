package me.utsob.meghnaad

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Test
import java.util.TimeZone

class DayTasksTest {
    private val utc = TimeZone.getTimeZone("UTC")
    private val noon = 1_792_238_400L // 2026-10-17 12:00 UTC
    private val midnight = 1_792_195_200L // 2026-10-17 00:00 UTC

    private fun row(uuid: String, description: String, due: Long? = null, scheduled: Long? = null, project: String? = null) = JSONObject()
        .put("uuid", uuid)
        .put("description", description)
        .put("due", due ?: JSONObject.NULL)
        .put("scheduled", scheduled ?: JSONObject.NULL)
        .put("project", project ?: JSONObject.NULL)

    private fun report(matched: Int, vararg rows: JSONObject) = JSONObject().put("kind", "report").put("matched", matched).put("rows", JSONArray(rows.toList()))

    @Test
    fun theTotalIsWhatMatchedNotWhatCame() {
        val day = Tasks.dayOf(report(5, row("a", "One", due = midnight), row("b", "Two", due = midnight)), "2026-10-17", noon, utc)
        assertEquals(2, day.tasks.size)
        assertEquals(5, day.total)
        assertEquals(DayTasks(emptyList(), 0), Tasks.dayOf(JSONObject().put("kind", "report"), "2026-10-17", noon, utc))
    }

    @Test
    fun aTaskSaysWhatItIsForThatDay() {
        val timed = Tasks.dayOf(report(1, row("a", "Call", due = noon + 5400, project = "Work")), "2026-10-17", noon, utc).tasks[0]
        // Due at 13:30 that day: the time, in the phone's own style, then the project.
        val expectedTime = java.text.DateFormat.getTimeInstance(java.text.DateFormat.SHORT).apply { timeZone = utc }.format(java.util.Date((noon + 5400) * 1000))
        assertEquals("$expectedTime · Work", timed.detail)
        // Due with no time of day is just due; scheduled that day says so; the project alone is the project.
        assertEquals("due", Tasks.dayOf(report(1, row("a", "x", due = midnight)), "2026-10-17", noon, utc).tasks[0].detail)
        assertEquals("scheduled · Home", Tasks.dayOf(report(1, row("a", "x", scheduled = midnight, project = "Home")), "2026-10-17", noon, utc).tasks[0].detail)
        assertEquals("", Tasks.dayOf(report(1, row("a", "x")), "2026-10-17", noon, utc).tasks[0].detail)
    }

    @Test
    fun aTaskDueBeforeNowIsOverdue() {
        val tasks = Tasks.dayOf(report(2, row("a", "Late", due = midnight), row("b", "Later", due = noon + 3600)), "2026-10-17", noon, utc).tasks
        assertEquals(listOf(true, false), tasks.map { it.overdue })
    }
}
