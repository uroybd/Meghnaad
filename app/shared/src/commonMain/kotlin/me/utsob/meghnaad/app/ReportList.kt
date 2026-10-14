package me.utsob.meghnaad.app

// Reports whose names start with `_` are Taskwarrior's hidden helpers (`_reviewed`, `_unique`): they are for scripts and other
// commands, so the lists a person picks from leave them out until asked, as `task reports` does. Typing one still works
// (web/src/lib/reportList.ts).

fun isHiddenReport(name: String): Boolean = name.startsWith("_")

/**
 * The reports to offer: all of them when [showAll], otherwise those not hidden, plus the one in use (a hidden report that was
 * opened by name must not vanish from under the person looking at it).
 */
fun offeredReports(reports: List<ReportMeta>, showAll: Boolean, current: String): List<ReportMeta> =
    if (showAll) reports else reports.filter { !isHiddenReport(it.name) || it.name == current }
