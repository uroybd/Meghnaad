package me.utsob.meghnaad.app

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

// Making and changing boards on the phone (web/src/lib/boards.ts and BoardsPage.svelte). Which boards and sections are valid is the
// engine's (`board_normalize`), which the draft asks before it can be saved, so what is refused here is what the web refuses. The
// phone has one column, so a section's place is just its order: every section is full width, one under the other.

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

private val DEFAULT_HEIGHT = mapOf("number" to 2, "table" to 6)

/** The sections one under the other, in the order given. */
fun relayout(sections: List<Section>): List<Section> {
    var y = 0
    return sections.map { s -> s.copy(x = 0, y = y, w = 12).also { y += s.h } }
}

/** A new section of this type, under the others. */
fun newSection(board: Board, type: String): Section {
    val label = SECTION_TYPES.firstOrNull { it.id == type }?.label ?: type
    val by = when {
        type == "line" || type == "area" -> By(time = TimeBy("entry", "week"))
        isChartType(type) -> By(field = "project")
        else -> null
    }
    return Section(
        id = freshId(type, board.sections.map { it.id }),
        title = label,
        x = 0,
        y = board.sections.sumOf { it.h },
        w = 12,
        h = DEFAULT_HEIGHT[type] ?: 5,
        type = type,
        report = if (type == "table") "next" else null,
        by = by,
    )
}

/** [id] one place earlier (-1) or later (1) in the reading order; the sections are laid one under the other in the new order. */
fun reorder(sections: List<Section>, id: String, delta: Int): List<Section> {
    val order = stacked(sections).toMutableList()
    val at = order.indexOfFirst { it.id == id }
    val to = at + delta
    if (at < 0 || to < 0 || to >= order.size) return sections
    val moved = order.removeAt(at)
    order.add(to, moved)
    return relayout(order)
}

/** A section's type changed: a chart gets something to group by, a table a report, a round chart loses what it cannot have. */
fun withType(section: Section, type: String): Section {
    var next = section.copy(type = type)
    if (isChartType(type) && next.by == null) {
        next = next.copy(by = if (type == "line" || type == "area") By(time = TimeBy("entry", "week")) else By(field = "project"))
    }
    if (type == "table" && next.report == null) next = next.copy(report = "next")
    if (isRoundType(type)) next = next.copy(series = emptyList(), stacked = null)
    return next
}

/** The draft of a board being edited: a copy, so cancelling leaves the saved one alone. */
class BoardEditor(private val store: Store, original: Board) {
    var draft by mutableStateOf(original)
        private set

    /** The slug the draft had when it was opened, which it may change. */
    val from = original.id

    /** The section whose settings are open. */
    var selected by mutableStateOf<String?>(null)

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
    }

    fun remove(id: String) {
        edit { b -> b.copy(sections = relayout(b.sections.filter { it.id != id })) }
        if (selected == id) selected = null
    }

    fun move(id: String, delta: Int) = edit { it.copy(sections = reorder(it.sections, id, delta)) }

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
