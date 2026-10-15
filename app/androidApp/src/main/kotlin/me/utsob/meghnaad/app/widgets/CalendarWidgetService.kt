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
            palette.setTextColor(row, R.id.day, if (cell.today) Palette.WHITE else Palette.TEXT)
            // A day of another month is quieter by being fainter, which is the same in both themes.
            if (cell.dim && !cell.today) row.setFloat(R.id.day, "setAlpha", 0.55f)
            if (cell.today) {
                row.setViewVisibility(R.id.today, android.view.View.VISIBLE)
                palette.setTint(row, R.id.today, Palette.ACCENT)
            }
            val dot = when (cell.due) {
                "overdue" -> Palette.OVERDUE
                "due-today" -> Palette.DUE_TODAY
                "due" -> Palette.DUE
                else -> if (cell.scheduled) Palette.DIM else null
            }
            if (dot != null) {
                row.setViewVisibility(R.id.dot, android.view.View.VISIBLE)
                // The dots that differ between the themes are drawables with their colour from a resource (see Palette.set).
                if (!cell.today && dot === Palette.OVERDUE) row.setImageViewResource(R.id.dot, R.drawable.circle_overdue)
                else if (!cell.today && dot === Palette.DIM) row.setImageViewResource(R.id.dot, R.drawable.circle_dim)
                palette.setTint(row, R.id.dot, if (cell.today) Palette.WHITE else dot)
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
