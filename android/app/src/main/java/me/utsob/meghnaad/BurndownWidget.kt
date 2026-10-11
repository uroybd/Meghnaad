package me.utsob.meghnaad

import android.content.Context

/** A burndown chart on the home screen. */
class BurndownWidget : ImageWidget() {
    override fun render(context: Context, id: Int, width: Int, height: Int): Rendered {
        val config = WidgetPrefs.burndown(context, id)
        val title = "Burndown · ${config.period}"
        val result = Tasks.burndown(context, config)
            ?: return Rendered(title, message = context.getString(R.string.widget_not_ready))
        if (result.getJSONArray("bars").length() == 0) return Rendered(title)
        return Rendered(title, BurndownRenderer.draw(result, width, height, Palette(context)), BurndownRenderer.caption(result))
    }
}
