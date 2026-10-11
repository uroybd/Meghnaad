package me.utsob.meghnaad

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.widget.RemoteViews

/** The home-screen widget: the next tasks, a tick to finish one, and a button to add one. */
class TasksWidget : AppWidgetProvider() {
    override fun onUpdate(context: Context, manager: AppWidgetManager, ids: IntArray) {
        for (id in ids) manager.updateAppWidget(id, views(context, id))
        manager.notifyAppWidgetViewDataChanged(ids, R.id.list)
    }

    override fun onEnabled(context: Context) {
        SyncWorker.schedule(context)
        SyncWorker.runNow(context)
    }

    override fun onDisabled(context: Context) = SyncWorker.cancel(context)

    override fun onReceive(context: Context, intent: Intent) {
        super.onReceive(context, intent)
        if (intent.action == ACTION_SYNC) {
            SyncWorker.runNow(context)
            refresh(context)
        }
    }

    companion object {
        const val ACTION_SYNC = "me.utsob.meghnaad.SYNC"

        private fun views(context: Context, id: Int): RemoteViews {
            val views = RemoteViews(context.packageName, R.layout.widget_tasks)
            val adapter = Intent(context, TasksWidgetService::class.java).apply {
                putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, id)
                data = Uri.parse(toUri(Intent.URI_INTENT_SCHEME))
            }
            views.setRemoteAdapter(R.id.list, adapter)
            views.setEmptyView(R.id.list, R.id.empty)
            // A tap on a row, or on its tick, comes back as this template filled in with what was tapped.
            views.setPendingIntentTemplate(R.id.list, activity(context, WidgetActionActivity::class.java, 1, mutable = true))
            views.setOnClickPendingIntent(R.id.add, activity(context, QuickAddActivity::class.java, 2))
            views.setOnClickPendingIntent(R.id.title, activity(context, MainActivity::class.java, 3))
            val sync = Intent(context, TasksWidget::class.java).setAction(ACTION_SYNC)
            views.setOnClickPendingIntent(
                R.id.refresh,
                PendingIntent.getBroadcast(context, 4, sync, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT),
            )
            return views
        }

        private fun activity(context: Context, target: Class<*>, code: Int, mutable: Boolean = false): PendingIntent {
            val flags = PendingIntent.FLAG_UPDATE_CURRENT or if (mutable) PendingIntent.FLAG_MUTABLE else PendingIntent.FLAG_IMMUTABLE
            return PendingIntent.getActivity(context, code, Intent(context, target), flags)
        }

        /** Redraw the list from the engine, wherever the widget is placed. */
        fun refresh(context: Context) {
            val manager = AppWidgetManager.getInstance(context)
            val ids = manager.getAppWidgetIds(ComponentName(context, TasksWidget::class.java))
            if (ids.isNotEmpty()) manager.notifyAppWidgetViewDataChanged(ids, R.id.list)
        }
    }
}
