package me.utsob.meghnaad.app

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

// Making and changing boards on the phone (web/src/lib/boards.ts and BoardsPage.svelte). Which boards and sections are valid is the
// engine's (`board_normalize`), which the draft asks before it can be saved, so what is refused here is what the web refuses. Where
// sections go (a new one's place, moving one, resizing one in the phone's columns) is the engine's too (`tc_core::board::layout`).

class SectionType(val id: String, val label: String)

val SECTION_TYPES = listOf(
    SectionType("number", "Number"),
    SectionType("bar", "Bar chart"),
    SectionType("line", "Line chart"),
    SectionType("area", "Area chart"),
    SectionType("pie", "Pie chart"),
    SectionType("donut", "Donut chart"),
    SectionType("radar", "Radar chart"),
    SectionType("polar", "Polar area chart"),
    SectionType("heatmap", "Heatmap"),
    SectionType("table", "Table"),
)

val MEASURES = listOf("count" to "Count of tasks", "sum" to "Sum of", "avg" to "Average of", "min" to "Smallest", "max" to "Largest")

val STEPS = listOf("day", "week", "month", "quarter", "year")

/** The orders a chart of values can take; none is the default (largest first, or time order for a date). */
val ORDERS = listOf(null to "Default", "value-" to "Largest first", "value+" to "Smallest first", "key+" to "By name, A to Z", "key-" to "By name, Z to A")

class SpanPreset(val id: String, val label: String, val span: StateSpan)

/** The states of `task burndown`, and how a span writes each. */
val SPAN_PRESETS = listOf(
    SpanPreset("pending", "Pending at the end of each period", StateSpan("entry", "end", "start")),
    SpanPreset("started", "Started at the end of each period", StateSpan("start", "end")),
    SpanPreset("done", "Done by the end of each period (adds up)", StateSpan("end")),
)

/** The most series a chart may have (the engine's limit). */
const val MAX_SERIES = 12

fun isRoundType(type: String) = type == "pie" || type == "donut" || type == "polar"

/** A pie, donut or polar chart has one series; the others may have several. */
fun hasManySeries(type: String) = isChartType(type) && !isRoundType(type)

/** Which of the states of `task burndown` a span is, `custom` for any other, and `period` for none. */
fun spanKind(series: Series): String {
    val span = series.span ?: return "period"
    return SPAN_PRESETS.firstOrNull { it.span == span }?.id ?: "custom"
}

/** The span a kind of series has to begin with; none for what happened in each period. */
fun spanFor(kind: String): StateSpan? = when (kind) {
    "period" -> null
    "custom" -> StateSpan("entry")
    else -> SPAN_PRESETS.firstOrNull { it.id == kind }?.span
}

/** The dates a chart can be cut by or a span can run between: Taskwarrior's, and the user's date UDAs. */
fun dateFields(udas: List<UdaDef>) = listOf("due", "entry", "end", "start", "wait", "scheduled", "until", "modified") + udas.filter { it.type == "date" }.map { it.name }

/** What a section can measure: urgency and the user's numeric and duration UDAs. */
fun measurableFields(udas: List<UdaDef>) = listOf("urgency") + udas.filter { it.type == "numeric" || it.type == "duration" }.map { it.name }

/** What a chart can be grouped by. */
fun groupableFields(udas: List<UdaDef>) = listOf("project", "tags", "priority", "status") + udas.filter { it.type == "string" || it.type == "numeric" }.map { it.name }

/** An id built from [base] that is not in [taken]: `weekly`, `weekly-2`, … */
fun freshId(base: String, taken: Collection<String>): String {
    val stem = base.lowercase().replace(Regex("[^a-z0-9]+"), "-").trim('-').take(30).ifEmpty { "board" }
    if (stem !in taken) return stem
    var n = 2
    while ("$stem-$n" in taken) n++
    return "$stem-$n"
}

fun newBoard(name: String, boards: List<Board>): Board = Board(freshId(name, boards.map { it.id }), name.trim())

/** A copy of [section], in the first free place of [board], with an id and a title of its own (copySection in boards.ts). */
fun copySection(board: Board, section: Section): Section {
    val at = firstFree(board.sections, section.w, section.h)
    return section.copy(
        id = freshId(section.id.replace(Regex("-\\d+$"), ""), board.sections.map { it.id }),
        title = if (section.title.isEmpty()) "" else "${section.title} copy",
        x = at.x,
        y = at.y,
        w = at.w,
        h = at.h,
    )
}

/** A copy of [board] with a name and an address of its own (copyBoard in boards.ts). */
fun copyBoard(board: Board, boards: List<Board>): Board {
    val name = "${board.name} copy"
    return board.copy(id = freshId(name, boards.map { it.id }), name = name)
}

/** The size, in the grid's columns and rows, a new section of each type has to begin with (as on the web). */
private val DEFAULT_SIZE = mapOf(
    "number" to (3 to 2), "bar" to (6 to 5), "line" to (6 to 5), "area" to (6 to 5),
    "pie" to (4 to 5), "donut" to (4 to 5), "radar" to (4 to 5), "polar" to (4 to 5), "heatmap" to (6 to 5), "table" to (12 to 6),
)

/** A new section of this type, in the first free place. */
fun newSection(board: Board, type: String): Section {
    val label = SECTION_TYPES.firstOrNull { it.id == type }?.label ?: type
    val by = when {
        type == "line" || type == "area" || type == "heatmap" -> By(time = TimeBy("entry", "week"))
        isChartType(type) -> By(field = "project")
        else -> null
    }
    // A heatmap is a row for each series and a column for each week: what was added, and what of it is done.
    val series = if (type == "heatmap") listOf(Series("Pending", filter = "status:pending"), Series("Done", filter = "status:completed")) else emptyList()
    val (w, h) = DEFAULT_SIZE.getValue(type)
    val at = firstFree(board.sections, w, h)
    return Section(
        id = freshId(type, board.sections.map { it.id }),
        title = label,
        x = at.x,
        y = at.y,
        w = at.w,
        h = at.h,
        type = type,
        report = if (type == "table") "next" else null,
        by = by,
        series = series,
    )
}

/** A section's type changed: a chart gets something to group by, a table a report, a round chart loses what it cannot have. */
fun withType(section: Section, type: String): Section {
    var next = section.copy(type = type)
    if (isChartType(type) && next.by == null) {
        next = next.copy(by = if (type == "line" || type == "area" || type == "heatmap") By(time = TimeBy("entry", "week")) else By(field = "project"))
    }
    if (type == "table" && next.report == null) next = next.copy(report = "next")
    // A pie has one series, and none stacks; neither does a heatmap, which is a row for each series.
    if (isRoundType(type)) next = next.copy(series = emptyList(), stacked = null)
    if (type == "heatmap") next = next.copy(stacked = null)
    return next
}

/** The draft of a board being edited: a copy, so cancelling leaves the saved one alone. */
class BoardEditor(private val store: Store, original: Board) {
    var draft by mutableStateOf(original)
        private set

    /** The slug the draft had when it was opened, which it may change. */
    val from = original.id

    /** The section last touched, outlined on the board. */
    var selected by mutableStateOf<String?>(null)

    /** The section whose settings are open in the pop-up. */
    var opened by mutableStateOf<String?>(null)

    var busy by mutableStateOf(false)
    var error by mutableStateOf<String?>(null)

    fun edit(change: (Board) -> Board) {
        draft = change(draft)
        error = null
    }

    fun update(section: Section) = edit { b -> b.copy(sections = b.sections.map { if (it.id == section.id) section else it }) }

    fun add(type: String) {
        val s = newSection(draft, type)
        edit { it.copy(sections = it.sections + s) }
        selected = s.id
        opened = s.id
    }

    fun duplicate(id: String) {
        val original = draft.sections.firstOrNull { it.id == id } ?: return
        val s = copySection(draft, original)
        edit { it.copy(sections = it.sections + s) }
        selected = s.id
    }

    fun remove(id: String) {
        edit { b -> b.copy(sections = b.sections.filter { it.id != id }) }
        if (selected == id) selected = null
        if (opened == id) opened = null
    }

    fun move(id: String, delta: Int) = edit { it.copy(sections = reorder(it.sections, id, delta)) }

    /** Put the sections as [sections] have them (a drag of a corner works from the layout it began with). */
    fun layout(sections: List<Section>) = edit { it.copy(sections = sections) }

    /** What the engine says of the draft: the sections it would drop, and the board it would keep. */
    val check get() = normalizeBoards(boardsToJson(listOf(draft)))

    val slugError get() = slugProblem(draft.id, store.boards.filter { it.id != from }.map { it.id })

    val valid get() = slugError == null && check.let { it.errors.isEmpty() && it.boards.size == 1 }

    /** Save the draft (the home page moves with the board if its slug changed). True when it was saved. */
    suspend fun save(): Boolean {
        // The engine's tidied board leaves out what it refuses, so a draft with a problem is not saved at all.
        if (!valid) return false
        val kept = check.boards.firstOrNull() ?: return false
        busy = true
        error = null
        return try {
            val moved = store.homepage?.board == from && kept.id != from
            store.saveBoards(store.boards.map { if (it.id == from) kept else it }, if (moved) Homepage(board = kept.id) else null, setHomepage = moved)
            store.boardId = kept.id
            true
        } catch (e: Exception) {
            error = e.message ?: e.toString()
            false
        } finally {
            busy = false
        }
    }

    /** Delete the board; the home page is taken out with it if it was the home page. */
    suspend fun delete(): Boolean {
        busy = true
        error = null
        return try {
            val wasHome = store.homepage?.board == from
            store.saveBoards(store.boards.filter { it.id != from }, null, setHomepage = wasHome)
            store.boardId = null
            true
        } catch (e: Exception) {
            error = e.message ?: e.toString()
            false
        } finally {
            busy = false
        }
    }
}

/** Save a copy of [board] and show it; null with the reason in [Store.toast] when it could not be saved. */
suspend fun duplicateBoard(store: Store, board: Board): Board? {
    val made = copyBoard(board, store.boards)
    return try {
        store.saveBoards(store.boards + made)
        store.boardId = made.id
        made
    } catch (e: Exception) {
        store.notify(e.message ?: e.toString(), bad = true)
        null
    }
}

/** Make a board with this name and open it to edit; null with the reason in [Store.toast] when it could not be saved. */
suspend fun createBoard(store: Store, name: String): Board? {
    val made = newBoard(name, store.boards)
    return try {
        store.saveBoards(store.boards + made)
        made
    } catch (e: Exception) {
        store.notify(e.message ?: e.toString(), bad = true)
        null
    }
}
