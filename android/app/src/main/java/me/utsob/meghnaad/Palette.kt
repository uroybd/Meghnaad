package me.utsob.meghnaad

import android.content.Context
import android.content.res.Configuration

/** How a widget looks apart from what it shows: light, dark, or following the phone, and how see-through its background is. */
data class WidgetLook(val theme: String = "auto", val opacity: Int = 95) {
    fun toJson(): org.json.JSONObject = org.json.JSONObject().put("theme", theme).put("opacity", opacity)

    companion object {
        val THEMES = listOf("auto", "light", "dark")

        fun fromJson(j: org.json.JSONObject) = WidgetLook(
            j.optString("theme", "auto").takeIf { it in THEMES } ?: "auto",
            j.optInt("opacity", 95).coerceIn(0, 100),
        )
    }
}

/** The colours a widget uses, for the look it was given: the text colours follow the theme, whatever the phone's is. */
class Palette(context: Context, val look: WidgetLook = WidgetLook()) {
    val dark = when (look.theme) {
        "light" -> false
        "dark" -> true
        else -> context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES
    }
    val background = if (dark) 0xFF161B22.toInt() else 0xFFFFFFFF.toInt()
    val text = if (dark) 0xFFE6EDF3.toInt() else 0xFF1F2328.toInt()
    val dim = if (dark) 0xFF8B949E.toInt() else 0xFF6E7781.toInt()
    val line = (dim and 0x00FFFFFF) or 0x40000000
    val pending = 0xFFE5534B.toInt()
    val started = 0xFFD29922.toInt()
    val done = 0xFF3FB950.toInt()
    val accent = 0xFF2F81F7.toInt()
    val overdue = if (dark) 0xFFFF7B72.toInt() else 0xFFCF222E.toInt()
    val dueToday = 0xFFD29922.toInt()
    val due = 0xFF58A6FF.toInt()

    /** The background's opacity as ImageView's 0..255. */
    val alpha get() = look.opacity * 255 / 100
}
