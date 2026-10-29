package me.utsob.meghnaad.app

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import kotlin.math.pow

// The colours of a chart's series (web/src/lib/colors.ts, `chartColor`): a Taskwarrior colour name (`red`, `bright blue`), drawn
// in the app's own palette so it follows a light and a dark page, or a `#rrggbb` value. A series that names none takes the
// palette's blue, red, green, yellow, magenta and cyan in turn.

private val BASIC = listOf("black", "red", "green", "yellow", "blue", "magenta", "cyan", "white")

/** The names a series may use: the basic colours and their bright forms, not black and white, which would vanish on one page. */
val CHART_COLOR_NAMES: List<String> = BASIC.subList(1, 7) + BASIC.subList(1, 7).map { "bright $it" }

private val DEFAULT_SERIES = listOf("blue", "red", "green", "yellow", "magenta", "cyan", "bright blue", "bright red", "bright green")

/** The page's colour for a basic colour name (`red`, `bright blue`), or null for what is not one. */
fun namedColor(name: String, dark: Boolean): Color? {
    val n = name.trim().lowercase()
    val bright = n.startsWith("bright ")
    val i = BASIC.indexOf(if (bright) n.removePrefix("bright ") else n)
    return if (i < 0) null else paletteColor(i + if (bright) 8 else 0, dark)
}

/** `#rgb`, `#rrggbb` or `#rrggbbaa`, or null. */
fun hexColor(text: String): Color? {
    val h = text.removePrefix("#")
    if (!text.startsWith("#") || h.any { it !in "0123456789abcdefABCDEF" }) return null
    val full = when (h.length) {
        3 -> h.map { "$it$it" }.joinToString("")
        6, 8 -> h
        else -> return null
    }
    val rgb = full.take(6).toLong(16)
    val alpha = if (full.length == 8) full.takeLast(2).toInt(16) else 255
    return Color((alpha.toLong() shl 24) or rgb)
}

/** A chart colour as the app draws it: a name through the palette, a `#hex` as it is; null for neither. */
fun chartColor(color: String?, dark: Boolean): Color? =
    color?.takeIf { it.isNotBlank() }?.let { if (it.startsWith("#")) hexColor(it) else namedColor(it, dark) }

/** The colour the series (or slice) at [i] has when it names none. */
fun defaultSeriesColor(i: Int, dark: Boolean): Color = namedColor(DEFAULT_SERIES[i % DEFAULT_SERIES.size], dark) ?: Color.Gray

/** The default colour's name, for a picker to say which one it is. */
fun defaultSeriesName(i: Int): String = DEFAULT_SERIES[i % DEFAULT_SERIES.size]

private fun luminance(c: Color): Double {
    fun lin(v: Float): Double = v.toDouble().let { if (it <= 0.03928) it / 12.92 else ((it + 0.055) / 1.055).pow(2.4) }
    return 0.2126 * lin(c.red) + 0.7152 * lin(c.green) + 0.0722 * lin(c.blue)
}

/** The contrast between two colours, 1 (the same) to 21 (black on white). */
fun contrast(a: Color, b: Color): Double {
    val (x, y) = listOf(luminance(a), luminance(b)).sortedDescending()
    return (x + 0.05) / (y + 0.05)
}

/** [fg] moved toward black or white, whichever stands out more on [bg], until it shows at [min] (web/src/lib/colors.ts, `readable`). */
fun readableOn(fg: Color, bg: Color, min: Double): Color {
    if (contrast(fg, bg) >= min) return fg
    val target = if (contrast(Color.Black, bg) >= contrast(Color.White, bg)) Color.Black else Color.White
    for (step in 9 downTo 0) {
        val c = lerp(target, fg, step / 10f)
        if (contrast(c, bg) >= min) return c
    }
    return target
}

/**
 * The colour each value of a chart grouped by a UDA has in the taskrc (`color.uda.<name>.<value>`, `none` for a task with no value), by
 * position in [labels]; null for a value with no colour of its own, which keeps the chart's. A style with no text colour is its
 * background's; one too faint to show on [page] is moved toward readable (web/src/lib/colors.ts, `valueColors`).
 */
fun valueColors(field: String?, labels: List<ChartLabel>, colors: Map<String, Resolved>, dark: Boolean, page: Color): List<Color?> =
    labels.map { l ->
        val style = field?.let { colors["uda.$it.${l.key.ifEmpty { "none" }}"] } ?: return@map null
        val index = (if (style.inverse) style.bg ?: style.fg else style.fg ?: style.bg) ?: return@map null
        readableOn(paletteColor(index, dark), page, 1.8)
    }

/** The taskrc's colours (`uda.priority.H` and the like, as palette indexes) from the body of the engine's `/api/config` answer; none when it is not one. */
fun parseColors(body: String): Map<String, Resolved> =
    runCatching { engineJson.decodeFromString(ConfigResponse.serializer(), body).colors }.getOrDefault(emptyMap())
