package me.utsob.meghnaad.app

// Renders a report cell from a column spec like `due.relative` or `description.truncated`, following Taskwarrior's column
// formats where they make sense on a phone: web/src/lib/format.ts.

/** What a cell shows. */
class Cell(
    val text: String,
    /** Extra lines under the text (annotations). */
    val lines: List<String>? = null,
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
) {
    override fun equals(other: Any?) = other is Cell && text == other.text && lines == other.lines && cls == other.cls &&
        segments == other.segments && chips == other.chips && uuid == other.uuid && links == other.links

    override fun hashCode() = text.hashCode()

    override fun toString() = "Cell($text, lines=$lines, cls=$cls, segments=$segments, chips=$chips, uuid=$uuid, links=$links)"
}

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

class Indicators(val active: String? = null, val tag: String? = null, val dependency: String? = null)

/** What a cell needs to know besides the task. */
class Ctx(
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

private fun Ctx.zone(ts: Long) = offsetAt?.invoke(ts) ?: tz

private val DATE_PROPS = setOf("entry", "start", "end", "due", "wait", "scheduled", "until", "modified")

fun shortUuid(u: String): String = u.take(8)

private fun dateOf(row: Row, name: String): Long? = when (name) {
    "entry" -> row.entry
    "start" -> row.start
    "end" -> row.end
    "due" -> row.due
    "wait" -> row.wait
    "scheduled" -> row.scheduled
    "until" -> row.until
    else -> row.modified
}

private fun dateCell(ts: Long?, format: String?, ctx: Ctx): Cell {
    if (ts == null) return Cell("")
    return when (format) {
        // A duration is Taskwarrior's own `format_vague`, from the engine.
        "relative", "countdown" -> Cell(rustHelpers().vague(ts - ctx.now))
        "remaining" -> Cell(if (ts > ctx.now) rustHelpers().vague(ts - ctx.now) else "")
        "age" -> Cell(rustHelpers().vague(ctx.now - ts))
        "epoch" -> Cell(ts.toString())
        "iso" -> Cell(isoStamp(ts))
        "julian" -> Cell(fixed(ts / 86_400.0 + 2_440_587.5, 5))
        else -> Cell(formatMoment(ts, ctx.zone(ts), ctx.reportDates))
    }
}

private fun truncate(s: String, n: Int) = if (s.length <= n) s else s.take(maxOf(0, n - 1)) + "…"

private fun descriptionCell(row: Row, format: String?, ctx: Ctx): Cell {
    val shown = visibleNotes(row.annotations, ctx.journal)
    val notes = shown.map { "${formatMoment(it.entry, ctx.zone(it.entry), ctx.annotationDates)} ${it.text}" }
    return when (format) {
        "desc" -> Cell(row.description)
        // Like Taskwarrior: each annotation follows the description with its date.
        "oneline" -> Cell((listOf(row.description) + notes).joinToString(" "))
        "truncated" -> Cell(truncate(row.description, 40))
        "truncated_count" -> Cell(truncate(row.description, 40) + if (notes.isNotEmpty()) " [${notes.size}]" else "")
        "count" -> Cell(row.description + if (notes.isNotEmpty()) " [${notes.size}]" else "")
        // Taskwarrior's default shows the annotations beneath the description.
        else -> Cell(row.description, lines = notes.ifEmpty { null })
    }
}

/** A project path as its parts (`Home.Kitchen` is `Home` and `Kitchen`). */
fun projectSegments(path: String): List<String> = path.split('.')

private fun projectCell(p: String?, format: String?): Cell {
    if (p.isNullOrEmpty()) return Cell("")
    val parts = p.split('.')
    if (format == "indented") return Cell("  ".repeat(parts.size - 1) + parts.last())
    val text = if (format == "parent" && parts.size > 1) parts.dropLast(1).joinToString(".") else p
    return Cell(text, segments = projectSegments(text))
}

/** UDA values for date-typed UDAs are stored as epoch seconds. */
private fun udaCell(row: Row, col: Column, ctx: Ctx): Cell {
    val raw = row.extra[col.name] ?: return Cell("")
    val def = ctx.udas[col.name]
    if (def?.type == "date" || col.kind == "date") {
        val ts = raw.toDoubleOrNull()
        return if (ts != null && ts.isFinite()) dateCell(ts.toLong(), col.format, ctx) else Cell(raw)
    }
    if (def?.indicator != null && col.format == "indicator") return Cell(def.indicator)
    // A multi-line value keeps its lines instead of running them together.
    return if ('\n' in raw) Cell(raw, cls = "multiline") else Cell(raw)
}

fun cell(col: Column, row: Row, ctx: Ctx): Cell {
    val f = col.format
    val name = col.name

    // `start.active`: the indicator while the task is started and not ended.
    if (name == "start" && f == "active") return Cell(if (row.start != null && row.end == null) ctx.indicators.active ?: "*" else "")

    if (name in DATE_PROPS) return dateCell(dateOf(row, name), f, ctx)

    return when (name) {
        // Ids are only meaningful on this replica; the uuid is what to copy.
        "id" -> if (row.id != null) Cell(row.id.toString(), uuid = row.uuid) else Cell(shortUuid(row.uuid), cls = "dim", uuid = row.uuid)
        "uuid" -> Cell(if (f == "short") shortUuid(row.uuid) else row.uuid)
        "status" -> {
            if (f == "short") Cell(if (row.waiting) "W" else row.status.take(1).uppercase()) else Cell(if (row.waiting) "waiting" else row.status)
        }
        "description" -> descriptionCell(row, f, ctx)
        "project" -> projectCell(row.project, f)
        "priority" -> Cell(row.priority ?: "", cls = row.priority?.let { "pri-${it.lowercase()}" })
        "tags" -> when (f) {
            "count" -> Cell(if (row.tags.isNotEmpty()) "[${row.tags.size}]" else "")
            "indicator" -> Cell(if (row.tags.isNotEmpty()) ctx.indicators.tag ?: "+" else "")
            "list" -> Cell(row.tags.joinToString(","))
            else -> Cell(row.tags.joinToString(" "), chips = row.tags.ifEmpty { null })
        }
        "depends" -> {
            // Only the tasks still open count: a finished one no longer holds this one up.
            val open = row.pendingDeps
            when (f) {
                "indicator" -> Cell(if (open > 0) ctx.indicators.dependency ?: "D" else "")
                "count" -> Cell(if (open > 0) "[$open]" else "")
                else -> Cell(row.depends.joinToString(" ", transform = ::shortUuid), links = row.depends.ifEmpty { null })
            }
        }
        "urgency" -> Cell(
            if (f == "integer") roundHalfUp(row.urgency).toString() else fixed(row.urgency, 2),
            cls = "urg-${urgencyLevel(row.urgency).name.lowercase()}",
        )
        "recur" -> Cell(if (f == "indicator") (if (row.recur != null) ctx.recurIndicator ?: "R" else "") else row.recur ?: "")
        "parent" -> Cell(row.parent?.let(::shortUuid) ?: "", links = row.parent?.let { listOf(it) })
        "annotations" -> Cell(
            if (f == "count") (row.annotations.size.takeIf { it > 0 }?.toString() ?: "") else row.annotations.joinToString("; ") { it.text },
        )
        // A UDA, an orphan, or something Taskwarrior has that is not drawn specially.
        else -> udaCell(row, col, ctx)
    }
}

private fun roundHalfUp(x: Double): Long = kotlin.math.floor(x + 0.5).toLong()

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
