package me.utsob.meghnaad

import android.content.Context
import android.widget.RemoteViews

/** Puts a [WidgetLook] on a widget's views: the background (colour and opacity) and the colours of its header. */
object Looks {
    fun apply(context: Context, views: RemoteViews, id: Int, extraButtons: List<Int> = emptyList()): Palette {
        val palette = Palette(context, WidgetPrefs.look(context, id))
        // The background is a white shape, tinted to the theme's colour and made as see-through as asked.
        views.setInt(R.id.bg, "setColorFilter", palette.background)
        views.setInt(R.id.bg, "setImageAlpha", palette.alpha)
        views.setTextColor(R.id.title, palette.text)
        for (button in listOf(R.id.refresh) + extraButtons) views.setInt(button, "setColorFilter", palette.text)
        return palette
    }
}
