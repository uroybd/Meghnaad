package me.utsob.meghnaad

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.text.DateFormat
import java.util.Date
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

    /** The first [limit] tasks of [report]. */
    fun report(context: Context, report: String, limit: Int): List<WidgetTask> {
        val answer = run(context, "args" to JSONArray(listOf(report, "limit:$limit")))
        val result = answer.optJSONObject("result") ?: return emptyList()
        if (result.optString("kind") != "report") {
            android.util.Log.w("TasksWidget", "the engine answered ${answer.toString().take(300)}")
            return emptyList()
        }
        val rows = result.optJSONArray("rows") ?: return emptyList()
        val soon = System.currentTimeMillis() / 1000
        return (0 until rows.length()).map { i ->
            val row = rows.getJSONObject(i)
            val due = if (row.isNull("due")) null else row.getLong("due")
            val parts = mutableListOf<String>()
            if (!row.isNull("project")) parts += row.getString("project")
            if (due != null) parts += DateFormat.getDateInstance(DateFormat.MEDIUM).format(Date(due * 1000))
            row.optJSONArray("tags")?.let { tags ->
                if (tags.length() > 0) parts += (0 until tags.length()).joinToString(" ") { "+" + tags.getString(it) }
            }
            WidgetTask(row.getString("uuid"), row.getString("description"), parts.joinToString(" · "), due != null && due < soon)
        }
    }

    /** Mark a task done. Null when it worked, the reason when it did not. */
    fun done(context: Context, uuid: String): String? =
        problem(run(context, "args" to JSONArray(listOf(uuid, "done")), "confirmed" to true))

    /** Add a task from a line as typed (`Buy milk due:tomorrow +home`). Null when it worked, the reason when it did not. */
    fun add(context: Context, line: String): String? = problem(run(context, "line" to "add $line"))
}
