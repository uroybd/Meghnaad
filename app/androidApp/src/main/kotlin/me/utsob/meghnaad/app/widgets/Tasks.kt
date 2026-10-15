package me.utsob.meghnaad

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.util.TimeZone

/** One task as the widget shows it. */
data class WidgetTask(val uuid: String, val description: String, val detail: String, val overdue: Boolean)

/** The tasks of one day of the calendar: the first few of them, and how many there are in all. */
data class DayTasks(val tasks: List<WidgetTask>, val total: Int)

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

    /** The reports there are (the built-in ones and the user's) with how each is described, hidden ones left out; null when the engine could not say. */
    private fun reports(context: Context): List<Pair<String, String>>? = try {
        val answer = JSONObject(Engines.get(context).call("api", JSONObject().put("method", "GET").put("path", "/api/config").toString()))
        val reports = JSONObject(answer.getString("body")).getJSONArray("reports")
        (0 until reports.length()).map { reports.getJSONObject(it) }
            .map { it.getString("name") to (if (it.isNull("description")) "" else it.optString("description", "")) }
            .filter { !it.first.startsWith("_") }
    } catch (e: Exception) {
        null
    }

    /** The names of the reports there are, for choosing one. */
    fun reportNames(context: Context): List<String> = reports(context)?.map { it.first } ?: listOf("next")

    /** How a report is described (`next`: "Most urgent tasks"), empty when it has no description; null when the engine could not say. */
    fun reportDescription(context: Context, report: String): String? {
        val all = reports(context) ?: return null
        return all.firstOrNull { it.first == report }?.second ?: ""
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

    /**
     * The tasks due or scheduled on [date] (`2026-10-17`), the way the app lists them for a day: the `list` report, narrowed by the
     * widget's filter. Only the first [limit] come, and how many there are.
     */
    fun day(context: Context, config: CalendarConfig, date: String, limit: Int): DayTasks {
        val filter = config.filter.trim().split(Regex("\\s+")).filter { it.isNotEmpty() }
        val args = JSONArray(listOf("list", "(", "due:$date", "or", "scheduled:$date", ")") + filter + "limit:$limit")
        val result = run(context, "args" to args).optJSONObject("result")
        return if (result?.optString("kind") == "report") dayOf(result, date, System.currentTimeMillis() / 1000) else DayTasks(emptyList(), 0)
    }

    /** The day's tasks out of a report answer: each with what it is for that day (a time it is due, or that it is scheduled), and its project. */
    fun dayOf(report: JSONObject, date: String, nowSecs: Long, zone: TimeZone = TimeZone.getDefault()): DayTasks {
        val rows = report.optJSONArray("rows") ?: return DayTasks(emptyList(), 0)
        fun long(row: JSONObject, key: String) = if (row.isNull(key)) null else row.optLong(key)
        fun day(epoch: Long) = java.text.SimpleDateFormat("yyyy-MM-dd", java.util.Locale.US).apply { timeZone = zone }.format(java.util.Date(epoch * 1000))
        fun time(epoch: Long): String? {
            val c = java.util.Calendar.getInstance(zone).apply { timeInMillis = epoch * 1000 }
            if (c.get(java.util.Calendar.HOUR_OF_DAY) == 0 && c.get(java.util.Calendar.MINUTE) == 0) return null
            return java.text.DateFormat.getTimeInstance(java.text.DateFormat.SHORT).apply { timeZone = zone }.format(java.util.Date(epoch * 1000))
        }
        val tasks = (0 until rows.length()).map { i ->
            val row = rows.getJSONObject(i)
            val due = long(row, "due")
            val scheduled = long(row, "scheduled")
            val mark = when {
                due != null && day(due) == date -> time(due) ?: "due"
                scheduled != null && day(scheduled) == date -> "scheduled" + (time(scheduled)?.let { " $it" } ?: "")
                else -> null
            }
            val project = if (row.isNull("project")) null else row.optString("project").takeIf { it.isNotEmpty() }
            WidgetTask(row.getString("uuid"), row.getString("description"), listOfNotNull(mark, project).joinToString(" · "), due != null && due < nowSecs)
        }
        return DayTasks(tasks, report.optInt("matched", tasks.size))
    }

    /** Mark a task done. Null when it worked, the reason when it did not. */
    fun done(context: Context, uuid: String): String? =
        problem(run(context, "args" to JSONArray(listOf(uuid, "done")), "confirmed" to true))

    /** Add a task from a line as typed (`Buy milk due:tomorrow +home`). Null when it worked, the reason when it did not. */
    fun add(context: Context, line: String): String? = problem(run(context, "line" to "add $line"))
}
