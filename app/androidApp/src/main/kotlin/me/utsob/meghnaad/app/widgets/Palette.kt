package me.utsob.meghnaad

import android.content.Context
import android.content.res.Configuration
import android.os.Build
import android.widget.RemoteViews

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

/** A colour for each of the phone's themes: the same twice when it does not change with them. */
class Tone(val light: Int, val dark: Int) {
    constructor(both: Int) : this(both, both)

    fun pick(dark: Boolean) = if (dark) this.dark else light
}

/** The colours a widget uses, for the look it was given: the text colours follow the theme, whatever the phone's is. */
class Palette(context: Context, val look: WidgetLook = WidgetLook()) {
    val dark = when (look.theme) {
        "light" -> false
        "dark" -> true
        else -> context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES
    }
    val background = BACKGROUND.pick(dark)
    val text = TEXT.pick(dark)
    val dim = DIM.pick(dark)
    val line = (dim and 0x00FFFFFF) or 0x40000000
    val pending = 0xFFE5534B.toInt()
    val started = 0xFFD29922.toInt()
    val done = 0xFF3FB950.toInt()
    val accent = 0xFF2F81F7.toInt()
    val overdue = OVERDUE.pick(dark)
    val dueToday = 0xFFD29922.toInt()
    val due = 0xFF58A6FF.toInt()

    /** The background's opacity as ImageView's 0..255. */
    val alpha get() = look.opacity * 255 / 100

    /**
     * Whether a widget follows the phone's theme, and so has to change when the phone's switch does, though nothing here
     * runs then: a widget is only drawn when something asks for it.
     */
    val follows get() = look.theme == "auto"

    /**
     * Put a colour on one property of a view (`setTextColor`, `setColorFilter`, `setBackgroundColor`). A look that follows the
     * phone sets none that differs between the themes: nothing here runs when the phone changes theme, and a colour set from
     * here would stay the one it was drawn with (the text stayed dark on a dark widget). What follows the phone is what the
     * layouts and drawables name by a resource, which has a night variant (`values-night`) and which the home screen reads again
     * itself. A fixed look (light or dark) sets its colours here, as it does not change.
     */
    fun set(views: RemoteViews, id: Int, method: String, tone: Tone) {
        when {
            !follows -> views.setInt(id, method, tone.pick(dark))
            tone.light == tone.dark -> views.setInt(id, method, tone.light)
            else -> Unit
        }
    }

    fun setTextColor(views: RemoteViews, id: Int, tone: Tone) = set(views, id, "setTextColor", tone)

    fun setTint(views: RemoteViews, id: Int, tone: Tone) = set(views, id, "setColorFilter", tone)

    companion object {
        val BACKGROUND = Tone(0xFFFFFFFF.toInt(), 0xFF161B22.toInt())
        val TEXT = Tone(0xFF1F2328.toInt(), 0xFFE6EDF3.toInt())
        val DIM = Tone(0xFF6E7781.toInt(), 0xFF8B949E.toInt())
        val OVERDUE = Tone(0xFFCF222E.toInt(), 0xFFFF7B72.toInt())
        val WHITE = Tone(0xFFFFFFFF.toInt())
        val ACCENT = Tone(0xFF2F81F7.toInt())
        val DUE_TODAY = Tone(0xFFD29922.toInt())
        val DUE = Tone(0xFF58A6FF.toInt())
    }
}
