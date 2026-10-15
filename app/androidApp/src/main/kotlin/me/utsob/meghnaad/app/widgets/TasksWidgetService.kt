package me.utsob.meghnaad

import me.utsob.meghnaad.app.R
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
            nameAfterTheReport()
        }

        /**
         * The widget is called what its report is described as (unless it was given a title). That is the engine's to say, which
         * is asked here, off the main thread; when it differs from what was kept, it is kept and the title is drawn again.
         */
        private fun nameAfterTheReport() {
            val config = WidgetPrefs.list(context, widgetId)
            val said = try {
                Tasks.reportDescription(context, config.report)
            } catch (e: Exception) {
                null
            }
            if (said != null && said != config.description) {
                WidgetPrefs.saveList(context, widgetId, config.copy(description = said))
                TasksWidget.update(context, widgetId)
            }
        }

        override fun onDestroy() {}

        override fun getCount() = tasks.size

        override fun getViewAt(position: Int): RemoteViews {
            val task = tasks[position]
            val row = RemoteViews(context.packageName, R.layout.widget_task_item)
            val palette = Palette(context, WidgetPrefs.look(context, widgetId))
            row.setTextViewText(R.id.description, task.description)
            // An overdue task's line is the red one, which takes its colour from a resource (see Palette.set).
            val line = if (task.overdue) R.id.detail_overdue else R.id.detail
            row.setTextViewText(line, task.detail)
            palette.setTextColor(row, R.id.description, Palette.TEXT)
            palette.setTextColor(row, line, if (task.overdue) Palette.OVERDUE else Palette.DIM)
            palette.setTint(row, R.id.done, Palette.DIM)
            row.setViewVisibility(line, if (task.detail.isEmpty()) android.view.View.GONE else android.view.View.VISIBLE)
            if (task.overdue) row.setViewVisibility(R.id.detail, android.view.View.GONE)
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
