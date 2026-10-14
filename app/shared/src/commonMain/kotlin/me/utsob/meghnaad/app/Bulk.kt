package me.utsob.meghnaad.app

// Pure helpers for the bulk actions: how tasks are named to the engine, and the fields the Modify editor offers (bulk.ts).

/** A task as the bulk actions need it. */
data class Pick(val uuid: String, val id: Int?)

/** Task numbers as a short list (`1,3-5,9`). Taskwarrior reads the same list back, and a long selection stays short enough to see. */
fun idList(ids: List<Int>): String {
    val sorted = ids.toSortedSet().toList()
    val parts = mutableListOf<String>()
    var i = 0
    while (i < sorted.size) {
        var j = i
        while (j + 1 < sorted.size && sorted[j + 1] == sorted[j] + 1) j++
        parts += if (j - i >= 2) "${sorted[i]}-${sorted[j]}" else sorted.subList(i, j + 1).joinToString(",")
        i = j + 1
    }
    return parts.joinToString(",")
}

/**
 * The words that name these tasks at the prompt: their numbers as one list, then the whole uuid of each one without a number (a
 * finished task has none). Side by side they pick all of them, as in Taskwarrior.
 */
fun namedWords(tasks: List<Pick>): List<String> {
    val ids = tasks.mapNotNull { it.id }
    val words = if (ids.isNotEmpty()) mutableListOf(idList(ids)) else mutableListOf()
    for (t in tasks) if (t.id == null) words += t.uuid
    return words
}

/**
 * The filter for a command run by a button. The whole uuid of every task: the numbers can change between the tap and the command
 * (another device finished a task), a uuid cannot.
 */
fun filterWords(tasks: List<Pick>): List<String> = tasks.map { it.uuid }

/** What the prompt starts with after **Command**: the tasks named, and a space to carry on from. */
fun promptFor(tasks: List<Pick>): String = namedWords(tasks).joinToString(" ") + " "

enum class FieldKind { Text, Priority, Date, Project, Tags, Uda }

class BulkField(
    /** What is sent: `project`, `due`, … for a value; `+` and `-` for tags added and taken off. */
    val key: String,
    val label: String,
    val kind: FieldKind,
    val hint: String,
    /** A value is needed (an empty one is not allowed): tags to add or remove. */
    val required: Boolean = false,
    /** Values the field takes, for a choice. */
    val values: List<String>? = null,
)

private const val DATE_HINT = "a date: tomorrow, eow, 2026-12-25, 3d (empty clears it)"

/** The fields the Modify editor offers, then the taskrc's UDAs. */
fun fieldsFor(udas: Map<String, UdaDef>): List<BulkField> {
    val core = listOf(
        BulkField("project", "Project", FieldKind.Project, "a project name (empty clears it)"),
        BulkField("priority", "Priority", FieldKind.Priority, "H, M or L (empty clears it)", values = listOf("H", "M", "L")),
        BulkField("due", "Due", FieldKind.Date, DATE_HINT),
        BulkField("wait", "Wait", FieldKind.Date, DATE_HINT),
        BulkField("scheduled", "Scheduled", FieldKind.Date, DATE_HINT),
        BulkField("until", "Until", FieldKind.Date, DATE_HINT),
        BulkField("recur", "Repeat", FieldKind.Text, "daily, weekly, 3d, … (empty clears it)"),
        BulkField("+", "Add tags", FieldKind.Tags, "tags to add, separated by spaces", required = true),
        BulkField("-", "Remove tags", FieldKind.Tags, "tags to take off, separated by spaces", required = true),
        BulkField("depends", "Depends on", FieldKind.Text, "task numbers or uuids (empty clears them)"),
    )
    val custom = udas.values.map { u ->
        BulkField(
            u.name, u.label?.takeIf { it.isNotEmpty() } ?: u.name, FieldKind.Uda,
            if (u.values.isNotEmpty()) "one of ${u.values.joinToString(", ")} (empty clears it)" else "${u.type} (empty clears it)",
            values = u.values.takeIf { it.isNotEmpty() },
        )
    }
    return core + custom
}

data class ModifyEntry(val key: String, val value: String)

/** Why an entry cannot be applied, or null when it can. */
fun problem(e: ModifyEntry, fields: List<BulkField>): String? {
    val f = fields.firstOrNull { it.key == e.key } ?: return "Choose a field."
    if (f.required && e.value.isBlank()) return "${f.label}: say which."
    return null
}

private fun tagWords(value: String) = value.split(Regex("[\\s,]+")).filter { it.isNotEmpty() }.map { it.replaceFirst(Regex("^[+-]"), "") }

/** The words `modify` takes for these entries, in order. A tag entry makes one word per tag. */
fun modifyWords(entries: List<ModifyEntry>): List<String> = entries.flatMap { e ->
    if (e.key == "+" || e.key == "-") tagWords(e.value).map { "${e.key}$it" } else listOf("${e.key}:${e.value.trim()}")
}

/** What Apply will do, in words: "set project to Home, clear due". */
fun describe(entries: List<ModifyEntry>, fields: List<BulkField>): String {
    fun label(key: String) = fields.firstOrNull { it.key == key }?.label?.lowercase() ?: key
    return entries.joinToString(", ") { e ->
        val v = e.value.trim()
        when {
            e.key == "+" -> "add " + tagWords(v).joinToString(" ") { "+$it" }
            e.key == "-" -> "remove " + tagWords(v).joinToString(" ") { "+$it" }
            v.isNotEmpty() -> "set ${label(e.key)} to $v"
            else -> "clear ${label(e.key)}"
        }
    }
}

/**
 * Tasks to a command at a time: a long selection goes in several commands rather than one too long to take. (The web's Worker
 * takes 200 words; the same bound keeps a command a person can read.)
 */
const val BATCH = 120

fun <T> batches(items: List<T>, size: Int = BATCH): List<List<T>> = items.chunked(size)
