package me.utsob.meghnaad.app.widgets

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.util.TimeZone

/** One task as the widget shows it. */
data class WidgetTask(val uuid: String, val description: String, val detail: String, val overdue: Boolean)

/** The few things the widget asks of the engine, as ordinary commands. */
object Tasks {
    private fun request(vararg fields: Pair<String, Any>): String {
        val now = System.currentTimeMillis() / 1000.0
        val json = JSONObject().put("now", now).put("tz", TimeZone.getDefault().getOffset(System.currentTimeMillis()) / 1000)
        fields.forEach { (k, v) -> json.put(k, v) }
        return json.toString()
    }

    private fun run(context: Context, vararg fields: Pair<String, Any>): JSONObject =
        JSONObject(Engines.get(context).call("run", request(*fields)))

    /** What went wrong, from an engine answer; null when it worked. */
    private fun problem(answer: JSONObject): String? {
        answer.optString("error").takeIf { it.isNotEmpty() }?.let { return it }
        val result = answer.optJSONObject("result") ?: return null
        return when (result.optString("kind")) {
            "error" -> result.optString("message")
            "confirm" -> "That needs an answer: open Meghnaad."
            else -> null
        }
    }

    /** The tasks a list widget shows: its report, narrowed by its filter, as many as it was set up to show. */
    fun report(context: Context, config: ListConfig): List<WidgetTask> {
        val line = listOf(config.report, config.filter.trim(), "limit:${config.limit}").filter { it.isNotEmpty() }.joinToString(" ")
        val answer = run(context, "line" to line)
        val result = answer.optJSONObject("result") ?: return emptyList()
        if (result.optString("kind") != "report") {
            android.util.Log.w("TasksWidget", "the engine answered ${answer.toString().take(300)}")
            return emptyList()
        }
        val rows = result.optJSONArray("rows") ?: return emptyList()
        val now = System.currentTimeMillis() / 1000
        return (0 until rows.length()).map { i ->
            val row = rows.getJSONObject(i)
            val due = if (row.isNull("due")) null else row.getLong("due")
            WidgetTask(
                row.getString("uuid"),
                row.getString("description"),
                Fields.line(row, config.fields, now),
                due != null && due < now,
            )
        }
    }

    /** The names of the reports there are (the built-in ones and the user's), for choosing one. */
    fun reportNames(context: Context): List<String> = try {
        val answer = JSONObject(Engines.get(context).call("api", JSONObject().put("method", "GET").put("path", "/api/config").toString()))
        val body = JSONObject(answer.getString("body"))
        val reports = body.getJSONArray("reports")
        (0 until reports.length()).map { reports.getJSONObject(it).getString("name") }.filter { !it.startsWith("_") }
    } catch (e: Exception) {
        listOf("next")
    }

    /** A burndown (`burndown.daily|weekly|monthly`) of the tasks the filter picks; null when the engine had none to give. */
    fun burndown(context: Context, config: BurndownConfig): JSONObject? {
        val line = listOf("burndown.${config.period}", config.filter.trim()).filter { it.isNotEmpty() }.joinToString(" ")
        val result = run(context, "line" to line).optJSONObject("result") ?: return null
        return result.takeIf { it.optString("kind") == "burndown" }
    }

    /** One month of the calendar with what is due marked, for the tasks the filter picks. */
    fun calendar(context: Context, config: CalendarConfig, year: Int, month: Int): JSONObject? {
        val args = JSONArray(listOf("rc.calendar.monthsperline:1", "calendar", month.toString(), year.toString()) + config.filter.trim().split(Regex("\\s+")).filter { it.isNotEmpty() })
        val result = run(context, "args" to args).optJSONObject("result") ?: return null
        return result.takeIf { it.optString("kind") == "calendar" }
    }

    /** Mark a task done. Null when it worked, the reason when it did not. */
    fun done(context: Context, uuid: String): String? =
        problem(run(context, "args" to JSONArray(listOf(uuid, "done")), "confirmed" to true))

    /** Add a task from a line as typed (`Buy milk due:tomorrow +home`). Null when it worked, the reason when it did not. */
    fun add(context: Context, line: String): String? = problem(run(context, "line" to "add $line"))
}
