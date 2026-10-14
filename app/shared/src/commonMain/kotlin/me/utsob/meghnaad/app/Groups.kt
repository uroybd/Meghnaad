package me.utsob.meghnaad.app

// Group headers for a report whose sort has a `/` (`outcome+/`, `project+/`): Taskwarrior leaves a blank line where the group
// changes; the app draws a header above each group saying what its tasks have in common (web/src/lib/groups.ts).

private const val NONE = "(none)"

private fun capitalise(s: String) = s.replaceFirstChar { it.uppercase() }

/** The column to read a sort key's value with: the report's own when it shows it, else a plain one. */
private fun columnFor(result: ReportResult, name: String, ctx: Ctx): Column {
    result.columns.firstOrNull { it.name == name }?.let { return it }
    val uda = ctx.udas[name]
    return Column(spec = name, name = name, format = null, kind = "string", label = if (uda != null) udaLabel(uda) else capitalise(name))
}

/**
 * One entry per row: the text of the header that goes above it, or null when it continues the group above. Empty when the
 * report's sort has no `/`. A header reads `Outcome: fail`, and `Outcome: fail · Proj: Y` when several columns break it.
 */
fun groupHeads(result: ReportResult, ctx: Ctx): List<String?> {
    val keys = parseSort(result.sort).filter { it.brk }
    if (keys.isEmpty()) return emptyList()
    val cols = keys.map { columnFor(result, it.column, ctx) }
    fun head(row: Row) = cols.joinToString(" · ") { c -> "${c.label}: ${cell(c, row, ctx).text.trim().ifEmpty { NONE }}" }
    return result.rows.mapIndexed { i, row -> if (i == 0 || result.breaks.getOrNull(i) == true) head(row) else null }
}
