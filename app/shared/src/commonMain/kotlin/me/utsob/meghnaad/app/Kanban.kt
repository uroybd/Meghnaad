package me.utsob.meghnaad.app

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject

// Kanbans (spec 0026; web/src/lib/kanban.ts): ordered columns, each a filter that picks its tasks and a command line run on
// a task moved into it. A kanban is drawn from a report (its filter, sort and columns, which are what a card shows) and adds
// its own filter; the screen can change the report, the filter and Completed since while it is open. Which task is in which
// column is what the engine answers, never decided here. The UDA, the report, the filter, Completed since and the columns
// are Kanban's own, for every kanban; a kanban has a name and an address and may have its own report, filter, Completed
// since or columns in place of those.

data class KanbanColumn(
    val name: String,
    /** Picks the column's tasks; a task is in the first column whose filter matches it. */
    val filter: String,
    /** Run on a task dropped into the column, written as after a filter. Empty: the column takes no drops. */
    val move: String,
)

/** A kanban as it is saved: what it doesn't have of its own (null) is Kanban's. */
data class KanbanEntry(
    /** Its slug, the last part of its link (`meghnaad://kanban/<id>`). */
    val id: String,
    val name: String,
    val report: String? = null,
    val filter: String? = null,
    val completedSince: String? = null,
    val columns: List<KanbanColumn>? = null,
)

/** A kanban as it is drawn: its own settings, else Kanban's. */
data class Kanban(
    val id: String,
    val name: String,
    /** The report it opens on: its filter, sort and columns. */
    val report: String,
    /** Added to every column (empty: none). */
    val filter: String = "",
    /** Completed tasks that ended before this date are left out (empty: none are). */
    val completedSince: String = "",
    val columns: List<KanbanColumn> = emptyList(),
)

const val PROGRESS_UDA = "progressState"

data class KanbanSettings(
    val enabled: Boolean = false,
    /** How it was set up (`automatic` or `manual`), when it was. */
    val setup: String? = null,
    /** The UDA the automatic setup defines and its columns use. */
    val uda: String = PROGRESS_UDA,
    val report: String = "next",
    val filter: String = "",
    val completedSince: String = "",
    val columns: List<KanbanColumn> = emptyList(),
    val kanbans: List<KanbanEntry> = emptyList(),
)

/** [k] as it is drawn: each setting its own, else Kanban's. */
fun resolveKanban(s: KanbanSettings, k: KanbanEntry) = Kanban(
    k.id, k.name, k.report ?: s.report, k.filter ?: s.filter, k.completedSince ?: s.completedSince, k.columns ?: s.columns,
)

val PROGRESS_VALUES = listOf("inprogress", "review")
const val AUTO_REPORT = "all"

/** Leaves out deleted and waiting tasks and recurring templates, which `all` (no filter of its own) shows. */
const val AUTO_FILTER = "( +PENDING or +COMPLETED )"
const val AUTO_COMPLETED_SINCE = "today-14d"

/** Taskwarrior's own attributes, which a UDA can't be named. */
private val ATTRIBUTES = (
    "annotations depends description due end entry id imask last mask modified parent priority project recur scheduled start status tags " +
        "until urgency uuid wait"
    ).split(" ").toSet()

/** Why [name] can't be the kanban's UDA, or null. */
fun udaNameProblem(name: String): String? = when {
    name.isEmpty() -> "needs a UDA name"
    !Regex("^[A-Za-z][A-Za-z0-9_]*$").matches(name) -> "a UDA name is letters, digits and _, starting with a letter"
    name in ATTRIBUTES -> "$name is one of Taskwarrior's own attributes"
    else -> null
}

data class UdaStatus(val part: Part, val type: String?, val values: List<String>)

fun udaStatus(config: ConfigResponse?, name: String): UdaStatus {
    val uda = config?.config?.udas?.get(name) ?: return UdaStatus(Part.Missing, null, emptyList())
    // No values allows any; values that leave one of ours out refuse it.
    val fits = uda.type == "string" && (uda.values.isEmpty() || PROGRESS_VALUES.all { it in uda.values })
    return UdaStatus(if (fits) Part.Ok else Part.Different, uda.type, uda.values)
}

/** The `config` lines (name, value) that define the UDA as the kanban needs it, leaving alone what is already so. */
fun kanbanSetupLines(config: ConfigResponse?, name: String): List<Pair<String, String>> {
    val s = udaStatus(config, name)
    if (s.part == Part.Ok) return emptyList()
    val lines = mutableListOf<Pair<String, String>>()
    if (s.type != "string") lines += "uda.$name.type" to "string"
    if (config?.config?.udas?.get(name)?.label.isNullOrEmpty()) lines += "uda.$name.label" to "Progress"
    // Values already there are kept, ours added after them.
    val values = s.values.filter { it.isNotEmpty() } + PROGRESS_VALUES.filter { it !in s.values }
    if (s.values.isNotEmpty() || s.type != "string" || s.part == Part.Missing) lines += "uda.$name.values" to values.joinToString(",")
    return lines
}

/**
 * The report the automatic setup can add to the taskrc, so a kanban needs no filter of its own: open tasks and finished
 * ones (no deleted or waiting tasks, no recurring templates), the most urgent first. Completed since still applies (kanban.ts).
 */
const val KANBAN_REPORT = "kanban"
val KANBAN_REPORT_LINES = listOf(
    "report.$KANBAN_REPORT.description" to "Open tasks and the ones finished lately, for Kanban",
    "report.$KANBAN_REPORT.columns" to "id,priority,project,tags,due.relative,scheduled.countdown,description",
    "report.$KANBAN_REPORT.labels" to "ID,P,Project,Tags,Due,Sch,Description",
    "report.$KANBAN_REPORT.filter" to AUTO_FILTER,
    "report.$KANBAN_REPORT.sort" to "urgency-",
)

/** The lines that add the `kanban` report, or none when the taskrc has a report of that name already (it is used as it is). */
fun kanbanReportLines(config: ConfigResponse?): List<Pair<String, String>> =
    if (config?.config?.reports?.containsKey(KANBAN_REPORT) == true) emptyList() else KANBAN_REPORT_LINES

/** Whether tasksh's review is set up enough for a task to go back in its queue: the `reviewed` date UDA is defined. */
fun hasReview(config: ConfigResponse?): Boolean = config?.config?.udas?.get("reviewed")?.type == "date"

/** The automatic columns, on the UDA [u]. With tasksh's review set up, a task moved to Review goes back in its queue. */
fun automaticColumns(review: Boolean, u: String = PROGRESS_UDA): List<KanbanColumn> = listOf(
    KanbanColumn("To do", "status:pending $u:", "modify status:pending $u:"),
    KanbanColumn("In progress", "status:pending $u:inprogress", "modify status:pending $u:inprogress"),
    KanbanColumn("Review", "status:pending $u:review", "modify status:pending $u:review" + if (review) " reviewed:" else ""),
    KanbanColumn("Done", "status:completed", "done"),
)

/**
 * Kanban as the automatic setup leaves it: on, its settings on the UDA [uda], and a first kanban with none of its own. With
 * [ownReport] it is drawn from the `kanban` report, whose filter is enough; else from `all` with a filter of its own.
 */
fun automaticSettings(s: KanbanSettings, review: Boolean, uda: String, ownReport: Boolean = false) = s.copy(
    enabled = true,
    setup = "automatic",
    uda = uda,
    report = if (ownReport) KANBAN_REPORT else AUTO_REPORT,
    filter = if (ownReport) "" else AUTO_FILTER,
    completedSince = AUTO_COMPLETED_SINCE,
    columns = automaticColumns(review, uda),
    kanbans = s.kanbans.ifEmpty { listOf(KanbanEntry("kanban", "Kanban")) },
)

/** A new kanban named [name], with nothing of its own. */
fun newKanban(name: String, taken: Collection<String>) = KanbanEntry(freshId(name.ifEmpty { "kanban" }, taken), name)

/**
 * [k] renamed to [name]. A kanban not saved yet whose address is still the one its name made follows the new name, as a
 * board's is made from its name; a saved one, or an address typed by hand, stays. [others] are the other kanbans' slugs.
 */
fun renamedKanban(k: KanbanEntry, name: String, saved: Boolean, others: Collection<String>): KanbanEntry {
    val follows = !saved && k.id == newKanban(k.name, others).id
    return k.copy(name = name, id = if (follows) newKanban(name, others).id else k.id)
}

/** Why [slug] can't be the link of a kanban, or null; [others] are the other kanbans' slugs. */
fun kanbanSlugProblem(slug: String, others: Collection<String>): String? = when {
    slug.isEmpty() -> "needs an address"
    !SLUG.matches(slug) -> "an address is lower case letters a to z, digits, - and _, up to 40"
    slug in others -> "there is already a kanban at /kanban/$slug"
    else -> null
}

/** What is wrong with a column, or null. */
fun columnProblem(c: KanbanColumn): String? = when {
    c.name.isBlank() -> "a column needs a name"
    c.filter.isBlank() -> "a column needs a filter"
    c.move.isNotBlank() -> commandProblem(c.move)
    else -> null
}

/** What is wrong with a kanban as it is drawn (not its columns), or null; [others] are the other kanbans' slugs. */
fun kanbanProblem(k: Kanban, others: Collection<String>): String? = when {
    k.name.isBlank() -> "needs a name"
    k.report.isBlank() -> "needs a report, its own or Kanban's"
    else -> kanbanSlugProblem(k.id, others)
}

private fun JsonElement?.str(): String = (this as? JsonPrimitive)?.takeIf { it.isString }?.content ?: ""

/** A text setting: its text, null when it isn't there, and whether it is there but not text. */
private fun JsonObject.text(key: String): Pair<String?, Boolean> {
    val v = this[key] ?: return null to false
    val p = (v as? JsonPrimitive)?.takeIf { it.isString } ?: return null to true
    return p.content to false
}

/** The columns in [v], or what is wrong with the first that can't be used. */
private fun readColumns(v: JsonArray): Pair<List<KanbanColumn>?, String?> {
    val columns = mutableListOf<KanbanColumn>()
    for ((j, c) in v.withIndex()) {
        if (c !is JsonObject) return null to "column ${j + 1} must be an object"
        val col = KanbanColumn(c["name"].str(), c["filter"].str(), c["move"].str())
        columnProblem(col)?.let { return null to "column ${j + 1}${if (col.name.isNotEmpty()) " (${col.name})" else ""}: $it" }
        columns += col
    }
    return columns to null
}

private val TEXTS = listOf("report", "filter", "completedSince")

/**
 * The valid part of a saved `kanban`, and what is wrong with the rest, one line each. A kanban with a problem is left out, and
 * Kanban's columns are none when one of them has one (they stay in the file).
 */
fun parseKanban(v: JsonElement?): Pair<KanbanSettings, List<String>> {
    if (v == null) return KanbanSettings() to emptyList()
    val o = v as? JsonObject ?: return KanbanSettings() to listOf("kanban must be an object: { \"enabled\": true, \"kanbans\": [ … ] }")
    val errors = mutableListOf<String>()
    val enabledValue = o["enabled"]
    val enabled = (enabledValue as? JsonPrimitive)?.takeIf { !it.isString }?.content?.toBooleanStrictOrNull()
    if (enabledValue != null && enabled == null) errors += "kanban.enabled must be true or false"
    val setupText = o["setup"].str()
    val setup = setupText.takeIf { it == "automatic" || it == "manual" }
    if (o["setup"] != null && setup == null) errors += "kanban.setup must be \"automatic\" or \"manual\""
    var base = KanbanSettings(enabled ?: false, setup)
    o["uda"]?.let { u ->
        val name = (u as? JsonPrimitive)?.takeIf { it.isString }?.content
        val bad = if (name == null) "must be a UDA name" else udaNameProblem(name)
        if (bad != null) errors += "kanban.uda: $bad" else base = base.copy(uda = name!!)
    }
    val texts = TEXTS.associateWith { key ->
        val (t, wrong) = o.text(key)
        if (wrong) errors += "kanban.$key must be text"
        t
    }
    base = base.copy(
        report = texts["report"] ?: base.report,
        filter = texts["filter"] ?: base.filter,
        completedSince = texts["completedSince"] ?: base.completedSince,
    )
    (o["columns"] as? JsonArray)?.let { a ->
        val (cols, why) = readColumns(a)
        if (why != null) errors += "kanban.columns: $why" else base = base.copy(columns = cols!!)
    }
    val list = o["kanbans"] ?: return base to errors
    if (list !is JsonArray) return base to errors + "kanban.kanbans must be a list"
    val seen = mutableListOf<String>()
    val kanbans = mutableListOf<KanbanEntry>()
    list.forEachIndexed { i, k ->
        val at = "kanban.kanbans[$i]"
        if (k !is JsonObject) {
            errors += "$at: must be an object"
            return@forEachIndexed
        }
        var one = KanbanEntry(k["id"].str(), k["name"].str())
        val named = at + if (one.id.isNotEmpty()) " (${one.id})" else ""
        val own = mutableMapOf<String, String?>()
        for (key in TEXTS) {
            val (t, wrong) = k.text(key)
            if (wrong) {
                errors += "$named: $key must be text"
                return@forEachIndexed
            }
            own[key] = t
        }
        one = one.copy(report = own["report"], filter = own["filter"], completedSince = own["completedSince"])
        k["columns"]?.let { c ->
            if (c !is JsonArray) {
                errors += "$named: columns must be a list"
                return@forEachIndexed
            }
            val (cols, why) = readColumns(c)
            if (why != null) {
                errors += "$named: $why"
                return@forEachIndexed
            }
            one = one.copy(columns = cols)
        }
        kanbanProblem(resolveKanban(base, one), seen)?.let {
            errors += "$named: $it"
            return@forEachIndexed
        }
        seen += one.id
        kanbans += one
    }
    return base.copy(kanbans = kanbans) to errors
}

private fun columnsJson(columns: List<KanbanColumn>) = JsonArray(
    columns.map { c ->
        buildJsonObject {
            put("name", JsonPrimitive(c.name))
            put("filter", JsonPrimitive(c.filter))
            put("move", JsonPrimitive(c.move))
        }
    },
)

/**
 * The Lucide icon a card shows for one of Taskwarrior's own fields, by the field's name (`due`, whatever the report's
 * format or label), when Appearance's `cardIcons` is on (kanban.ts `FIELD_ICONS`). None for a UDA or a field without one: that is a
 * badge, its label then its value.
 */
val FIELD_ICONS: Map<String, String> = mapOf(
    "id" to "hash", "uuid" to "fingerprint-pattern", "status" to "circle-dot", "priority" to "flag", "project" to "folder",
    "tags" to "tag", "due" to "clock", "scheduled" to "calendar", "wait" to "hourglass", "until" to "calendar-x",
    "entry" to "calendar-plus", "modified" to "pencil", "start" to "play", "end" to "circle-check", "recur" to "repeat",
    "depends" to "link", "urgency" to "zap", "parent" to "git-branch",
)

fun fieldIcon(name: String): String? = FIELD_ICONS[name]

/** How a card shows a field (kanban.ts `cardShape`): one of Taskwarrior's as it is, a UDA as a pill, a free-text one as a paragraph. */
enum class CardShape { Field, Pill, Text }

/** A string UDA with no `values` is free to hold anything, so a paragraph of its own; any other UDA is a pill. */
fun cardShape(name: String, udas: Map<String, UdaDef>): CardShape {
    val def = udas[name] ?: return CardShape.Field
    return if (def.type == "string" && def.values.isEmpty()) CardShape.Text else CardShape.Pill
}

/** `kanban` as it is written to `settings.json`. */
fun kanbanJson(k: KanbanSettings): JsonObject = buildJsonObject {
    put("enabled", JsonPrimitive(k.enabled))
    k.setup?.let { put("setup", JsonPrimitive(it)) }
    put("uda", JsonPrimitive(k.uda))
    put("report", JsonPrimitive(k.report))
    put("filter", JsonPrimitive(k.filter))
    put("completedSince", JsonPrimitive(k.completedSince))
    put("columns", columnsJson(k.columns))
    put(
        "kanbans",
        JsonArray(
            k.kanbans.map { b ->
                buildJsonObject {
                    put("id", JsonPrimitive(b.id))
                    put("name", JsonPrimitive(b.name))
                    b.report?.let { put("report", JsonPrimitive(it)) }
                    b.filter?.let { put("filter", JsonPrimitive(it)) }
                    b.completedSince?.let { put("completedSince", JsonPrimitive(it)) }
                    b.columns?.let { put("columns", columnsJson(it)) }
                }
            },
        ),
    )
}

/** What the screen shows a kanban with: the saved report, filter and Completed since, or what was typed over them. */
data class KanbanView(val report: String, val filter: String, val completedSince: String)

/** The filter every column adds to: the kanban's (or the screen's) filter and Completed since, each in brackets. */
fun baseFilter(v: KanbanView): List<String> = buildList {
    if (v.filter.isNotBlank()) add("( ${v.filter.trim()} )")
    if (v.completedSince.isNotBlank()) add("( -COMPLETED or end.after:${shellQuote(v.completedSince.trim())} )")
}

/**
 * The command line that reads a column (or, with [column] null, every task the kanban's report and filter pick, for Other):
 * the filters, each in brackets so an `or` in one can't spill into another, then the report.
 */
fun columnArgs(v: KanbanView, column: KanbanColumn?): List<String> {
    val parts = baseFilter(v).toMutableList()
    if (column != null && column.filter.isNotBlank()) parts += "( ${column.filter.trim()} )"
    return splitWords(parts.joinToString(" ")) + v.report
}

data class Placed(val columns: List<List<Row>>, val other: List<Row>)

/** Swimlanes from the report's groups, and the lane each task is in (kanban.ts `lanesOf`). */
data class Lanes(val lanes: List<String> = emptyList(), val laneOf: Map<String, Int> = emptyMap())

/**
 * Swimlanes, from the report's own groups (a `/` in its sort, `project+/`): [heads] is the group heading above each of
 * [all]'s rows, in the report's order ([groupHeads]), null where a row continues the group above. A lane for each heading,
 * in the order they first come (a heading met again is the same lane), and the lane each task is in. None without groups.
 */
fun lanesOf(all: List<Row>, heads: List<String?>): Lanes {
    if (heads.isEmpty()) return Lanes()
    val lanes = mutableListOf<String>()
    val laneOf = LinkedHashMap<String, Int>()
    var at = -1
    all.forEachIndexed { i, r ->
        heads.getOrNull(i)?.let { h ->
            at = lanes.indexOf(h)
            if (at < 0) { lanes += h; at = lanes.size - 1 }
        }
        laneOf[r.uuid] = maxOf(at, 0)
    }
    return Lanes(lanes, laneOf)
}

/** Each task in the first column that has it, in the report's order; [all] (every task the kanban picks) less those is Other. */
fun placeTasks(columns: List<List<Row>>, all: List<Row>): Placed {
    val placed = HashSet<String>()
    val out = columns.map { rows -> rows.filter { placed.add(it.uuid) } }
    return Placed(out, all.filter { it.uuid !in placed })
}

/** Which column has the task (`-1`: Other or none). */
fun columnOf(columns: List<List<Row>>, uuid: String): Int = columns.indexOfFirst { rows -> rows.any { it.uuid == uuid } }

/**
 * The statements a move runs on one task: the column's line with the task's uuid in front of each statement, and the task
 * commands in it written out (a task command keeps the uuid in front of each of its own statements).
 */
/** A move changes the card's task only: a recurring task's instance, without asking about the rest of its series. */
const val ONLY_THIS_TASK = "rc.recurrence.confirmation:no"

fun moveSteps(move: String, uuid: String, commands: List<TaskCommand>): List<CommandStep> {
    val line = splitChain(move).mapIndexed { i, s -> (if (i > 0) " ${s.join?.text} " else "") + "$ONLY_THIS_TASK $uuid ${s.line}" }.joinToString("")
    return planChain(line, commands).map { CommandStep(splitWords(it.line), it.join) }
}
