package me.utsob.meghnaad.app

// Tab completion for the console: commands and reports, attributes and their modifiers, tags, task ids, and values (projects,
// priorities, UDA values, date words). Everything is built from what the engine has told us, and each option can carry a
// short hint (web/src/lib/completion.ts).

class TaskRef(val id: Int?, val uuid: String, val description: String)

class ReportName(val name: String, val description: String? = null)

class Vocab(
    /** Report names, with their descriptions as hints. */
    val reports: List<ReportName> = emptyList(),
    val projects: List<String> = emptyList(),
    val tags: List<String> = emptyList(),
    val udas: List<UdaDef> = emptyList(),
    val tasks: List<TaskRef> = emptyList(),
    val contexts: List<String> = emptyList(),
)

data class Option(val value: String, val hint: String? = null)

class Completion(
    /** Replace `line[start, end)` with [common] (or with one option's value). */
    val start: Int,
    val end: Int,
    val options: List<Option>,
    /** The longest common prefix of the option values. */
    val common: String,
)

val COMMANDS = linkedMapOf(
    "add" to "create a task",
    "review" to "go through the tasks that need review, tasksh's way: review 10 stops after ten",
    "log" to "record a task that is already done",
    "duplicate" to "copy tasks, with changes: duplicate due:eow",
    "purge" to "remove deleted tasks for good (they must be deleted first)",
    "modify" to "change tasks",
    "done" to "complete tasks",
    "delete" to "delete tasks",
    "start" to "start working on tasks",
    "stop" to "stop working on tasks",
    "annotate" to "add a note",
    "denotate" to "remove a note",
    "append" to "add words to the end of the description",
    "prepend" to "add words to the start of the description",
    "undo" to "revert the last change",
    "info" to "show everything about tasks",
    "count" to "count matching tasks",
    "projects" to "list projects",
    "tags" to "list tags",
    "summary" to "progress of each project",
    "calendar" to "months, with what is due when",
    "calc" to "evaluate an expression: 1 + 2 * 3, 2 days + 3 hours",
    "burndown.daily" to "pending, started and done, by day",
    "burndown.weekly" to "pending, started and done, by week",
    "burndown.monthly" to "pending, started and done, by month",
    "burndown.annual" to "pending, started and done, by year",
    "udas" to "list user defined attributes",
    "columns" to "list report columns",
    "reports" to "list reports",
    "context" to "list, define (define work +work), delete or switch contexts; context none",
    "stats" to "task database statistics",
    "commands" to "every command and report, and what each takes",
    "_get" to "the value of a reference: _get 1.due rc.bulk tw.version",
    "show" to "show every setting, changed ones highlighted",
    "config" to "change a setting: config name value (no value removes it)",
    "information" to "show everything about tasks (same as info)",
    "colors" to "the colour palette; `colors legend` lists yours; `colors bold red on blue` shows a sample",
    "timesheet" to "what was completed and started, by week (the last four weeks)",
    "history.daily" to "tasks added, completed and deleted, by day",
    "history.weekly" to "tasks added, completed and deleted, by week",
    "history.monthly" to "tasks added, completed and deleted, by month",
    "history.annual" to "tasks added, completed and deleted, by year",
    "ghistory.daily" to "the same as a graph, by day",
    "ghistory.weekly" to "the same as a graph, by week",
    "ghistory.monthly" to "the same as a graph, by month",
    "ghistory.annual" to "the same as a graph, by year",
    "export" to "tasks as a JSON file to download (what task import reads)",
    "import" to "tasks from a JSON file (what export writes): choose the file on the page",
    "ids" to "ids of matching tasks",
    "uuids" to "uuids of matching tasks",
    "help" to "how to use this",
)

private val ATTRS = linkedMapOf(
    "project" to "project name",
    "priority" to "H, M or L",
    "due" to "due date",
    "wait" to "hide until",
    "scheduled" to "scheduled date",
    "until" to "expires",
    "recur" to "repeat period: daily, weekly, 3d…",
    "start" to "when started",
    "end" to "when finished",
    "entry" to "when created",
    "depends" to "tasks this depends on",
    "description" to "the text",
    "status" to "pending, completed, deleted…",
    "urgency" to "computed score",
    "tags" to "tag list",
    "limit" to "page, none or a number",
)

private val DATE_ATTRS = setOf("due", "wait", "scheduled", "until", "start", "end", "entry", "modified")
private val NUMERIC_ATTRS = setOf("urgency", "id")

private val DATE_WORDS = listOf(
    Option("now"), Option("today"), Option("tomorrow"), Option("yesterday"),
    Option("sod", "start of day"), Option("eod", "end of day"), Option("sow", "start of week"), Option("eow", "end of week"),
    Option("som", "start of month"), Option("eom", "end of month"), Option("soy", "start of year"), Option("eoy", "end of year"),
    Option("monday"), Option("tuesday"), Option("wednesday"), Option("thursday"), Option("friday"), Option("saturday"),
    Option("sunday"),
    Option("1d", "in a day"), Option("3d", "in 3 days"), Option("1w", "in a week"), Option("2w", "in 2 weeks"),
    Option("1mo", "in a month"),
)

val VIRTUAL_TAGS = listOf(
    "ACTIVE", "ANNOTATED", "BLOCKED", "BLOCKING", "CHILD", "COMPLETED", "DELETED", "DUE", "DUETODAY", "INSTANCE", "LATEST",
    "MONTH", "ORPHAN", "OVERDUE", "PARENT", "PENDING", "PRIORITY", "PROJECT", "QUARTER", "READY", "SCHEDULED", "TAGGED",
    "TEMPLATE", "TODAY", "TOMORROW", "UDA", "UNBLOCKED", "UNTIL", "WAITING", "WEEK", "YEAR", "YESTERDAY",
)

private val MOD_HINTS = mapOf(
    "is" to "exactly", "isnt" to "not exactly", "not" to "not (prefix)", "has" to "contains", "hasnt" to "does not contain",
    "startswith" to "starts with", "endswith" to "ends with", "word" to "as a whole word", "noword" to "not as a word",
    "before" to "earlier than", "after" to "later than", "by" to "at or before", "none" to "is unset", "any" to "is set",
)
private val MODS_STRING = listOf("is", "isnt", "not", "has", "hasnt", "startswith", "endswith", "word", "noword", "none", "any")
private val MODS_DATE = listOf("before", "after", "by", "is", "isnt", "not", "none", "any")
private val MODS_NUMBER = listOf("before", "after", "by", "is", "isnt", "not", "none", "any")

private const val MAX_OPTIONS = 40

private fun commonPrefix(xs: List<String>): String {
    if (xs.isEmpty()) return ""
    var p = xs[0]
    for (x in xs) while (!x.startsWith(p)) p = p.dropLast(1)
    return p
}

private fun matching(stem: String, pool: List<Option>, ci: Boolean = false): List<Option> {
    val s = if (ci) stem.lowercase() else stem
    val seen = HashSet<String>()
    return pool
        .filter { (if (ci) it.value.lowercase() else it.value).startsWith(s) }
        .filter { seen.add(it.value) }
        .sortedBy { it.value }
        .take(MAX_OPTIONS)
}

private fun opt(values: List<String>, hint: String? = null) = values.map { Option(it, hint) }

private enum class AttrKind { Date, Number, Text }

private fun attrKind(name: String, v: Vocab): AttrKind {
    if (name in DATE_ATTRS) return AttrKind.Date
    if (name in NUMERIC_ATTRS) return AttrKind.Number
    val u = v.udas.firstOrNull { it.name == name }
    if (u?.type == "date") return AttrKind.Date
    if (u?.type == "numeric") return AttrKind.Number
    return AttrKind.Text
}

private fun taskOptions(v: Vocab): List<Option> = v.tasks.flatMap { t ->
    listOfNotNull(t.id?.let { Option(it.toString(), t.description) }) + Option(t.uuid.take(8), t.description)
}

fun complete(line: String, caret: Int, v: Vocab): Completion {
    val before = line.substring(0, caret)
    val start = before.lastIndexOf(' ') + 1
    val word = before.substring(start)
    val none = Completion(start, caret, emptyList(), word)
    if (word.isEmpty()) return none

    fun done(prefix: String, options: List<Option>): Completion {
        val full = options.map { it.copy(value = prefix + it.value) }
        return Completion(start, caret, full, commonPrefix(full.map { it.value }).ifEmpty { word })
    }

    // +tag / -tag, including virtual tags.
    if (word[0] == '+' || word[0] == '-') {
        val pool = opt(v.tags) + opt(VIRTUAL_TAGS, "virtual")
        return done(word[0].toString(), matching(word.drop(1), pool, true))
    }

    // rc.report.<name>.<setting>:<value> and rc.context:<name> (per-command overrides).
    if (word.startsWith("rc.")) return completeOverride(word, start, caret, v)

    // A bare number is a task id; list the ids whose digits start with it.
    if (Regex("^\\d+$").matches(word)) {
        val ids = taskOptions(v).filter { Regex("^\\d+$").matches(it.value) }
        return done("", matching(word, ids))
    }

    val sep = word.indexOfFirst { it == ':' || it == '=' }
    if (sep > 0) {
        // name[.modifier]:value
        val lhs = word.substring(0, sep + 1)
        val stem = word.substring(sep + 1)
        val name = lhs.dropLast(1).substringBefore('.')
        val uda = v.udas.firstOrNull { it.name == name }
        var pool: List<Option> = emptyList()
        when {
            name == "project" -> pool = opt(v.projects)
            name == "priority" -> pool = opt(listOf("H", "M", "L"))
            name == "status" -> pool = opt(listOf("pending", "completed", "deleted", "recurring", "waiting"))
            name == "recur" -> pool = RECUR_WORDS.map { Option(it.value, it.hint) }
            name == "limit" -> pool = listOf(Option("page"), Option("none"), Option("10"), Option("25"))
            name == "depends" -> {
                // A comma list: complete the last item, keep the rest.
                val lastComma = stem.lastIndexOf(',')
                val kept = lhs + stem.substring(0, lastComma + 1)
                return done(kept, matching(stem.substring(lastComma + 1).replace(Regex("^[-+]"), ""), taskOptions(v)))
            }
            name in DATE_ATTRS || uda?.type == "date" -> pool = DATE_WORDS
            uda != null && uda.values.isNotEmpty() -> pool = opt(uda.values.filter { it.isNotEmpty() })
        }
        return done(lhs, matching(stem, pool))
    }

    // name.modifier (no colon yet): offer the modifiers that make sense for the attribute.
    val dot = word.indexOf('.')
    if (dot > 0) {
        val name = word.substring(0, dot)
        val known = name in ATTRS || v.udas.any { it.name == name }
        if (known) {
            val mods = when (attrKind(name, v)) {
                AttrKind.Date -> MODS_DATE
                AttrKind.Number -> MODS_NUMBER
                AttrKind.Text -> MODS_STRING
            }
            val pool = mods.map { Option("$it:", MOD_HINTS[it]) }
            return done("$name.", matching(word.substring(dot + 1), pool))
        }
        return none
    }

    // Command, report, or attribute name.
    val pool = COMMANDS.map { (value, hint) -> Option(value, hint) } +
        v.reports.map { Option(it.name, it.description ?: "report") } +
        ATTRS.map { (n, hint) -> Option("$n:", hint) } +
        v.udas.map { Option("${it.name}:", it.label ?: "UDA") }
    val options = matching(word, pool)
    return Completion(start, caret, options, commonPrefix(options.map { it.value }).ifEmpty { word })
}

private val REPORT_SETTINGS = linkedMapOf(
    "sort" to "sort order, e.g. due+,priority-",
    "filter" to "extra filter for this report",
    "columns" to "columns to show",
    "labels" to "column headings",
)

private fun completeOverride(word: String, start: Int, caret: Int, v: Vocab): Completion {
    fun make(prefix: String, options: List<Option>): Completion {
        val full = options.map { it.copy(value = prefix + it.value) }
        return Completion(start, caret, full, commonPrefix(full.map { it.value }).ifEmpty { word })
    }
    val rest = word.drop(3) // after "rc."

    Regex("^context[:=](.*)$").find(rest)?.let { m ->
        val arg = m.groupValues[1]
        return make(word.dropLast(arg.length), matching(arg, opt(v.contexts, "context")))
    }

    // rc.report.<name>.sort:<keys>: complete the last key, e.g. `du` -> `due+` / `due-`.
    Regex("^report\\.([^.:=]+)\\.sort[:=](.*)$").find(rest)?.let { m ->
        val keys = m.groupValues[2]
        val stem = keys.substring(keys.lastIndexOf(',') + 1)
        val prefix = word.dropLast(stem.length)
        val cols = SORTABLE.toList() + v.udas.map { it.name }
        val pool = cols.flatMap { c -> listOf(Option("$c+", "ascending"), Option("$c-", "descending")) }
        return make(prefix, matching(stem, pool))
    }

    // rc.report.<name>.<setting>
    Regex("^report\\.([^.:=]+)\\.([a-z]*)$").find(rest)?.let { m ->
        val pool = REPORT_SETTINGS.map { (k, hint) -> Option("$k:", hint) }
        return make("rc.report.${m.groupValues[1]}.", matching(m.groupValues[2], pool))
    }

    // rc.report.<name>
    Regex("^report\\.([^.:=]*)$").find(rest)?.let { m ->
        val pool = v.reports.map { Option("${it.name}.", it.description ?: "report") }
        return make("rc.report.", matching(m.groupValues[1], pool))
    }

    // rc.<what>
    val pool = listOf(Option("report.", "change a report for this command"), Option("context:", "use a context for this command"))
    return make("rc.", matching(rest, pool))
}

class Inserted(val line: String, val caret: Int)

/** Insert [value] in place of the completed word; add a space unless it ends mid-token. */
fun insert(line: String, c: Completion, value: String, space: Boolean = true): Inserted {
    // No space after a separator, or after a sort direction (`due+`), which a `,` usually follows.
    val tail = if (space && !Regex("[:=.,+-]$").containsMatchIn(value)) " " else ""
    val next = line.substring(0, c.start) + value + tail + line.substring(c.end)
    return Inserted(next, c.start + value.length + tail.length)
}

class Applied(val line: String, val caret: Int, val progressed: Boolean)

/**
 * Tab behaviour: complete the shared prefix; when that makes no progress, the caller opens a menu of the options. A single
 * option is completed in full (plus a space).
 */
fun apply(line: String, c: Completion): Applied {
    if (c.options.isEmpty()) return Applied(line, c.end, false)
    val typed = line.substring(c.start, c.end)
    val unique = c.options.size == 1
    val r = insert(line, c, c.common, unique)
    return Applied(r.line, r.caret, unique || c.common.length > typed.length)
}
