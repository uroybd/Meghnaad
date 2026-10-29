package me.utsob.meghnaad.app

import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonEncoder
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put

// Boards: named pages of sections (a number, a table or a chart each) kept in `settings.json` under `boards`
// (web/src/lib/boards.ts). What a board file means (which sections are valid, the numbers a section shows, where each
// sits) is the engine's (`tc_core::board`, through `callHelper`), which every front end shares; this file asks it and runs
// the commands that give it tasks. A phone has 4 or 6 columns instead of the grid's 12 (`flowed`): the sections keep their
// reading order and each is as wide as its share of the row, so several numbers sit side by side.

@Serializable
data class Measure(val fn: String, val field: String? = null)

/** The range a chart's date axis covers: one of the board's by its name, or two dates of the chart's own (`tc_core::board::ranges::RangeRef`). */
@Serializable(with = AxisRangeSerializer::class)
sealed interface AxisRange {
    data class Named(val name: String) : AxisRange

    data class Own(val start: String, val end: String) : AxisRange
}

/** A name is written as text and two dates as an object, as the engine reads them. */
object AxisRangeSerializer : KSerializer<AxisRange> {
    override val descriptor: SerialDescriptor = JsonElement.serializer().descriptor

    override fun deserialize(decoder: Decoder): AxisRange {
        val e = (decoder as JsonDecoder).decodeJsonElement()
        return if (e is JsonObject) AxisRange.Own(e["start"]?.jsonPrimitive?.content.orEmpty(), e["end"]?.jsonPrimitive?.content.orEmpty())
        else AxisRange.Named(e.jsonPrimitive.content)
    }

    override fun serialize(encoder: Encoder, value: AxisRange) = (encoder as JsonEncoder).encodeJsonElement(
        when (value) {
            is AxisRange.Named -> JsonPrimitive(value.name)
            is AxisRange.Own -> buildJsonObject {
                put("start", value.start)
                put("end", value.end)
            }
        },
    )
}

/** With a [range] the axis is that range's periods, whatever the tasks are. */
@Serializable
data class TimeBy(val field: String, val step: String, val range: AxisRange? = null)

/** One of the two: the value of an attribute, or a date cut into days, weeks, months, quarters or years. */
@Serializable
data class By(val field: String? = null, val time: TimeBy? = null)

/** A state as of the end of each period, as `task burndown` counts them (see `tc_core::board::aggregate::Span`). */
@Serializable
data class StateSpan(val from: String, val to: String? = null, val unless: String? = null)

/** One series of a chart, as Chart.js has a dataset: a label, and what it is of. */
@Serializable
data class Series(
    val label: String,
    val filter: String? = null,
    val measure: Measure? = null,
    /** A Taskwarrior colour name (`red`, `bright blue`), drawn in the page's palette, or `#rrggbb`. */
    val color: String? = null,
    val span: StateSpan? = null,
)

/** A chart's Keep in `settings.json`: a number, or text (`$week.day_count`). A number is written as one. */
object KeepSerializer : KSerializer<String> {
    override val descriptor: SerialDescriptor = PrimitiveSerialDescriptor("Keep", PrimitiveKind.STRING)
    override fun deserialize(decoder: Decoder): String = (decoder as JsonDecoder).decodeJsonElement().jsonPrimitive.content
    override fun serialize(encoder: Encoder, value: String) =
        (encoder as JsonEncoder).encodeJsonElement(value.trim().takeIf { it.isNotEmpty() && it.all(Char::isDigit) }?.let { JsonPrimitive(it.toInt()) } ?: JsonPrimitive(value))
}

/** A date and the part of it (`weekday`, `hour`, `monthday`, `month`) that is a heatmap's row (`tc_core::board::heat`). */
@Serializable
data class PartBy(val field: String, val part: String)

/** A heatmap's rows: one of the two, the value of an attribute or a part of a date. */
@Serializable
data class Rows(val field: String? = null, val time: PartBy? = null)

val ROW_PARTS = listOf("weekday" to "The weekday of a date", "hour" to "The hour of a date", "monthday" to "The day of the month of a date", "month" to "The month of a date")

@Serializable
data class Section(
    val id: String,
    val title: String = "",
    val x: Int,
    val y: Int,
    val w: Int,
    val h: Int,
    val type: String,
    val report: String? = null,
    val filter: String? = null,
    /** Ask about this section's own tasks only: the board's filter is not added. */
    val ignoreBoardFilter: Boolean? = null,
    val measure: Measure = Measure("count"),
    val by: By? = null,
    val series: List<Series> = emptyList(),
    val stacked: Boolean? = null,
    val sort: String? = null,
    /** How many groups a chart keeps: a number, or the count of a date range of the board (`$week.day_count`), as text. */
    @Serializable(with = KeepSerializer::class) val limit: String? = null,
    /** A heatmap's rows (`by` is its columns): a series is then a colour and a grid of its own, or, stacked, added into one. None: a row for each series. */
    val rows: Rows? = null,
)

/** A named pair of dates any filter on the board can use: `$name.start` and `$name.end`. Each is a date as Taskwarrior reads one. */
@Serializable
data class DateRange(val name: String, val start: String, val end: String)

/** A range worked out for now: the two moments (epoch seconds), or why it cannot be. */
@Serializable
data class ResolvedRange(val name: String, val start: Long? = null, val end: Long? = null, val error: String? = null)

@Serializable
data class Board(
    val id: String,
    val name: String,
    val description: String? = null,
    /** Added to every section's filter. */
    val filter: String? = null,
    val sections: List<Section> = emptyList(),
    val ranges: List<DateRange> = emptyList(),
)

@Serializable
data class NormalizedBoards(val boards: List<Board> = emptyList(), val errors: List<String> = emptyList())

@Serializable
data class BoardGroup(val key: String, val label: String, val value: Double? = null, val count: Int = 0)

/** A number's answer: the measure over every task, whatever the groups. */
@Serializable
data class BoardAnswer(val unit: String = "number", val total: Double? = null, val tasks: Int = 0, val groups: List<BoardGroup> = emptyList())

@Serializable
data class ChartLabel(val key: String, val label: String)

/** One series' value at each label (`null`: nothing to measure there) and the tasks behind each. */
@Serializable
data class Dataset(
    val label: String,
    val data: List<Double?>,
    val counts: List<Int> = emptyList(),
    val total: Double? = null,
    val tasks: Int = 0,
)

/** What a chart draws, in Chart.js's shape: the labels along the axis and a dataset for each series. */
@Serializable
data class BoardChart(
    val unit: String = "number",
    val labels: List<ChartLabel> = emptyList(),
    val datasets: List<Dataset> = emptyList(),
    /** A heatmap's grid: the series it is of. */
    val label: String = "",
)

/** Reading boards and writing them out: what the app does not know about is kept, nothing is written that is a default. */
val boardsJson = Json { ignoreUnknownKeys = true; encodeDefaults = false; explicitNulls = false }

private val BOARDS = ListSerializer(Board.serializer())

fun boardsToJson(boards: List<Board>): JsonElement = boardsJson.encodeToJsonElement(BOARDS, boards)

/** The boards in a saved `boards` value: the valid ones, tidied, and a line for everything dropped. */
fun normalizeBoards(value: JsonElement): NormalizedBoards =
    boardsJson.decodeFromJsonElement(NormalizedBoards.serializer(), callHelper("board_normalize", value))

// ---- The sections' order and slugs

private fun cellsOf(sections: List<Section>) =
    JsonArray(sections.map {
        buildJsonObject {
            put("id", it.id); put("x", it.x); put("y", it.y); put("w", it.w); put("h", it.h)
            // A table or a chart: a phone gives it half a row or a whole one.
            if (it.type != "number") put("wide", true)
        }
    })

/** The sections with the places in [cells] (a list of `{id, x, y, w, h}`), in the order the cells are in. */
private fun withCells(sections: List<Section>, cells: JsonElement): List<Section> {
    val by = sections.associateBy { it.id }
    return cells.jsonArray.mapNotNull { c ->
        val o = c as JsonObject
        by[o.getValue("id").jsonPrimitive.content]?.copy(
            x = o.getValue("x").jsonPrimitive.content.toInt(),
            y = o.getValue("y").jsonPrimitive.content.toInt(),
            w = o.getValue("w").jsonPrimitive.content.toInt(),
            h = o.getValue("h").jsonPrimitive.content.toInt(),
        )
    }
}

/** The columns of the grid a board is saved on, and the narrowest a section may be on it. */
const val BOARD_COLUMNS = 12
const val MIN_WIDTH = 2

/** The sections in the order a phone shows them: top to bottom, left to right. */
fun stacked(sections: List<Section>): List<Section> {
    val order = callHelper("board_stack", cellsOf(sections)).jsonArray.map { it.jsonPrimitive.content }
    val by = sections.associateBy { it.id }
    return order.mapNotNull { by[it] }
}

/** The columns a phone has for a screen [widthDp] wide: six, or four on a small one. */
fun phoneColumns(widthDp: Float) = if (widthDp < 480f) 4 else 6

/** The sections in reading order, placed on a screen of [cols] columns (`x` and `w` are that screen's columns, `y` and `h` rows). */
fun flowed(sections: List<Section>, cols: Int): List<Section> =
    withCells(sections, callHelper("board_flow", cellsOf(sections), JsonPrimitive(cols)))

/** One step of resizing [id] on a screen of [cols] columns: [dcols] of its columns wider and [dh] rows taller, kept in the board's twelve. */
fun resizeFlow(sections: List<Section>, id: String, cols: Int, dcols: Int, dh: Int): List<Section> =
    withCells(sections, callHelper("board_resize_flow", cellsOf(sections), JsonPrimitive(id), JsonPrimitive(cols), JsonPrimitive(dcols), JsonPrimitive(dh)))

/** [id] one place earlier (-1) or later (1) in the reading order, the sections keeping their widths. */
fun reorder(sections: List<Section>, id: String, delta: Int): List<Section> =
    withCells(sections, callHelper("board_reorder", cellsOf(sections), JsonPrimitive(id), JsonPrimitive(delta)))

/** Where a new section [w] wide and [h] high goes: the first free place in reading order, under everything if there is none. */
fun firstFree(sections: List<Section>, w: Int, h: Int): Section {
    val r = callHelper("board_first_free", cellsOf(sections), JsonPrimitive(w), JsonPrimitive(h)).jsonObject
    return Section(
        id = "", x = r.getValue("x").jsonPrimitive.content.toInt(), y = r.getValue("y").jsonPrimitive.content.toInt(),
        w = r.getValue("w").jsonPrimitive.content.toInt(), h = r.getValue("h").jsonPrimitive.content.toInt(), type = "",
    )
}

val SLUG = Regex("^[a-z0-9_-]{1,40}$")

/** Why [slug] cannot be the address of a board, or null. [others] are the slugs of the other boards. */
fun slugProblem(slug: String, others: Collection<String>): String? {
    if (slug.isEmpty()) return "needs an address"
    if (!SLUG.matches(slug)) return "an address is lower case letters a to z, digits, - and _, up to 40"
    if (slug in others) return "there is already a board at $slug"
    return null
}

// ---- What a section runs

/** The filter a section runs: the board's (unless the section ignores it), then its own, then a series', each in parentheses so an `or` cannot leak. */
fun effectiveFilter(board: Board, section: Section, series: Series? = null): String =
    listOf(if (section.ignoreBoardFilter == true) null else board.filter, section.filter, series?.filter)
        .mapNotNull { it?.trim()?.takeIf { f -> f.isNotEmpty() } }
        .joinToString(" ") { "( $it )" }

/** A range's name as a label: `sprint_week` is `Sprint week`. The name itself is what a filter writes. */
fun rangeLabel(name: String): String = name.replace(Regex("_+"), " ").trim().replaceFirstChar { it.uppercase() }

/**
 * The board as it is looked at: with the filter and the range dates typed on the page (BoardFilter.svelte, `viewed`), where there are some.
 * [typed] holds, for a range's name, the dates typed for it.
 */
fun viewedBoard(board: Board, filter: String?, typed: Map<String, DateRange>?): Board =
    board.copy(
        filter = filter ?: board.filter,
        ranges = board.ranges.map { r -> typed?.get(r.name)?.let { r.copy(start = it.start, end = it.end) } ?: r },
    )

/** The series of a chart section: its own, or the one the section itself measures. */
fun seriesOf(section: Section): List<Series> =
    section.series.ifEmpty { listOf(Series(label = section.title, measure = section.measure)) }

fun isChartType(type: String): Boolean = type != "number" && type != "table"

/** What the engine needs to cut dates into periods: the zone now in force, the first day of the week, and today. */
class Zone(val tz: Int, val weekStartsMonday: Boolean, val now: Long) {
    fun toJson(): JsonElement = buildJsonObject {
        put("tz", tz)
        put("week_starts_monday", weekStartsMonday)
        put("now", now)
    }
}

/** As the engine reads `weekstart`: Monday only if the taskrc says so. */
fun boardZone(settings: Map<String, String>?, tz: Int, nowSeconds: Double): Zone =
    Zone(tz, settings?.get("weekstart")?.trim()?.lowercase() == "monday", nowSeconds.toLong())

internal fun rangesJson(ranges: List<DateRange>): JsonElement = boardsJson.encodeToJsonElement(ListSerializer(DateRange.serializer()), ranges)

/** What the board's date ranges are right now, each worked out in the zone and week of the page. */
fun resolveRanges(ranges: List<DateRange>, zone: Zone): List<ResolvedRange> =
    if (ranges.isEmpty()) emptyList()
    else boardsJson.decodeFromJsonElement(ListSerializer(ResolvedRange.serializer()), callHelper("board_ranges", rangesJson(ranges), zone.toJson()))

/** [filter] with every `$name.start` and `$name.end` written as the moment it means; throws why it cannot be. */
fun substituteRanges(filter: String, ranges: List<DateRange>, zone: Zone): String {
    if ('$' !in filter) return filter
    val got = callHelper("board_substitute", JsonPrimitive(filter), rangesJson(ranges), zone.toJson()).jsonObject
    got["message"]?.let { error(it.jsonPrimitive.content) }
    return got["filter"]?.jsonPrimitive?.content ?: filter
}

/** How many groups a chart keeps, worked out now: a number, or the count of a date range (`$week.day_count`); null for all. Throws why not. */
fun keepOf(limit: String?, ranges: List<DateRange>, zone: Zone): Int? {
    if (limit.isNullOrBlank()) return null
    limit.trim().toIntOrNull()?.let { return it.takeIf { n -> n > 0 } }
    val got = callHelper("board_limit", JsonPrimitive(limit), rangesJson(ranges), zone.toJson()).jsonObject
    got["message"]?.let { error(it.jsonPrimitive.content) }
    return got["limit"]?.takeIf { it !is JsonNull }?.jsonPrimitive?.content?.toIntOrNull()
}

/** What the largest group of each range is right now (`day`, `week`, `month` or `year`), by the range's name; none for one that cannot be worked out. */
fun largestGroups(ranges: List<DateRange>, zone: Zone): Map<String, String> =
    ranges.mapNotNull { r -> runCatching { r.name to substituteRanges("\$${r.name}.lg_group.name", ranges, zone) }.getOrNull() }.toMap()

/** The grouping with a step written as a range's largest group worked out. */
fun resolveBy(by: By?, ranges: List<DateRange>, zone: Zone): By? {
    val time = by?.time ?: return by
    if (!time.step.startsWith("$")) return by
    return By(time = time.copy(step = substituteRanges(time.step, ranges, zone)))
}

private fun measureJson(m: Measure): JsonElement = boardsJson.encodeToJsonElement(Measure.serializer(), m)

/** The number for a section from an `export` text. */
fun aggregateNumber(text: String, section: Section, zone: Zone): BoardAnswer {
    val query = buildJsonObject {
        put("measure", measureJson(section.measure))
        section.by?.let { put("by", boardsJson.encodeToJsonElement(By.serializer(), it)) }
    }
    return boardsJson.decodeFromJsonElement(BoardAnswer.serializer(), callHelper("board_aggregate", JsonPrimitive(text), query, zone.toJson()))
}

/** What a chart section's series, laid on one axis. */
fun layChart(inputs: List<SeriesInput>, by: By?, zone: Zone, ranges: List<DateRange> = emptyList()): BoardChart {
    val series = JsonArray(
        inputs.map {
            buildJsonObject {
                put("label", it.label)
                put("tasks", it.tasks)
                put("measure", measureJson(it.measure))
                it.span?.let { s -> put("span", boardsJson.encodeToJsonElement(StateSpan.serializer(), s)) }
            }
        },
    )
    val byJson: JsonElement = by?.let { boardsJson.encodeToJsonElement(By.serializer(), it) } ?: JsonNull
    return boardsJson.decodeFromJsonElement(BoardChart.serializer(), callHelper("board_chart", series, byJson, zone.toJson(), rangesJson(ranges)))
}

class SeriesInput(val label: String, val tasks: String, val measure: Measure, val span: StateSpan?)

/** A heatmap with rows: a grid for each series (one, when they are stacked), the columns the same in each. */
fun layHeatmap(inputs: List<SeriesInput>, by: By, rows: Rows, stacked: Boolean, zone: Zone, ranges: List<DateRange> = emptyList()): List<BoardChart> {
    val series = JsonArray(
        inputs.map {
            buildJsonObject {
                put("label", it.label)
                put("tasks", it.tasks)
                put("measure", measureJson(it.measure))
                it.span?.let { s -> put("span", boardsJson.encodeToJsonElement(StateSpan.serializer(), s)) }
            }
        },
    )
    val got = callHelper(
        "board_heatmap", series, boardsJson.encodeToJsonElement(By.serializer(), by), boardsJson.encodeToJsonElement(Rows.serializer(), rows),
        JsonPrimitive(stacked), zone.toJson(), rangesJson(ranges),
    )
    return boardsJson.decodeFromJsonElement(ListSerializer(BoardChart.serializer()), got)
}

/**
 * The grids of a heatmap with their columns in the order the section asks for and as many as it keeps, the same columns in each: they
 * are ordered together, by what all the rows add up to, so the grids line up (orderGrids in boards.ts).
 */
fun orderGrids(grids: List<BoardChart>, section: Section, limit: Int? = section.limit?.toIntOrNull()): List<BoardChart> {
    val first = grids.firstOrNull() ?: return grids
    val together = BoardChart(first.unit, first.labels, grids.flatMap { it.datasets })
    val at = first.labels.withIndex().associate { (i, l) -> l.key to i }
    val order = orderChart(together, section, limit).labels.map { at[it.key] ?: 0 }
    return grids.map { g ->
        g.copy(
            labels = order.map { g.labels[it] },
            datasets = g.datasets.map { d -> d.copy(data = order.map { d.data.getOrNull(it) }, counts = order.map { d.counts.getOrElse(it) { 0 } }) },
        )
    }
}

/** A heatmap's rows in words: "weekday of end", "the value of project"; null when there are none. */
fun describeRows(rows: Rows?): String? {
    val part = mapOf("weekday" to "weekday", "hour" to "hour", "monthday" to "day of the month", "month" to "month")
    rows?.time?.let { return "${part[it.part] ?: it.part} of ${it.field}" }
    return rows?.field?.takeIf { it.isNotEmpty() }?.let { "the value of $it" }
}

/** The chart with its labels in the order the section asks for (the largest total first for a value, time order for a date), and as many as it keeps (for a date in time order, the latest). */
fun orderChart(chart: BoardChart, section: Section, limit: Int? = section.limit?.toIntOrNull()): BoardChart {
    val sort = section.sort ?: if (section.by?.time != null) null else "value-"
    fun total(i: Int) = chart.datasets.sumOf { it.data.getOrNull(i) ?: 0.0 }
    fun nothing(i: Int) = chart.datasets.all { it.data.getOrNull(i) == null }
    var order = chart.labels.indices.toList()
    if (sort != null) {
        val sign = if (sort.endsWith("-")) -1 else 1
        order = if (sort.startsWith("key")) {
            order.sortedWith { a, b -> sign * chart.labels[a].key.compareTo(chart.labels[b].key) }
        } else {
            // A label with nothing to measure goes last, whichever way it is sorted.
            order.sortedWith { a, b ->
                if (nothing(a) || nothing(b)) nothing(a).compareTo(nothing(b)) else sign * total(a).compareTo(total(b))
            }
        }
    }
    // A date in time order keeps the latest periods (a week is `per day`, keep 7); any other order keeps its first.
    limit?.takeIf { it > 0 }?.let { order = if (section.by?.time != null && sort == null) order.takeLast(it) else order.take(it) }
    return BoardChart(
        chart.unit,
        order.map { chart.labels[it] },
        chart.datasets.map { d -> d.copy(data = order.map { d.data.getOrNull(it) }, counts = order.map { d.counts.getOrElse(it) { 0 } }) },
        chart.label,
    )
}

/** What one filter got: the filter it ran, and the tasks it picked, as the text `export` printed them. */
class Source(val label: String, val filter: String, val text: String)

/** What a section got: its result, and (for the info panel) where the result came from. */
sealed interface SectionData {
    class Number(val answer: BoardAnswer, val sources: List<Source>) : SectionData
    /** [limit] is how many groups the chart keeps, worked out now (see [keepOf]); null for all. */
    class Chart(val chart: BoardChart, val sources: List<Source>, val limit: Int? = null, val grids: List<BoardChart>? = null) : SectionData
    class Table(val result: CliResult, val filter: String) : SectionData
    class Failed(val message: String) : SectionData
}

/**
 * Every section's data. Sections that run the same filter share one `export`; a table runs its report, as the Tasks page does,
 * with the section's filter.
 */
suspend fun loadBoard(tw: Tw, board: Board, zone: Zone): Map<String, SectionData> {
    val exports = HashMap<String, CliResult>()
    /** The filter a section (or one of its series) runs, with the board's date ranges in it. */
    fun filterOf(s: Section, x: Series? = null) = substituteRanges(effectiveFilter(board, s, x), board.ranges, zone)
    suspend fun exportOf(filter: String): CliResult =
        exports.getOrPut(filter) { tw.run(args = rustHelpers().splitWords(filter) + "export").result }

    /** The tasks a filter picks, as the text `export` prints; or why they could not be had. */
    suspend fun tasksOf(filter: String): Pair<String?, String?> = when (val r = exportOf(filter)) {
        is CliResult.File -> r.value.text to null
        is CliResult.Error -> null to r.value.message
        else -> null to "The tasks could not be read."
    }

    val out = LinkedHashMap<String, SectionData>()
    for (section in board.sections) {
        out[section.id] = try {
            // A step written as a range's largest group is the step it is now.
            val s = section.copy(by = resolveBy(section.by, board.ranges, zone))
            when {
                s.type == "table" -> {
                    val filter = filterOf(s)
                    val r = tw.run(args = rustHelpers().splitWords(filter) + (s.report ?: "next")).result
                    if (r is CliResult.Error) SectionData.Failed(r.value.message) else SectionData.Table(r, filter)
                }
                s.type == "number" -> {
                    val filter = filterOf(s)
                    val (text, error) = tasksOf(filter)
                    if (text == null) SectionData.Failed(error ?: "")
                    else SectionData.Number(aggregateNumber(text, s, zone), listOf(Source(s.title, filter, text)))
                }
                else -> {
                    val inputs = ArrayList<SeriesInput>()
                    val sources = ArrayList<Source>()
                    var failure: String? = null
                    for (x in seriesOf(s)) {
                        val filter = filterOf(s, x)
                        val (text, error) = tasksOf(filter)
                        if (text == null) {
                            failure = "${x.label}: $error"
                            break
                        }
                        inputs += SeriesInput(x.label, text, x.measure ?: s.measure, x.span)
                        sources += Source(x.label, filter, text)
                    }
                    val rows = s.rows
                    val by = s.by
                    if (failure != null) SectionData.Failed(failure)
                    else if (s.type == "heatmap" && rows != null && by != null) {
                        // A heatmap with rows: a grid for each series, the columns the same in each.
                        val grids = layHeatmap(inputs, by, rows, s.stacked == true, zone, board.ranges)
                        SectionData.Chart(grids.first(), sources, keepOf(s.limit, board.ranges, zone), grids)
                    } else SectionData.Chart(layChart(inputs, s.by, zone, board.ranges), sources, keepOf(s.limit, board.ranges, zone))
                }
            }
        } catch (e: Exception) {
            SectionData.Failed(e.message ?: e.toString())
        }
    }
    return out
}

// ---- In words, for the info panel and the screen reader

private val FN_WORDS = mapOf("count" to "Count", "sum" to "Sum", "avg" to "Average", "min" to "Smallest", "max" to "Largest")

/** What a measure computes, in words: "Count of tasks", "Sum of points". */
fun describeMeasure(m: Measure): String = if (m.fn == "count") "Count of tasks" else "${FN_WORDS[m.fn] ?: m.fn} of ${m.field ?: "…"}"

/** What a chart is grouped by, in words: "project", "end, per week"; null when it is not grouped. */
fun describeBy(by: By?): String? = by?.time?.let {
    val over = when (val r = it.range) {
        null -> ""
        is AxisRange.Named -> ", over ${rangeLabel(r.name)}"
        is AxisRange.Own -> ", over ${r.start} to ${r.end}"
    }
    "${it.field}, per ${it.step}$over"
} ?: by?.field?.takeIf { it.isNotEmpty() }

/** The span in words: "open at the end of each period, from entry until end, unless start has passed". */
fun describeSpan(span: StateSpan): String {
    val to = span.to?.let { " until $it" } ?: ""
    val unless = span.unless?.let { ", unless $it has passed" } ?: ""
    return "open at the end of each period, from ${span.from}$to$unless"
}

/** A value for a screen: a duration the way Taskwarrior writes one, a number without trailing noise; a dash for nothing. */
fun formatBoardValue(value: Double?, unit: String): String {
    if (value == null) return "–"
    if (unit == "duration") return rustHelpers().vague(value.toLong())
    return if (value == kotlin.math.floor(value) && kotlin.math.abs(value) < 1e15) value.toLong().toString() else (kotlin.math.round(value * 100) / 100).toString()
}

/** A task as `export` prints it. */
typealias ExportedTask = JsonObject

/** The tasks in an `export` text: a JSON array (`json.array` on), or one task per line. */
fun parseExportedTasks(text: String): List<ExportedTask> {
    val t = text.trim()
    if (t.isEmpty()) return emptyList()
    runCatching { Json.parseToJsonElement(t) }.getOrNull()?.let { v ->
        return if (v is JsonArray) v.filterIsInstance<JsonObject>() else listOfNotNull(v as? JsonObject)
    }
    return t.lines().mapNotNull { runCatching { Json.parseToJsonElement(it.trim()) }.getOrNull() as? JsonObject }
}

/** The value of a column of an exported task as text; a dash for nothing, a date in the user's format. */
fun exportedCell(task: ExportedTask, column: String, dateText: (Long) -> String): String {
    val v = task[column] ?: return "–"
    if (v is JsonNull) return "–"
    if (v is JsonArray) return v.joinToString(", ") { (it as? JsonPrimitive)?.contentOrNull ?: it.toString() }.ifEmpty { "–" }
    if (v !is JsonPrimitive) return v.toString()
    val s = v.contentOrNull ?: return "–"
    if (s.isEmpty() || (column == "id" && v.doubleOrNull == 0.0)) return "–"
    return exportedStampEpoch(s)?.let(dateText) ?: s
}

private val STAMP = Regex("^(\\d{4})(\\d\\d)(\\d\\d)T(\\d\\d)(\\d\\d)(\\d\\d)Z$")

/** A date as `export` prints it (`20261007T123000Z`) in seconds since 1970, or null for anything else. */
fun exportedStampEpoch(s: String): Long? {
    val m = STAMP.matchEntire(s) ?: return null
    val (y, mo, d, h, mi, se) = m.destructured
    val days = daysFromCivil(y.toInt(), mo.toInt(), d.toInt())
    return days * 86400 + h.toInt() * 3600 + mi.toInt() * 60 + se.toInt()
}
