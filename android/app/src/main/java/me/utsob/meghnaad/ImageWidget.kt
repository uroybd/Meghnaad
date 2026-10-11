package me.utsob.meghnaad

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.view.View
import android.widget.RemoteViews

/** What a chart-like widget draws: a picture with a title and a line under it, or a word on why there is none. */
class Rendered(
    val title: String,
    val bitmap: Bitmap? = null,
    val caption: String = "",
    val message: String? = null,
    /** Show the buttons that move to the month before and after. */
    val months: Boolean = false,
)

/**
 * A widget that is a picture the app draws (RemoteViews cannot draw a chart), with the buttons of its header. The pictures
 * are made off the main thread, from the engine, at the size the widget has on the home screen.
 */
abstract class ImageWidget : AppWidgetProvider() {
    abstract fun render(context: Context, id: Int, width: Int, height: Int): Rendered

    override fun onUpdate(context: Context, manager: AppWidgetManager, ids: IntArray) = refresh(context, ids, goAsync())

    override fun onAppWidgetOptionsChanged(context: Context, manager: AppWidgetManager, id: Int, options: android.os.Bundle) =
        refresh(context, intArrayOf(id))

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
                refreshAll(context)
            }
            ACTION_MONTH -> {
                WidgetPrefs.setMonthOffset(context, id, if (intent.getIntExtra("delta", 0) == 0) 0 else WidgetPrefs.monthOffset(context, id) + intent.getIntExtra("delta", 0))
                refresh(context, intArrayOf(id))
            }
        }
    }

    companion object {
        const val ACTION_SYNC = "me.utsob.meghnaad.IMAGE_SYNC"
        const val ACTION_MONTH = "me.utsob.meghnaad.IMAGE_MONTH"

        /** Draw every chart-like widget again. */
        fun refreshAll(context: Context) {
            val manager = AppWidgetManager.getInstance(context)
            val ids = listOf(BurndownWidget::class.java, CalendarWidget::class.java)
                .flatMap { manager.getAppWidgetIds(ComponentName(context, it)).toList() }
            if (ids.isNotEmpty()) refresh(context, ids.toIntArray())
        }

        /** Draw these widgets again, on a thread of their own. */
        fun refresh(context: Context, ids: IntArray, pending: android.content.BroadcastReceiver.PendingResult? = null) {
            val app = context.applicationContext
            Thread {
                try {
                    for (id in ids) {
                        try {
                            draw(app, id)
                        } catch (e: Exception) {
                            android.util.Log.e("ImageWidget", "could not draw widget $id", e)
                        }
                    }
                } finally {
                    pending?.finish()
                }
            }.start()
        }

        private fun draw(context: Context, id: Int) {
            val provider: ImageWidget = when (Widgets.kindOf(context, id)) {
                "burndown" -> BurndownWidget()
                "calendar" -> CalendarWidget()
                else -> return
            }
            val manager = AppWidgetManager.getInstance(context)
            val (width, height) = chartSize(context, manager, id)
            val rendered = provider.render(context, id, width, height)
            manager.updateAppWidget(id, views(context, id, provider.javaClass, rendered))
        }

        /** Room for the picture, in pixels: the widget's size less its header and its line of text. */
        private fun chartSize(context: Context, manager: AppWidgetManager, id: Int): Pair<Int, Int> {
            val options = manager.getAppWidgetOptions(id)
            val density = context.resources.displayMetrics.density
            val wide = options.getInt(AppWidgetManager.OPTION_APPWIDGET_MIN_WIDTH, 250)
            val high = options.getInt(AppWidgetManager.OPTION_APPWIDGET_MAX_HEIGHT, 250)
            var w = ((wide - 16) * density).toInt().coerceAtLeast(120)
            var h = ((high - 16 - 40 - 20) * density).toInt().coerceAtLeast(100)
            // A widget's pictures travel to the home screen in one parcel, which has a size limit.
            val limit = 420_000.0
            if (w * h > limit) {
                val scale = Math.sqrt(limit / (w * h))
                w = (w * scale).toInt()
                h = (h * scale).toInt()
            }
            return w to h
        }

        private fun views(context: Context, id: Int, provider: Class<*>, r: Rendered): RemoteViews {
            val views = RemoteViews(context.packageName, R.layout.widget_image)
            views.setTextViewText(R.id.title, r.title)
            views.setTextViewText(R.id.caption, r.caption)
            if (r.bitmap != null) {
                views.setImageViewBitmap(R.id.chart, r.bitmap)
                views.setViewVisibility(R.id.chart, View.VISIBLE)
                views.setViewVisibility(R.id.message, View.GONE)
            } else {
                views.setViewVisibility(R.id.chart, View.GONE)
                views.setTextViewText(R.id.message, r.message ?: context.getString(R.string.widget_nothing_to_show))
                views.setViewVisibility(R.id.message, View.VISIBLE)
            }
            val nav = if (r.months) View.VISIBLE else View.GONE
            views.setViewVisibility(R.id.previous, nav)
            views.setViewVisibility(R.id.next, nav)
            val open = PendingIntent.getActivity(
                context,
                id * 10,
                Intent(context, MainActivity::class.java),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
            views.setOnClickPendingIntent(R.id.chart, open)
            views.setOnClickPendingIntent(R.id.title, open)
            views.setOnClickPendingIntent(R.id.refresh, broadcast(context, id, provider, 1, ACTION_SYNC))
            views.setOnClickPendingIntent(R.id.previous, broadcast(context, id, provider, 2, ACTION_MONTH, -1))
            views.setOnClickPendingIntent(R.id.next, broadcast(context, id, provider, 3, ACTION_MONTH, 1))
            // A tap on the month's name goes back to this month.
            if (r.months) views.setOnClickPendingIntent(R.id.title, broadcast(context, id, provider, 4, ACTION_MONTH, 0))
            return views
        }

        private fun broadcast(context: Context, id: Int, provider: Class<*>, n: Int, action: String, delta: Int = 0): PendingIntent {
            val intent = Intent(context, provider)
                .setAction(action)
                .putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, id)
                .putExtra("delta", delta)
            return PendingIntent.getBroadcast(context, id * 10 + n, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        }
    }
}
