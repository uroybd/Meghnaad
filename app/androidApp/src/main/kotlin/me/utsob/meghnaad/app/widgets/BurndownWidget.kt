package me.utsob.meghnaad.app.widgets

import me.utsob.meghnaad.app.R
import android.content.Context

/** A burndown chart on the home screen. */
class BurndownWidget : ImageWidget() {
    override fun render(context: Context, id: Int, width: Int, height: Int): Rendered {
        val config = WidgetPrefs.burndown(context, id)
        val title = "Burndown · ${config.period}"
        val result = Tasks.burndown(context, config)
            ?: return Rendered(title, message = context.getString(R.string.widget_not_ready))
        if (result.getJSONArray("bars").length() == 0) return Rendered(title)
        val look = WidgetPrefs.look(context, id)
        val caption = BurndownRenderer.caption(result)
        // A look that follows the phone is drawn in both themes, and the home screen shows the one the phone is in.
        if (look.theme == "auto") {
            val light = BurndownRenderer.draw(result, width, height, Palette(context, look.copy(theme = "light")))
            val dark = BurndownRenderer.draw(result, width, height, Palette(context, look.copy(theme = "dark")))
            return Rendered(title, light, dark, caption)
        }
        return Rendered(title, BurndownRenderer.draw(result, width, height, Palette(context, look)), caption = caption)
    }
}
