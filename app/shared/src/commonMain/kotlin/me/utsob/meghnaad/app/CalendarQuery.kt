package me.utsob.meghnaad.app

// The Calendar page keeps its months and its task filter in one box, as `task calendar` takes them on one line: `due`, `y`, a
// year, a month and year, or a month name, plus filter words (web/src/lib/calendarQuery.ts).

private val MONTHS = listOf(
    "january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december",
)

/** Whether a word is one `calendar` itself reads (the same test the engine applies). */
fun isCalendarWord(word: String): Boolean {
    val w = word.lowercase()
    return (w.length >= 2 && "due".startsWith(w)) || w == "y" || Regex("^\\d+$").matches(w) || (w.length >= 2 && MONTHS.any { it.startsWith(w) })
}

/** The filter words of a query, without its months. */
fun calendarFilterWords(query: String): List<String> = splitWords(query).filter { !isCalendarWord(it) }

/** The same filter with other months asked for. */
fun withMonths(query: String, months: List<String>): String = joinWords(calendarFilterWords(query) + months)

data class YearMonth(val year: Int, val month: Int)

/** [first] moved by [delta] months. */
fun shiftMonth(first: YearMonth, delta: Int): YearMonth {
    val index = first.year * 12 + (first.month - 1) + delta
    return YearMonth(index.floorDiv(12), index.mod(12) + 1)
}

/**
 * The words that show the page before or after the one on screen. A year (12 months) steps by a year and keeps showing a whole
 * year; otherwise it steps by as many months as are shown.
 */
fun stepWords(first: YearMonth, shown: Int, direction: Int): List<String> {
    val next = shiftMonth(first, direction * shown)
    val words = listOf(next.month.toString(), next.year.toString())
    return if (shown == 12) words + "y" else words
}
