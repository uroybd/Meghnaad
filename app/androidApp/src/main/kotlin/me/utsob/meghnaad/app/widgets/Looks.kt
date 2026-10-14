package me.utsob.meghnaad

import me.utsob.meghnaad.app.R
import android.content.Context
import android.widget.RemoteViews

/** Puts a [WidgetLook] on a widget's views: the background (colour and opacity) and the colours of its header. */
object Looks {
    fun apply(context: Context, views: RemoteViews, id: Int, extraButtons: List<Int> = emptyList()): Palette {
        val palette = Palette(context, WidgetPrefs.look(context, id))
        // The background is a white shape, tinted to the theme's colour and made as see-through as asked.
        palette.setTint(views, R.id.bg, Palette.BACKGROUND)
        views.setInt(R.id.bg, "setImageAlpha", palette.alpha)
        palette.setTextColor(views, R.id.title, Palette.TEXT)
        for (button in listOf(R.id.refresh) + extraButtons) palette.setTint(views, button, Palette.TEXT)
        return palette
    }
}
