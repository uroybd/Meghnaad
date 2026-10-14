package me.utsob.meghnaad.app

// Which taskrc `dateformat*` setting applies where (web/src/lib/dateformat.ts). Writing a date in a pattern is the engine's
// (`formatPattern` asks it), so dates show the way the user's taskrc says: Taskwarrior's letters are tokens, everything else
// is copied as it is.

class DateFmt(
    val pattern: String,
    /** 0 = weeks start on Sunday, 1 = Monday (ISO weeks), from the `weekstart` setting. */
    val weekstart: Int,
)

/** Format [epoch] (seconds) in the zone [tz] seconds east of UTC: the engine's own `dateformat` writer. */
fun formatPattern(epoch: Long, fmt: DateFmt, tz: Int): String = rustHelpers().formatDate(fmt.pattern, epoch, tz, fmt.weekstart == 1)

/** Where a date is shown, which decides which `dateformat*` setting applies. */
enum class FormatKind {
    /** Table cells: report.<name>.dateformat, dateformat.report, dateformat. */
    Report,

    /** The detail view: dateformat.info, dateformat. */
    Info,

    /** Notes under a report's description: dateformat.annotation, dateformat. */
    Annotation,

    /** Notes in the detail view: dateformat.annotation, else the detail view's format. */
    InfoNote,
}

fun weekStartOf(settings: Map<String, String>?): Int =
    if (Regex("^mon", RegexOption.IGNORE_CASE).containsMatchIn(settings?.get("weekstart") ?: "")) 1 else 0

/** The pattern for a place, following Taskwarrior's fallbacks; null = none configured. */
fun formatFor(kind: FormatKind, settings: Map<String, String>?, reportDateformat: String? = null): DateFmt? {
    val s = settings ?: emptyMap()

    fun first(vararg v: String?) = v.firstOrNull { !it.isNullOrEmpty() } ?: ""
    val infoFmt = first(s["dateformat.info"], s["dateformat"])
    val pattern = when (kind) {
        FormatKind.Report -> first(reportDateformat, s["dateformat.report"], s["dateformat"])
        FormatKind.Info -> infoFmt
        FormatKind.Annotation -> first(s["dateformat.annotation"], s["dateformat"])
        FormatKind.InfoNote -> first(s["dateformat.annotation"], infoFmt)
    }
    return if (pattern.isNotEmpty()) DateFmt(pattern, weekStartOf(settings)) else null
}
