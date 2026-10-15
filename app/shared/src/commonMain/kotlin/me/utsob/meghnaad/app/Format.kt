package me.utsob.meghnaad.app

import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

// Report cells: what a column spec like `due.relative` or `description.truncated` shows for a task is the engine's
// (`tc_core::cells`, through the helpers); what is here is the zone the phone is in and the styling hints a row has.

/** A note under a description: when it was made, its text, and its Markdown when it uses any. */
@Serializable
data class NoteCell(val at: String, val text: String, val entry: Long, val blocks: List<Md>? = null)

/** What a cell shows. */
@Serializable
data class Cell(
    val text: String,
    /** The notes under the text (annotations). */
    val notes: List<NoteCell>? = null,
    /** The text as Markdown, in a string UDA that uses any. */
    val blocks: List<Md>? = null,
    /** How to style it: `dim`, `multiline`, `pri-h`, `urg-crit`, … */
    val cls: String? = null,
    /** A project path as its parts, so the dots between them can be muted. */
    val segments: List<String>? = null,
    /** Tags, each shown as a pill. */
    val chips: List<String>? = null,
    /** The task's uuid, for copying it from an id cell. */
    val uuid: String? = null,
    /** Other tasks this cell names (their uuids), each drawn as a link. */
    val links: List<String>? = null,
)

enum class UrgencyLevel { Low, Mid, High, Crit }

/**
 * How pressing a score is. Taskwarrior's `next` tag alone is worth 15 and a high priority 6, so 15 and over means something
 * is really on fire, and under 5 is background.
 */
fun urgencyLevel(u: Double): UrgencyLevel = when {
    u >= 15 -> UrgencyLevel.Crit
    u >= 10 -> UrgencyLevel.High
    u >= 5 -> UrgencyLevel.Mid
    else -> UrgencyLevel.Low
}

data class Indicators(val active: String? = null, val tag: String? = null, val dependency: String? = null)

/** What a cell needs to know besides the task. */
data class Ctx(
    /** Epoch seconds "now". */
    val now: Long,
    /** The viewer's offset east of UTC in seconds, for a time with no better offset. */
    val tz: Int = 0,
    /** The offset in force at a moment (it differs across a change of summer time); [tz] when not given. */
    val offsetAt: ((Long) -> Int)? = null,
    val udas: Map<String, UdaDef> = emptyMap(),
    /** Taskwarrior `dateformat*` patterns for table cells and for the notes under a description. */
    val reportDates: DateFmt? = null,
    val annotationDates: DateFmt? = null,
    /** `recurrence.indicator` (default R): what the `recur.indicator` column shows. */
    val recurIndicator: String? = null,
    val indicators: Indicators = Indicators(),
    /** `journal.time` marker texts: those annotations are bookkeeping and are left out of lists. */
    val journal: Journal? = null,
)

fun shortUuid(u: String): String = u.take(8)

/** A project path as its parts (`Home.Kitchen` is `Home` and `Kitchen`). */
fun projectSegments(path: String): List<String> = path.split('.')

private const val FORTNIGHT = 14L * 86_400

/**
 * The offsets the phone's zone had over the moments in the rows, as `[from, offset]` pairs: the engine writes the dates but
 * cannot ask the phone which offset applied, so it is told. Samples a fortnight apart, and finds the exact moment of a change
 * by halving.
 */
private fun offsetsFor(rows: List<Row>, ctx: Ctx, offsetAt: (Long) -> Int): JsonArray {
    var lo = ctx.now
    var hi = ctx.now
    fun see(t: Long?) {
        if (t == null) return
        lo = minOf(lo, t)
        hi = maxOf(hi, t)
    }
    for (r in rows) {
        listOf(r.entry, r.start, r.end, r.due, r.wait, r.scheduled, r.until, r.modified).forEach(::see)
        r.annotations.forEach { see(it.entry) }
    }
    val out = mutableListOf(lo to offsetAt(lo))
    var t = lo
    while (t < hi) {
        val next = minOf(t + FORTNIGHT, hi)
        val here = out.last().second
        if (offsetAt(next) != here) {
            var a = t
            var b = next
            while (b - a > 1) {
                val mid = (a + b) / 2
                if (offsetAt(mid) == here) a = mid else b = mid
            }
            out += b to offsetAt(b)
        }
        t = next
    }
    return buildJsonArray { out.forEach { (from, offset) -> add(buildJsonArray { add(JsonPrimitive(from)); add(JsonPrimitive(offset)) }) } }
}

private fun DateFmt?.json(): JsonElement =
    if (this == null) JsonNull else buildJsonObject { put("pattern", pattern); put("weekstart", weekstart) }

private fun Ctx.json(rows: List<Row>): JsonObject = buildJsonObject {
    put("now", now)
    put("tz", tz)
    put("offsets", offsetAt?.let { offsetsFor(rows, this@json, it) } ?: JsonArray(emptyList()))
    put("udas", buildJsonObject { udas.forEach { (name, u) -> put(name, buildJsonObject { put("type", u.type); put("indicator", u.indicator) }) } })
    put("dates", buildJsonObject { put("report", reportDates.json()); put("annotation", annotationDates.json()) })
    put("recur_indicator", recurIndicator)
    put("indicators", buildJsonObject { put("active", indicators.active); put("tag", indicators.tag); put("dependency", indicators.dependency) })
    put("journal", journal?.let { buildJsonObject { put("start", it.start); put("stop", it.stop) } } ?: JsonNull)
}

/** What every cell of a table shows, as `cells[row][column]`: the engine writes them, all in one call. */
fun formatCells(columns: List<Column>, rows: List<Row>, ctx: Ctx): List<List<Cell>> {
    if (rows.isEmpty() || columns.isEmpty()) return rows.map { emptyList() }
    val cols = buildJsonArray {
        columns.forEach { c -> add(buildJsonObject { put("name", c.name); put("format", c.format); put("kind", c.kind) }) }
    }
    val rowsJson = engineJson.encodeToJsonElement(ListSerializer(Row.serializer()), rows)
    return engineJson.decodeFromJsonElement(
        ListSerializer(ListSerializer(Cell.serializer())),
        callHelper("format_cells", cols, rowsJson, ctx.json(rows)),
    )
}

private class Kept(val result: ReportResult, val ctx: Ctx, val cells: List<List<Cell>>)

private val kept = ArrayDeque<Kept>()

/**
 * The cells of a report's table. A table is drawn again and again as the screen scrolls and changes, so the last few are
 * kept by the result they were made from (and the settings they were made with): one crossing into the engine per result.
 */
fun cellsOf(result: ReportResult, ctx: Ctx): List<List<Cell>> {
    kept.firstOrNull { it.result === result && it.ctx == ctx }?.let { return it.cells }
    val cells = formatCells(result.columns, result.rows, ctx)
    kept.addFirst(Kept(result, ctx, cells))
    while (kept.size > 6) kept.removeLast()
    return cells
}

/** The cells of one row of [result], which a screen that draws a row at a time reads from the table's. */
fun cellsOfRow(result: ReportResult, ctx: Ctx, row: Row): List<Cell> {
    val i = result.rows.indexOfFirst { it === row }
    return if (i >= 0) cellsOf(result, ctx)[i] else formatCells(result.columns, listOf(row), ctx)[0]
}

/** The states of a whole row, for styling it. */
fun rowClass(row: Row): String {
    val c = mutableListOf<String>()
    if (row.status == "completed" || row.status == "deleted") c += listOf("done", row.status)
    if (row.waiting) c += "waiting"
    if (row.blocked) c += "blocked"
    if (row.start != null && row.status == "pending") c += "active"
    return c.joinToString(" ")
}

/** A UDA's display label, falling back to its name. */
fun udaLabel(def: UdaDef): String = def.label?.takeIf { it.isNotEmpty() } ?: def.name
