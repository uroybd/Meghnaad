package me.utsob.meghnaad

import android.content.Context
import android.content.Intent
import android.util.Log
import android.widget.RemoteViews
import android.widget.RemoteViewsService

/** Feeds the widget's list from the engine. */
class TasksWidgetService : RemoteViewsService() {
    override fun onGetViewFactory(intent: Intent): RemoteViewsFactory =
        Factory(applicationContext, intent.data?.lastPathSegment?.toIntOrNull() ?: 0)

    private class Factory(private val context: Context, private val widgetId: Int) : RemoteViewsFactory {
        private var tasks: List<WidgetTask> = emptyList()

        override fun onCreate() {}

        /** Called off the main thread whenever the widget asks to be redrawn: read the engine again. */
        override fun onDataSetChanged() {
            tasks = try {
                Tasks.report(context, WidgetPrefs.list(context, widgetId))
            } catch (e: Exception) {
                Log.e("TasksWidget", "could not read the tasks", e)
                emptyList()
            }
            Log.d("TasksWidget", "${tasks.size} tasks")
        }

        override fun onDestroy() {}

        override fun getCount() = tasks.size

        override fun getViewAt(position: Int): RemoteViews {
            val task = tasks[position]
            val row = RemoteViews(context.packageName, R.layout.widget_task_item)
            val palette = Palette(context, WidgetPrefs.look(context, widgetId))
            row.setTextViewText(R.id.description, task.description)
            row.setTextViewText(R.id.detail, task.detail)
            palette.setTextColor(row, R.id.description, Palette.TEXT)
            palette.setTextColor(row, R.id.detail, if (task.overdue) Palette.OVERDUE else Palette.DIM)
            palette.setTint(row, R.id.done, Palette.DIM)
            row.setViewVisibility(R.id.detail, if (task.detail.isEmpty()) android.view.View.GONE else android.view.View.VISIBLE)
            row.setOnClickFillInIntent(R.id.done, fill("done", task.uuid))
            row.setOnClickFillInIntent(R.id.text, fill("open", task.uuid))
            return row
        }

        private fun fill(kind: String, uuid: String) = Intent().putExtra("kind", kind).putExtra("uuid", uuid)

        override fun getLoadingView(): RemoteViews? = null

        override fun getViewTypeCount() = 1

        override fun getItemId(position: Int) = tasks[position].uuid.hashCode().toLong()

        override fun hasStableIds() = true
    }

}
