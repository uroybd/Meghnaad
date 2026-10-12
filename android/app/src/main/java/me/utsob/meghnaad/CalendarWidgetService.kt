package me.utsob.meghnaad

import android.content.Context
import android.content.Intent
import android.widget.RemoteViews
import android.widget.RemoteViewsService
import org.json.JSONObject

/** One day of the month, as the widget's grid shows it. */
private class Cell(
    val day: Int?,
    /** `2026-10-17`, for the tap. */
    val date: String?,
    val today: Boolean,
    val dim: Boolean,
    val due: String,
    val scheduled: Boolean,
)

/** Feeds the calendar widget's grid: the days of its month, with their marks, from the engine. */
class CalendarWidgetService : RemoteViewsService() {
    override fun onGetViewFactory(intent: Intent): RemoteViewsFactory =
        Factory(applicationContext, intent.data?.lastPathSegment?.toIntOrNull() ?: 0)

    private class Factory(private val context: Context, private val widgetId: Int) : RemoteViewsFactory {
        private var cells: List<Cell> = emptyList()

        override fun onCreate() {}

        override fun onDataSetChanged() {
            cells = try {
                val (year, month) = CalendarWidget.month(context, widgetId)
                Tasks.calendar(context, WidgetPrefs.calendar(context, widgetId), year, month)
                    ?.optJSONArray("months")?.optJSONObject(0)?.let(::cellsOf) ?: emptyList()
            } catch (e: Exception) {
                android.util.Log.e("CalendarWidget", "could not read the month", e)
                emptyList()
            }
        }

        private fun cellsOf(month: JSONObject): List<Cell> {
            val prefix = String.format("%04d-%02d-", month.getInt("year"), month.getInt("month"))
            val out = mutableListOf<Cell>()
            val weeks = month.getJSONArray("weeks")
            for (w in 0 until weeks.length()) {
                val days = weeks.getJSONObject(w).getJSONArray("days")
                for (d in 0 until days.length()) {
                    if (days.isNull(d)) {
                        out += Cell(null, null, false, false, "", false)
                        continue
                    }
                    val day = days.getJSONObject(d)
                    val n = day.getInt("day")
                    out += Cell(
                        n,
                        prefix + String.format("%02d", n),
                        day.optBoolean("today"),
                        day.optBoolean("weekend") || day.optBoolean("holiday"),
                        day.optString("due"),
                        day.optBoolean("scheduled"),
                    )
                }
            }
            return out
        }

        override fun onDestroy() {}

        override fun getCount() = cells.size

        override fun getViewAt(position: Int): RemoteViews {
            val cell = cells[position]
            val palette = Palette(context, WidgetPrefs.look(context, widgetId))
            val row = RemoteViews(context.packageName, R.layout.widget_calendar_day)
            if (cell.day == null) {
                row.setTextViewText(R.id.day, "")
                return row
            }
            row.setTextViewText(R.id.day, cell.day.toString())
            row.setTextColor(R.id.day, if (cell.today) 0xFFFFFFFF.toInt() else if (cell.dim) palette.dim else palette.text)
            if (cell.today) {
                row.setViewVisibility(R.id.today, android.view.View.VISIBLE)
                row.setInt(R.id.today, "setColorFilter", palette.accent)
            }
            val dot = when (cell.due) {
                "overdue" -> palette.overdue
                "due-today" -> palette.dueToday
                "due" -> palette.due
                else -> if (cell.scheduled) palette.dim else null
            }
            if (dot != null) {
                row.setViewVisibility(R.id.dot, android.view.View.VISIBLE)
                row.setInt(R.id.dot, "setColorFilter", if (cell.today) 0xFFFFFFFF.toInt() else dot)
            }
            row.setOnClickFillInIntent(R.id.cell, Intent().putExtra("kind", "day").putExtra("date", cell.date))
            return row
        }

        override fun getLoadingView(): RemoteViews? = null

        override fun getViewTypeCount() = 1

        override fun getItemId(position: Int) = position.toLong()

        override fun hasStableIds() = true
    }
}
