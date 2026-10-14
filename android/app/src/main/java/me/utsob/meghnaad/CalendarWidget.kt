package me.utsob.meghnaad

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.widget.RemoteViews
import org.json.JSONObject
import java.util.Calendar

/**
 * A month on the home screen: today circled, a dot on the days with something due, arrows to other months, and a tap on a
 * day opens that day's tasks (due or scheduled) in the app.
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
        if (!Widgets.anyPlaced(context)) SyncWorker.cancel(context)
    }

    override fun onDeleted(context: Context, ids: IntArray) = WidgetPrefs.forget(context, ids)

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
                update(context, id)
            }
        }
    }

    companion object {
        const val ACTION_SYNC = "me.utsob.meghnaad.CALENDAR_SYNC"
        const val ACTION_MONTH = "me.utsob.meghnaad.CALENDAR_MONTH"

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
                    val result = Tasks.calendar(app, WidgetPrefs.calendar(app, id), year, month)
                    val manager = AppWidgetManager.getInstance(app)
                    manager.updateAppWidget(id, views(app, id, result))
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

        private fun views(context: Context, id: Int, result: JSONObject?): RemoteViews {
            val views = RemoteViews(context.packageName, R.layout.widget_calendar)
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

            // The days are the service's: one cell each, and a tap on one comes back as this template filled in with the day.
            val adapter = Intent(context, CalendarWidgetService::class.java).apply { data = Uri.parse("meghnaad://calendar/$id") }
            views.setRemoteAdapter(R.id.grid, adapter)
            views.setPendingIntentTemplate(
                R.id.grid,
                PendingIntent.getActivity(
                    context,
                    id * 10 + 5,
                    Intent(context, WidgetActionActivity::class.java),
                    PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE,
                ),
            )
            views.setOnClickPendingIntent(R.id.previous, broadcast(context, id, 2, -1))
            views.setOnClickPendingIntent(R.id.next, broadcast(context, id, 3, 1))
            // A tap on the month's name goes back to this month.
            views.setOnClickPendingIntent(R.id.title, broadcast(context, id, 4, 0))
            views.setOnClickPendingIntent(R.id.refresh, broadcast(context, id, 1, 0, ACTION_SYNC))
            return views
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
