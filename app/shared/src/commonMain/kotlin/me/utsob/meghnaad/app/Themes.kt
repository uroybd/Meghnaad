package me.utsob.meghnaad.app

// Colour themes: sets of `color.*` lines. Taskwarrior's own are plain files (`include dark-256.theme`), which an app cannot include,
// so they travel with it (resources/themes) and are put into the taskrc text when picked. The engine only ever sees the lines that
// were saved (web/src/lib/themes.ts).

class Theme(val id: String, val label: String, val note: String? = null)

/** The app's own theme first (it is what applies when nothing is set), then Taskwarrior's. */
val THEMES = listOf(
    Theme("meghnaad", "Meghnaad", "soft colours (the default)"),
    Theme("default", "Taskwarrior default", "Taskwarrior's own default colours"),
    Theme("dark-16", "Dark, 16 colours"),
    Theme("dark-256", "Dark, 256 colours"),
    Theme("dark-blue-256", "Dark blue"),
    Theme("dark-gray-256", "Dark gray"),
    Theme("dark-gray-blue-256", "Dark gray blue"),
    Theme("dark-green-256", "Dark green"),
    Theme("dark-red-256", "Dark red"),
    Theme("dark-violets-256", "Dark violets"),
    Theme("dark-yellow-green", "Dark yellow green"),
    Theme("bubblegum-256", "Bubblegum"),
    Theme("light-16", "Light, 16 colours"),
    Theme("light-256", "Light, 256 colours"),
    Theme("solarized-dark-256", "Solarized dark"),
    Theme("solarized-light-256", "Solarized light"),
    Theme("no-color", "No colour", "every colour rule switched off"),
)

fun isTheme(id: String) = THEMES.any { it.id == id }

/** A line that sets a colour or how colours combine (not the `color=on|off` switch). */
private fun colourLine(l: String) = Regex("^\\s*(color\\.|rule\\.precedence\\.color|rule\\.color\\.merge)").containsMatchIn(l)

/** The `name=value` lines of a theme file, without its comments (a `#` starts one anywhere in a line). */
fun themeLines(file: String): List<String> = file.split('\n').map { it.substringBefore('#').trim() }.filter { it.isNotEmpty() }

private fun dropTrailingBlanks(lines: MutableList<String>) {
    while (lines.isNotEmpty() && lines.last().isBlank()) lines.removeAt(lines.lastIndex)
}

/**
 * [text] with its colour lines replaced by the theme's. The Meghnaad theme is the engine's default, so choosing it just clears
 * the colour lines. The `color=on|off` line and everything else are kept.
 */
fun applyTheme(text: String, id: String, file: String): String {
    val kept = text.split('\n').filter { !colourLine(it) }.toMutableList()
    dropTrailingBlanks(kept)
    if (id == "meghnaad") return kept.joinToString("\n") + if (kept.isNotEmpty()) "\n" else ""
    return (kept + (if (kept.isNotEmpty()) listOf("") else emptyList()) + "# theme: $id" + themeLines(file)).joinToString("\n") + "\n"
}

/** Turn colours on or off: the `color` line of the taskrc. */
fun withColour(text: String, on: Boolean): String {
    val kept = text.split('\n').filter { !Regex("^\\s*color\\s*[=\\s]").containsMatchIn(it) }.toMutableList()
    dropTrailingBlanks(kept)
    if (!on) kept += "color=off"
    return kept.joinToString("\n") + if (kept.isNotEmpty()) "\n" else ""
}

/** Whether the text turns colour off. */
fun colourOff(text: String): Boolean =
    text.split('\n').any { Regex("^\\s*color\\s*[=\\s]\\s*(off|0|no|false|n)\\s*$", RegexOption.IGNORE_CASE).containsMatchIn(it.substringBefore('#')) }

/** `include dark-256.theme` (a path in front is fine) names one of the bundled themes. */
fun includedTheme(line: String): String? {
    val m = Regex("^\\s*include\\s+(?:\\S*/)?([\\w.-]+?)\\.theme\\s*$").find(line) ?: return null
    return m.groupValues[1].takeIf { isTheme(it) }
}

/** Replace each `include <bundled>.theme` line by that theme's lines. Other lines, and other includes, stay. */
suspend fun expandIncludes(text: String, fetchTheme: suspend (String) -> String): String {
    val out = mutableListOf<String>()
    for (line in text.split('\n')) {
        val id = includedTheme(line)
        if (id == null) {
            out += line
            continue
        }
        out += "# theme: $id"
        out += themeLines(fetchTheme(id))
    }
    return out.joinToString("\n")
}

/** A bundled theme's file. */
fun loadTheme(id: String): String =
    readResource("/themes/$id.theme") ?: throw EngineError("The $id theme could not be loaded.")
