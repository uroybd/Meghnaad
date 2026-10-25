package me.utsob.meghnaad

import me.utsob.meghnaad.app.R
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
        private var selected: String = ""

        override fun onCreate() {}

        override fun onDataSetChanged() {
            selected = CalendarWidget.selected(context, widgetId)
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
                for (mark in listOf(R.id.today, R.id.selected, R.id.dot)) row.setViewVisibility(mark, android.view.View.GONE)
                return row
            }
            row.setTextViewText(R.id.day, cell.day.toString())
            palette.setTextColor(row, R.id.day, if (cell.today) Palette.WHITE else Palette.TEXT)
            // Everything is set every time: the home screen draws a cell over the views of another day, and what is not set stays
            // as it was.
            // A day of another month is quieter by being fainter, which is the same in both themes.
            row.setFloat(R.id.day, "setAlpha", if (cell.dim && !cell.today) 0.55f else 1f)
            row.setViewVisibility(R.id.today, if (cell.today) android.view.View.VISIBLE else android.view.View.GONE)
            // The day whose tasks are shown, when it is not today (which is marked already).
            val picked = !cell.today && cell.date == selected
            row.setViewVisibility(R.id.selected, if (picked) android.view.View.VISIBLE else android.view.View.GONE)
            if (cell.today) palette.setTint(row, R.id.today, Palette.ACCENT)
            else if (picked) palette.setTint(row, R.id.selected, Palette.ACCENT)
            val dot = when (cell.due) {
                "overdue" -> Palette.OVERDUE
                "due-today" -> Palette.DUE_TODAY
                "due" -> Palette.DUE
                else -> if (cell.scheduled) Palette.DIM else null
            }
            row.setViewVisibility(R.id.dot, if (dot != null) android.view.View.VISIBLE else android.view.View.GONE)
            if (dot != null) {
                // The dots that differ between the themes are drawables with their colour from a resource (see Palette.set).
                row.setImageViewResource(
                    R.id.dot,
                    when {
                        !cell.today && dot === Palette.OVERDUE -> R.drawable.circle_overdue
                        !cell.today && dot === Palette.DIM -> R.drawable.circle_dim
                        else -> R.drawable.circle_filled
                    },
                )
                palette.setTint(row, R.id.dot, if (cell.today) Palette.WHITE else dot)
            }
            // A tap selects the day (the widget answers it); the app opens from the day's own pane.
            row.setOnClickFillInIntent(R.id.cell, Intent().putExtra("date", cell.date))
            return row
        }

        override fun getLoadingView(): RemoteViews? = null

        override fun getViewTypeCount() = 1

        override fun getItemId(position: Int) = position.toLong()

        override fun hasStableIds() = true
    }
}
