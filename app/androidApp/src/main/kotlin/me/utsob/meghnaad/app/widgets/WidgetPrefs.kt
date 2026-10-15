package me.utsob.meghnaad

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject

/** The columns a task list can show under each task's description, and what to call them in the settings. */
object ListFields {
    val ALL = listOf(
        "project" to "Project",
        "due" to "Due date",
        "priority" to "Priority",
        "tags" to "Tags",
        "urgency" to "Urgency",
        "age" to "Age",
        "scheduled" to "Scheduled",
        "wait" to "Wait until",
        "recur" to "Recurrence",
        "id" to "Task number",
        "annotations" to "Notes",
    )
    val DEFAULT = listOf("project", "due", "tags")
}

/** A task list widget: which report, narrowed how, how many tasks, and what is shown under each. */
data class ListConfig(
    val report: String = "next",
    val filter: String = "",
    val limit: Int = 12,
    val fields: List<String> = ListFields.DEFAULT,
    val title: String = "",
    /** The report's own description ("Most urgent tasks"), kept here so the title can be drawn without asking the engine. */
    val description: String = "",
) {
    /** What the widget calls itself: the title it was given, else what the report is described as, else the report's name. */
    fun heading() = title.ifBlank { description.ifBlank { report } }

    fun toJson(): JSONObject = JSONObject()
        .put("report", report)
        .put("filter", filter)
        .put("limit", limit)
        .put("fields", JSONArray(fields))
        .put("title", title)
        .put("description", description)

    companion object {
        fun fromJson(j: JSONObject): ListConfig {
            val fields = j.optJSONArray("fields")?.let { a -> (0 until a.length()).map { a.getString(it) } } ?: ListFields.DEFAULT
            return ListConfig(
                j.optString("report", "next").ifBlank { "next" },
                j.optString("filter", ""),
                j.optInt("limit", 12).coerceIn(1, 50),
                fields,
                j.optString("title", ""),
                j.optString("description", ""),
            )
        }
    }
}

/** A burndown chart widget: daily, weekly or monthly, of the tasks the filter picks. */
data class BurndownConfig(val period: String = "daily", val filter: String = "") {
    fun toJson(): JSONObject = JSONObject().put("period", period).put("filter", filter)

    companion object {
        val PERIODS = listOf("daily", "weekly", "monthly")

        fun fromJson(j: JSONObject) = BurndownConfig(
            j.optString("period", "daily").takeIf { it in PERIODS } ?: "daily",
            j.optString("filter", ""),
        )
    }
}

/** A calendar widget: a month, with the days that have something due marked, for the tasks the filter picks. */
data class CalendarConfig(val filter: String = "") {
    fun toJson(): JSONObject = JSONObject().put("filter", filter)

    companion object {
        fun fromJson(j: JSONObject) = CalendarConfig(j.optString("filter", ""))
    }
}

/** What each widget on the home screen was set up to show, by the id the system gave it. */
object WidgetPrefs {
    private fun prefs(context: Context) = context.applicationContext.getSharedPreferences("widgets", Context.MODE_PRIVATE)

    private fun read(context: Context, kind: String, id: Int): JSONObject? =
        prefs(context).getString("$kind:$id", null)?.let { runCatching { JSONObject(it) }.getOrNull() }

    private fun write(context: Context, kind: String, id: Int, json: JSONObject) {
        prefs(context).edit().putString("$kind:$id", json.toString()).apply()
    }

    fun list(context: Context, id: Int): ListConfig = read(context, "list", id)?.let(ListConfig::fromJson) ?: ListConfig()

    fun saveList(context: Context, id: Int, config: ListConfig) = write(context, "list", id, config.toJson())

    fun burndown(context: Context, id: Int): BurndownConfig =
        read(context, "burndown", id)?.let(BurndownConfig::fromJson) ?: BurndownConfig()

    fun saveBurndown(context: Context, id: Int, config: BurndownConfig) = write(context, "burndown", id, config.toJson())

    fun calendar(context: Context, id: Int): CalendarConfig =
        read(context, "calendar", id)?.let(CalendarConfig::fromJson) ?: CalendarConfig()

    fun saveCalendar(context: Context, id: Int, config: CalendarConfig) = write(context, "calendar", id, config.toJson())

    fun look(context: Context, id: Int): WidgetLook = read(context, "look", id)?.let(WidgetLook::fromJson) ?: WidgetLook()

    fun saveLook(context: Context, id: Int, look: WidgetLook) = write(context, "look", id, look.toJson())

    /** How many months a calendar widget has been moved from this one (the buttons in its header). */
    fun monthOffset(context: Context, id: Int): Int = prefs(context).getInt("offset:$id", 0)

    fun setMonthOffset(context: Context, id: Int, offset: Int) {
        prefs(context).edit().putInt("offset:$id", offset).apply()
    }

    /** The day a calendar widget shows the tasks of (`2026-10-17`); null is today. */
    fun selectedDay(context: Context, id: Int): String? = prefs(context).getString("day:$id", null)

    fun selectDay(context: Context, id: Int, date: String?) {
        prefs(context).edit().apply { if (date == null) remove("day:$id") else putString("day:$id", date) }.apply()
    }

    /** A widget was removed from the home screen. */
    fun forget(context: Context, ids: IntArray) {
        val edit = prefs(context).edit()
        for (id in ids) for (kind in listOf("list", "burndown", "calendar", "look", "offset", "day")) edit.remove("$kind:$id")
        edit.apply()
    }
}
