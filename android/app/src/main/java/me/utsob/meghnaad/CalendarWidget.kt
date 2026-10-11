package me.utsob.meghnaad

import android.content.Context
import java.util.Calendar

/** A month on the home screen, with the days that have something due marked, and buttons to look at other months. */
class CalendarWidget : ImageWidget() {
    override fun render(context: Context, id: Int, width: Int, height: Int): Rendered {
        val config = WidgetPrefs.calendar(context, id)
        val target = Calendar.getInstance().apply { add(Calendar.MONTH, WidgetPrefs.monthOffset(context, id)) }
        val year = target.get(Calendar.YEAR)
        val month = target.get(Calendar.MONTH) + 1
        val result = Tasks.calendar(context, config, year, month)
            ?: return Rendered("Calendar", message = context.getString(R.string.widget_not_ready), months = true)
        val shown = result.getJSONArray("months").getJSONObject(0)
        val names = result.getJSONArray("weekdays")
        val weekdays = (0 until names.length()).map { names.getString(it) }
        return Rendered(
            "${shown.getString("name")} ${shown.getInt("year")}",
            CalendarRenderer.draw(shown, weekdays, width, height, Palette(context)),
            CalendarRenderer.caption(shown),
            months = true,
        )
    }
}
