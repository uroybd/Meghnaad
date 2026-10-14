package me.utsob.meghnaad.app

// Sorting by a column. A sort is a Taskwarrior sort spec (`due+,priority-,project+/`), applied through the command line as
// `rc.report.<name>.sort:<spec>`, so the console and the list always agree: web/src/lib/sortSpec.ts.

data class SortKey(val column: String, val desc: Boolean = false, /** A trailing `/`: a break when the value changes. */ val brk: Boolean = false)

/** Columns a header (or a sort picker) can sort by; UDAs are added by the caller. */
val SORTABLE = setOf(
    "id", "uuid", "status", "description", "project", "priority", "tags", "depends", "entry", "start", "end", "due", "wait",
    "scheduled", "until", "modified", "urgency", "recur", "parent",
)

private const val MAX_KEYS = 4

/** `due.relative` sorts by `due`. */
fun baseColumn(spec: String): String = spec.substringBefore('.')

fun parseSort(spec: String?): List<SortKey> {
    val s = (spec ?: "").trim()
    if (s.isEmpty() || s == "none" || s == "random") return emptyList()
    val key = Regex("^(.+?)([+-])(/?)$")
    return s.split(',').mapNotNull { part ->
        key.find(part.trim())?.let { m -> SortKey(baseColumn(m.groupValues[1]), m.groupValues[2] == "-", m.groupValues[3] == "/") }
    }
}

fun serializeSort(keys: List<SortKey>): String =
    keys.joinToString(",") { "${it.column}${if (it.desc) "-" else "+"}${if (it.brk) "/" else ""}" }

class ClickResult(val keys: List<SortKey>, /** The third click on a column: go back to the report's own sort. */ val reset: Boolean)

/**
 * What choosing a column does:
 *  - a new column becomes the primary key (ascending); earlier keys stay as tie-breakers;
 *  - the primary column again: ascending, then descending, then back to the report's default;
 *  - [shift]: add the column as a further tie-breaker, or flip its direction if present.
 */
fun clickSort(current: List<SortKey>, column: String, shift: Boolean = false): ClickResult {
    val at = current.indexOfFirst { it.column == column }
    if (shift) {
        if (at >= 0) return ClickResult(current.mapIndexed { i, k -> if (i == at) k.copy(desc = !k.desc) else k }, false)
        return ClickResult((current + SortKey(column)).take(MAX_KEYS), false)
    }
    if (at == 0) {
        if (!current[0].desc) return ClickResult(listOf(current[0].copy(desc = true)) + current.drop(1), false)
        return ClickResult(emptyList(), true)
    }
    return ClickResult((listOf(SortKey(column)) + current.filter { it.column != column }).take(MAX_KEYS), false)
}

private fun tokenFor(report: String) = Regex("^rc\\.report\\.${Regex.escape(report)}\\.sort[:=]")

/** The sort override for [report] in a filter string, if any. */
fun sortOverride(filter: String, report: String): String? {
    val re = tokenFor(report)
    val w = splitWords(filter).firstOrNull { re.containsMatchIn(it) } ?: return null
    return w.substring(w.indexOfFirst { it == ':' || it == '=' } + 1)
}

/**
 * Set (or with null, remove) the sort override in a filter string. A spec equal to the report's default needs no
 * override, so it is dropped to keep the command short.
 */
fun withSortOverride(filter: String, report: String, spec: String?, defaultSpec: String?): String {
    val re = tokenFor(report)
    val words = splitWords(filter).filter { !re.containsMatchIn(it) }.toMutableList()
    if (!spec.isNullOrEmpty() && spec != (defaultSpec ?: "")) words += "rc.report.$report.sort:$spec"
    return joinWords(words)
}

class SortState(val desc: Boolean, val rank: Int?, val group: Boolean)

/** For a header: its place in the active sort (and whether it groups the list), or null. */
fun sortState(keys: List<SortKey>, column: String): SortState? {
    val i = keys.indexOfFirst { it.column == baseColumn(column) }
    if (i < 0) return null
    return SortState(keys[i].desc, if (keys.size > 1) i + 1 else null, keys[i].brk)
}

/**
 * Group by [column], or stop grouping by it: its sort key gains or loses the trailing `/`. A column the sort does not use
 * yet becomes the first key, ascending, since a group is only whole when the grouping column is what the list is sorted by
 * first.
 */
fun toggleGroup(current: List<SortKey>, column: String, defaults: List<SortKey> = emptyList()): List<SortKey> {
    val base = baseColumn(column)
    val key = current.firstOrNull { it.column == base }
    if (key != null) {
        // Ending a grouping that put the column into the sort takes it out again; one the report's own sort has keeps its
        // place and loses only the `/`.
        if (key.brk && defaults.none { it.column == base }) return current.filter { it !== key }
        return current.map { if (it === key) it.copy(brk = !it.brk) else it }
    }
    return (listOf(SortKey(base, brk = true)) + current).take(MAX_KEYS)
}

/** The same sort with no grouping at all. */
fun clearGroups(current: List<SortKey>): List<SortKey> = current.map { it.copy(brk = false) }

/** The column the list is grouped by first, if any. */
fun groupColumn(keys: List<SortKey>): String? = keys.firstOrNull { it.brk }?.column
