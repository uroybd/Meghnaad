package me.utsob.meghnaad

import me.utsob.meghnaad.app.R
import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.res.Configuration
import android.net.Uri
import android.os.Bundle
import android.view.View
import android.widget.RemoteViews
import org.json.JSONObject
import java.text.SimpleDateFormat
import java.util.Calendar
import java.util.Date
import java.util.Locale

/**
 * A month on the home screen: today circled, a dot on the days with something due, arrows to other months. A tap on a day
 * selects it, and when the widget has the room (see [CalendarLayout]) the tasks of that day (due or scheduled) are shown beside
 * the month or under it: two of them, and "Show all" to open the rest in the app.
 */
class CalendarWidget : AppWidgetProvider() {
    override fun onUpdate(context: Context, manager: AppWidgetManager, ids: IntArray) {
        for (id in ids) update(context, id)
    }

    override fun onEnabled(context: Context) {
        SyncWorker.schedule(context)
        SyncWorker.runNow(context)
    }

    override fun onDisabled(context: Context) {
        if (!Widgets.needsSync(context)) SyncWorker.cancel(context)
    }

    override fun onDeleted(context: Context, ids: IntArray) = WidgetPrefs.forget(context, ids)

    /** The widget was made bigger or smaller: it may now have the room for the day's tasks, or not. */
    override fun onAppWidgetOptionsChanged(context: Context, manager: AppWidgetManager, id: Int, newOptions: Bundle) {
        update(context, id)
    }

    override fun onReceive(context: Context, intent: Intent) {
        super.onReceive(context, intent)
        val id = intent.getIntExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, 0)
        when (intent.action) {
            ACTION_SYNC -> {
                SyncWorker.runNow(context)
                refresh(context)
            }
            ACTION_MONTH -> {
                val delta = intent.getIntExtra("delta", 0)
                WidgetPrefs.setMonthOffset(context, id, if (delta == 0) 0 else WidgetPrefs.monthOffset(context, id) + delta)
                // Back to this month is back to today as well.
                if (delta == 0) WidgetPrefs.selectDay(context, id, null)
                update(context, id)
            }
            // A day was tapped: it is the one whose tasks are shown.
            ACTION_SELECT -> {
                WidgetPrefs.selectDay(context, id, intent.getStringExtra("date"))
                update(context, id)
            }
        }
    }

    companion object {
        const val ACTION_SYNC = "me.utsob.meghnaad.CALENDAR_SYNC"
        const val ACTION_MONTH = "me.utsob.meghnaad.CALENDAR_MONTH"
        const val ACTION_SELECT = "me.utsob.meghnaad.CALENDAR_SELECT"

        /** How many tasks of the selected day are shown; the rest are a tap away. */
        const val SHOWN = 2

        /** The day a widget shows the tasks of: the one tapped, else today (`2026-10-17`). */
        fun selected(context: Context, id: Int): String =
            WidgetPrefs.selectedDay(context, id) ?: SimpleDateFormat("yyyy-MM-dd", Locale.US).format(Date())

        /** What the home screen gave the widget, in dp, for the way the phone is held; 0 when it did not say. */
        private fun room(context: Context, manager: AppWidgetManager, id: Int): Pair<Int, Int> {
            val options = manager.getAppWidgetOptions(id)
            val portrait = context.resources.configuration.orientation != Configuration.ORIENTATION_LANDSCAPE
            val width = options.getInt(if (portrait) AppWidgetManager.OPTION_APPWIDGET_MIN_WIDTH else AppWidgetManager.OPTION_APPWIDGET_MAX_WIDTH, 0)
            val height = options.getInt(if (portrait) AppWidgetManager.OPTION_APPWIDGET_MAX_HEIGHT else AppWidgetManager.OPTION_APPWIDGET_MIN_HEIGHT, 0)
            return width to height
        }

        /** The month a widget shows: this one, moved by the arrows. */
        fun month(context: Context, id: Int): Pair<Int, Int> {
            val target = Calendar.getInstance().apply { add(Calendar.MONTH, WidgetPrefs.monthOffset(context, id)) }
            return target.get(Calendar.YEAR) to target.get(Calendar.MONTH) + 1
        }

        /** Draw one widget again (its header from the engine, on a thread of its own; its days by the service). */
        fun update(context: Context, id: Int) {
            val app = context.applicationContext
            Thread {
                try {
                    val (year, month) = month(app, id)
                    val config = WidgetPrefs.calendar(app, id)
                    val result = Tasks.calendar(app, config, year, month)
                    val manager = AppWidgetManager.getInstance(app)
                    val (width, height) = room(app, manager, id)
                    val layout = CalendarLayout.choose(width, height)
                    val date = selected(app, id)
                    val day = if (layout == CalendarLayout.MONTH) null else Tasks.day(app, config, date, SHOWN)
                    manager.updateAppWidget(id, views(app, id, result, layout, date, day))
                    manager.notifyAppWidgetViewDataChanged(id, R.id.grid)
                } catch (e: Exception) {
                    android.util.Log.e("CalendarWidget", "could not draw widget $id", e)
                }
            }.start()
        }

        /** Draw every calendar widget again. */
        fun refresh(context: Context) {
            val manager = AppWidgetManager.getInstance(context)
            for (id in manager.getAppWidgetIds(ComponentName(context, CalendarWidget::class.java))) update(context, id)
        }

        private fun views(context: Context, id: Int, result: JSONObject?, layout: CalendarLayout, date: String, day: DayTasks?): RemoteViews {
            val views = RemoteViews(
                context.packageName,
                when (layout) {
                    CalendarLayout.MONTH -> R.layout.widget_calendar
                    CalendarLayout.WIDE -> R.layout.widget_calendar_wide
                    CalendarLayout.TALL -> R.layout.widget_calendar_tall
                },
            )
            val palette = Looks.apply(context, views, id, listOf(R.id.previous, R.id.next))
            val shown = result?.optJSONArray("months")?.optJSONObject(0)
            val (year, month) = month(context, id)
            views.setTextViewText(
                R.id.title,
                shown?.let { "${it.getString("name")} ${it.getInt("year")}" }
                    ?: Calendar.getInstance().apply { set(year, month - 1, 1) }
                        .getDisplayName(Calendar.MONTH, Calendar.LONG, java.util.Locale.getDefault()) + " $year",
            )
            val names = result?.optJSONArray("weekdays")
            val ids = listOf(R.id.wd0, R.id.wd1, R.id.wd2, R.id.wd3, R.id.wd4, R.id.wd5, R.id.wd6)
            for ((i, view) in ids.withIndex()) {
                views.setTextViewText(view, names?.optString(i) ?: listOf("Su", "Mo", "Tu", "We", "Th", "Fr", "Sa")[i])
                palette.setTextColor(views, view, Palette.DIM)
            }
            views.setTextViewText(R.id.caption, shown?.let(::caption) ?: "")
            palette.setTextColor(views, R.id.caption, Palette.DIM)

            // The days are the service's: one cell each, and a tap on one comes back as this template filled in with the day,
            // which selects it.
            val adapter = Intent(context, CalendarWidgetService::class.java).apply { data = Uri.parse("meghnaad://calendar/$id") }
            views.setRemoteAdapter(R.id.grid, adapter)
            views.setPendingIntentTemplate(
                R.id.grid,
                PendingIntent.getBroadcast(
                    context,
                    id * 10 + 5,
                    Intent(context, CalendarWidget::class.java).setAction(ACTION_SELECT).putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, id),
                    PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE,
                ),
            )
            if (day != null) fillDay(context, views, id, palette, date, day)
            views.setOnClickPendingIntent(R.id.previous, broadcast(context, id, 2, -1))
            views.setOnClickPendingIntent(R.id.next, broadcast(context, id, 3, 1))
            // A tap on the month's name goes back to this month.
            views.setOnClickPendingIntent(R.id.title, broadcast(context, id, 4, 0))
            views.setOnClickPendingIntent(R.id.refresh, broadcast(context, id, 1, 0, ACTION_SYNC))
            return views
        }

        /** `Sat, Oct 17` for `2026-10-17`, in the phone's own order and language. */
        fun dayTitle(date: String): String {
            val parsed = runCatching { SimpleDateFormat("yyyy-MM-dd", Locale.US).parse(date) }.getOrNull() ?: return date
            val pattern = android.text.format.DateFormat.getBestDateTimePattern(Locale.getDefault(), "EEEMMMd")
            return SimpleDateFormat(pattern, Locale.getDefault()).format(parsed)
        }

        private fun open(context: Context, id: Int, n: Int, vararg extras: Pair<String, String>, slot: String): PendingIntent {
            val intent = Intent(context, WidgetActionActivity::class.java)
                // Each its own: extras do not tell one pending intent from another, the address does.
                .setData(Uri.parse("meghnaad://calendar/$id/$slot"))
            for ((k, v) in extras) intent.putExtra(k, v)
            return PendingIntent.getActivity(context, id * 10 + n, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        }

        /** The selected day: its date and how many tasks, the first few of them, and a way to the rest. */
        private fun fillDay(context: Context, views: RemoteViews, id: Int, palette: Palette, date: String, day: DayTasks) {
            views.setTextViewText(R.id.day_title, dayTitle(date))
            views.setTextViewText(R.id.day_count, if (day.total == 0) "" else context.resources.getQuantityString(R.plurals.widget_day_count, day.total, day.total))
            palette.setTextColor(views, R.id.day_title, Palette.TEXT)
            palette.setTextColor(views, R.id.day_count, Palette.DIM)
            // The date opens the whole day in the app.
            views.setOnClickPendingIntent(R.id.day_head, open(context, id, 9, "kind" to "day", "date" to date, slot = "head"))

            val rows = listOf(
                Triple(R.id.task0, R.id.task0_text, listOf(R.id.task0_detail, R.id.task0_overdue)),
                Triple(R.id.task1, R.id.task1_text, listOf(R.id.task1_detail, R.id.task1_overdue)),
            )
            for ((slot, row) in rows.withIndex()) {
                val (container, text, lines) = row
                val task = day.tasks.getOrNull(slot)
                if (task == null) {
                    views.setViewVisibility(container, View.GONE)
                    continue
                }
                views.setViewVisibility(container, View.VISIBLE)
                views.setTextViewText(text, task.description)
                palette.setTextColor(views, text, Palette.TEXT)
                // An overdue task's line is the red one, which takes its colour from a resource (see Palette.set).
                val (calm, red) = lines
                val line = if (task.overdue) red else calm
                views.setTextViewText(line, task.detail)
                palette.setTextColor(views, line, if (task.overdue) Palette.OVERDUE else Palette.DIM)
                views.setViewVisibility(calm, if (!task.overdue && task.detail.isNotEmpty()) View.VISIBLE else View.GONE)
                views.setViewVisibility(red, if (task.overdue && task.detail.isNotEmpty()) View.VISIBLE else View.GONE)
                views.setOnClickPendingIntent(container, open(context, id, 6 + slot, "kind" to "open", "uuid" to task.uuid, slot = "task$slot"))
            }
            val more = day.total > SHOWN
            views.setViewVisibility(R.id.more, if (more) View.VISIBLE else View.GONE)
            if (more) {
                views.setTextViewText(R.id.more, context.getString(R.string.widget_show_all, day.total))
                palette.setTextColor(views, R.id.more, Palette.ACCENT)
                views.setOnClickPendingIntent(R.id.more, open(context, id, 8, "kind" to "day", "date" to date, slot = "more"))
            }
            views.setViewVisibility(R.id.empty, if (day.total == 0) View.VISIBLE else View.GONE)
            views.setTextViewText(R.id.empty, context.getString(R.string.widget_day_empty))
            palette.setTextColor(views, R.id.empty, Palette.DIM)
        }

        private fun broadcast(context: Context, id: Int, n: Int, delta: Int, action: String = ACTION_MONTH): PendingIntent =
            ImageWidget.broadcast(context, id, CalendarWidget::class.java, n, action, delta)

        /** How many days have something due, and how many of them are overdue, for the line under the month. */
        fun caption(month: JSONObject): String {
            var due = 0
            var overdue = 0
            val weeks = month.getJSONArray("weeks")
            for (w in 0 until weeks.length()) {
                val days = weeks.getJSONObject(w).getJSONArray("days")
                for (d in 0 until days.length()) {
                    if (days.isNull(d)) continue
                    when (days.getJSONObject(d).optString("due")) {
                        "overdue" -> {
                            overdue++
                            due++
                        }
                        "due-today", "due" -> due++
                    }
                }
            }
            return when {
                due == 0 -> "Nothing due this month"
                overdue > 0 -> "$due days with something due · $overdue overdue"
                else -> "$due days with something due"
            }
        }
    }
}
