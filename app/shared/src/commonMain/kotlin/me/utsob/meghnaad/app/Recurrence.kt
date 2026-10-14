package me.utsob.meghnaad.app

// Recurrence periods as Taskwarrior writes them (`recur:weekly`, `recur:3d`, ISO `P2W`). The engine validates; this only
// offers sensible choices and describes what is stored (web/src/lib/recurrence.ts).

class Preset(val value: String, val label: String)

val PRESETS = listOf(
    Preset("daily", "Every day"),
    Preset("weekdays", "Every weekday"),
    Preset("weekly", "Every week"),
    Preset("biweekly", "Every 2 weeks"),
    Preset("monthly", "Every month"),
    Preset("quarterly", "Every 3 months"),
    Preset("yearly", "Every year"),
)

/** Names Taskwarrior accepts that are not in the presets, for describing and completing. */
private val NAMED = mapOf(
    "day" to "every day", "daily" to "every day", "weekdays" to "every weekday", "week" to "every week",
    "weekly" to "every week", "biweekly" to "every 2 weeks", "fortnight" to "every 2 weeks", "month" to "every month",
    "monthly" to "every month", "bimonthly" to "every 2 months", "quarter" to "every 3 months",
    "quarterly" to "every 3 months", "semiannual" to "every 6 months", "year" to "every year", "yearly" to "every year",
    "annual" to "every year", "biannual" to "every 2 years", "biyearly" to "every 2 years",
)

private val UNITS = listOf(
    Regex("^(d|day|days)$") to "day",
    Regex("^(w|wk|wks|week|weeks)$") to "week",
    Regex("^(mo|mth|mths|month|months)$") to "month",
    Regex("^(q|qtr|qtrs|quarter|quarters)$") to "quarter",
    Regex("^(y|yr|yrs|year|years)$") to "year",
)

private val ISO = Regex("^P(?:(\\d+)Y)?(?:(\\d+)M)?(?:(\\d+)W)?(?:(\\d+)D)?$", RegexOption.IGNORE_CASE)

/** "every 3 days" for `3d`; unknown spellings come back as typed. */
fun describeRecur(period: String?): String {
    val p = (period ?: "").trim()
    if (p.isEmpty()) return ""
    val lower = p.lowercase()
    NAMED[lower]?.let { return it }
    Regex("^(\\d+)\\s*([a-z]+)$").find(lower)?.let { m ->
        val n = m.groupValues[1].toInt()
        val unit = UNITS.firstOrNull { it.first.matches(m.groupValues[2]) }?.second
        if (unit != null) return if (n == 1) "every $unit" else "every $n ${unit}s"
    }
    val iso = ISO.find(p)
    if (iso != null && p.length > 1) {
        val names = listOf("year", "month", "week", "day")
        val parts = names.mapIndexedNotNull { i, name ->
            val n = iso.groupValues[i + 1]
            if (n.isEmpty()) null else "$n $name${if (n == "1") "" else "s"}"
        }
        if (parts.isNotEmpty()) return "every ${parts.joinToString(" ")}"
    }
    return p
}

/** The preset a stored period corresponds to exactly, or empty when it needs the custom field. */
fun presetFor(period: String?): String = (period ?: "").trim().let { p -> if (PRESETS.any { it.value == p }) p else "" }

/** `recur:` completion candidates, with the plain-English meaning as a hint. */
class RecurWord(val value: String, val hint: String)

val RECUR_WORDS = PRESETS.map { RecurWord(it.value, it.label.lowercase()) } +
    listOf(RecurWord("3d", "every 3 days"), RecurWord("2w", "every 2 weeks"), RecurWord("2m", "every 2 months"))
