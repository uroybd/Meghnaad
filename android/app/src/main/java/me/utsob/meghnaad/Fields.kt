package me.utsob.meghnaad

import org.json.JSONObject
import java.text.DateFormat
import java.util.Date
import java.util.Locale

/** What to show of a task under its description, worked out from a row of a report. */
object Fields {
    /** The text for one field of a row, or null when the task has nothing there. */
    fun text(row: JSONObject, field: String, nowSecs: Long): String? = when (field) {
        "project" -> row.optStringOrNull("project")
        "priority" -> row.optStringOrNull("priority")?.let { "priority $it" }
        "tags" -> row.optJSONArray("tags")?.let { tags ->
            (0 until tags.length()).joinToString(" ") { "+" + tags.getString(it) }.ifEmpty { null }
        }
        "due" -> row.optLongOrNull("due")?.let { date(it, nowSecs) }
        "scheduled" -> row.optLongOrNull("scheduled")?.let { "scheduled " + date(it, nowSecs) }
        "wait" -> row.optLongOrNull("wait")?.let { "waits until " + date(it, nowSecs) }
        "age" -> row.optLongOrNull("entry")?.let { age(nowSecs - it) }
        "urgency" -> if (row.has("urgency")) String.format(Locale.US, "urgency %.1f", row.optDouble("urgency")) else null
        "recur" -> row.optStringOrNull("recur")?.let { "every $it" }
        "id" -> if (row.isNull("id")) null else "#" + row.optInt("id")
        "annotations" -> row.optJSONArray("annotations")?.length()?.takeIf { it > 0 }?.let { if (it == 1) "1 note" else "$it notes" }
        else -> null
    }

    /** The fields that have something, in the order the widget was set up with, joined for one line. */
    fun line(row: JSONObject, fields: List<String>, nowSecs: Long): String =
        fields.mapNotNull { text(row, it, nowSecs) }.joinToString(" · ")

    /** `Oct 13`, or `Oct 13, 2027` when it is not this year. */
    fun date(epoch: Long, nowSecs: Long): String {
        val year = java.util.Calendar.getInstance().apply { timeInMillis = nowSecs * 1000 }.get(java.util.Calendar.YEAR)
        val that = java.util.Calendar.getInstance().apply { timeInMillis = epoch * 1000 }.get(java.util.Calendar.YEAR)
        val format = if (year == that) DateFormat.getDateInstance(DateFormat.MEDIUM) else DateFormat.getDateInstance(DateFormat.LONG)
        val text = format.format(Date(epoch * 1000))
        // The medium style carries the year; leave it out for this year.
        return if (year == that) text.replace(Regex(",?\\s*$year"), "").trim() else text
    }

    /** How old, as Taskwarrior writes it: `3d`, `5h`, `2w`. */
    fun age(seconds: Long): String {
        val s = seconds.coerceAtLeast(0)
        return when {
            s < 3600 -> "${s / 60}min"
            s < 86_400 -> "${s / 3600}h"
            s < 14 * 86_400 -> "${s / 86_400}d"
            s < 60 * 86_400 -> "${s / (7 * 86_400)}w"
            s < 365 * 86_400 -> "${s / (30 * 86_400)}mo"
            else -> "${s / (365 * 86_400)}y"
        }
    }

    private fun JSONObject.optStringOrNull(key: String): String? =
        if (isNull(key)) null else optString(key).takeIf { it.isNotEmpty() }

    private fun JSONObject.optLongOrNull(key: String): Long? = if (isNull(key)) null else optLong(key)
}
