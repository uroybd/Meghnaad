package me.utsob.meghnaad

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class FieldsTest {
    private val now = 1_791_799_200L // 2026-10-12

    private fun row(vararg pairs: Pair<String, Any?>): JSONObject {
        val json = JSONObject()
        for ((k, v) in pairs) json.put(k, v ?: JSONObject.NULL)
        return json
    }

    @Test
    fun aFieldIsWhatTheTaskHasThereAndNothingWhenItHasNone() {
        val task = row(
            "project" to "Home.Kitchen",
            "priority" to "H",
            "tags" to JSONArray(listOf("errand", "now")),
            "urgency" to 12.34,
            "id" to 7,
            "recur" to "weekly",
            "annotations" to JSONArray(listOf("a", "b")),
        )
        assertEquals("Home.Kitchen", Fields.text(task, "project", now))
        assertEquals("priority H", Fields.text(task, "priority", now))
        assertEquals("+errand +now", Fields.text(task, "tags", now))
        assertEquals("urgency 12.3", Fields.text(task, "urgency", now))
        assertEquals("#7", Fields.text(task, "id", now))
        assertEquals("every weekly", Fields.text(task, "recur", now))
        assertEquals("2 notes", Fields.text(task, "annotations", now))

        val bare = row("project" to null, "priority" to null, "tags" to JSONArray(), "due" to null, "id" to null)
        for (field in listOf("project", "priority", "tags", "due", "id", "annotations", "scheduled", "wait", "age", "recur", "nonsense"))
            assertNull(field, Fields.text(bare, field, now))
    }

    @Test
    fun theLineHasOnlyWhatIsThereInTheOrderChosen() {
        val task = row("project" to "Home", "priority" to null, "tags" to JSONArray(listOf("a")))
        assertEquals("+a · Home", Fields.line(task, listOf("tags", "priority", "project", "due"), now))
        assertEquals("", Fields.line(task, listOf("priority", "due"), now))
    }

    @Test
    fun aDateLeavesOutThisYearAndKeepsAnotherOne() {
        val thisYear = Fields.date(now, now)
        assertFalse(thisYear, thisYear.contains("2026"))
        val nextYear = Fields.date(now + 400L * 86_400, now)
        assertTrue(nextYear, nextYear.contains("2027"))
    }

    @Test
    fun anAgeIsInTheLargestUnitThatFits() {
        assertEquals("5min", Fields.age(5 * 60))
        assertEquals("3h", Fields.age(3 * 3600 + 10))
        assertEquals("4d", Fields.age(4 * 86_400))
        assertEquals("3w", Fields.age(21 * 86_400))
        assertEquals("4mo", Fields.age(125 * 86_400))
        assertEquals("2y", Fields.age(800L * 86_400))
        assertEquals("0min", Fields.age(-30))
    }

    @Test
    fun aListWidgetsSettingsSurviveBeingSavedAndStayWithinLimits() {
        val config = ListConfig("overdue", "project:Home", 20, listOf("due", "urgency"), "Late")
        assertEquals(config, ListConfig.fromJson(config.toJson()))
        assertEquals("Late", config.heading())
        assertEquals("next", ListConfig().heading())
        // What a later version or a damaged file might hold falls back to something usable.
        val odd = ListConfig.fromJson(JSONObject("""{"report":"","limit":9999}"""))
        assertEquals("next", odd.report)
        assertEquals(50, odd.limit)
        assertEquals(ListFields.DEFAULT, odd.fields)
        assertEquals("daily", BurndownConfig.fromJson(JSONObject("""{"period":"hourly"}""")).period)
    }
}
