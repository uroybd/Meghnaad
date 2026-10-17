package me.utsob.meghnaad.app

import androidx.compose.ui.graphics.Color

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
