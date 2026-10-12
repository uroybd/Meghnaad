package me.utsob.meghnaad

import android.appwidget.AppWidgetManager
import android.content.ComponentName
import android.content.Context

/** All the widgets there can be, so that a change redraws every one of them. */
object Widgets {
    private val PROVIDERS = listOf(TasksWidget::class.java, BurndownWidget::class.java, CalendarWidget::class.java)

    /** Redraw them all from the engine. Never call this while holding the engine: the lists read it to draw. */
    fun refreshAll(context: Context) {
        TasksWidget.refresh(context)
        ImageWidget.refreshAll(context)
        CalendarWidget.refresh(context)
    }

    /** Whether any widget of ours is on a home screen: the background sync is only needed while there is. */
    fun anyPlaced(context: Context): Boolean {
        val manager = AppWidgetManager.getInstance(context)
        return PROVIDERS.any { manager.getAppWidgetIds(ComponentName(context, it)).isNotEmpty() }
    }

    /** Which kind of widget an id is, from the provider it was placed from. */
    fun kindOf(context: Context, id: Int): String? {
        val provider = AppWidgetManager.getInstance(context).getAppWidgetInfo(id)?.provider?.className ?: return null
        return when (provider) {
            TasksWidget::class.java.name -> "list"
            BurndownWidget::class.java.name -> "burndown"
            CalendarWidget::class.java.name -> "calendar"
            else -> null
        }
    }

    /** Redraw one widget, whatever it is, after its settings changed. */
    fun update(context: Context, id: Int) {
        when (kindOf(context, id)) {
            "list" -> TasksWidget.update(context, id)
            "burndown" -> ImageWidget.refresh(context, intArrayOf(id))
            "calendar" -> CalendarWidget.update(context, id)
        }
    }
}
