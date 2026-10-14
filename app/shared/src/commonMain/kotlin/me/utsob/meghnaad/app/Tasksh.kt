package me.utsob.meghnaad.app

// What tasksh's `review` needs in the taskrc: a `reviewed` date UDA and a `_reviewed` report that lists the tasks due for
// review (web/src/lib/tasksh.ts). tasksh offers to write both the first time it runs; the same lines are written from here.
// (src/review.cpp in https://github.com/GothenburgBitFactory/taskshell)

const val REVIEW_REPORT = "_reviewed"
const val REVIEW_DESCRIPTION = "Tasksh review report.  Adjust the filter to your needs."
const val REVIEW_SORT = "reviewed+,modified+"
const val REVIEW_FILTER = "( reviewed.none: or reviewed.before:now-6days ) and ( +PENDING or +WAITING )"

/** `Ok`: as tasksh defines it. `Different`: there, but not the way tasksh needs it. */
enum class Part { Ok, Missing, Different }

class TaskshStatus(
    val uda: Part,
    /** The `reviewed` UDA's type, when it is defined. */
    val udaType: String?,
    val report: Part,
    /** The columns the `_reviewed` report has now (tasksh reads the uuid of each row). */
    val columns: List<String>,
    /** The saved `_reviewed` filter and sort, when the report is defined. */
    val filter: String?,
    val sort: String?,
) {
    /** Both parts are as tasksh needs them: `review` can run. */
    val enabled get() = uda == Part.Ok && report == Part.Ok
}

fun taskshStatus(config: ConfigResponse?): TaskshStatus {
    val uda = config?.config?.udas?.get("reviewed")
    val report = config?.reports?.firstOrNull { it.name == REVIEW_REPORT }
    val columns = report?.columns?.map { it.spec } ?: emptyList()
    val udaPart = when {
        uda == null -> Part.Missing
        uda.type == "date" -> Part.Ok
        else -> Part.Different
    }
    val reportPart = when {
        report == null -> Part.Missing
        columns == listOf("uuid") -> Part.Ok
        else -> Part.Different
    }
    return TaskshStatus(udaPart, uda?.type, reportPart, columns, report?.filter, report?.sort)
}

/** The part of the report you can adjust: which tasks are due, and in what order. */
data class ReviewWanted(val filter: String, val sort: String)

/**
 * The `config` lines (name, value) that bring the taskrc to what is wanted, leaving alone what is already so. `columns` goes
 * before the rest of the report: a report without columns is not kept, so a description sent first would be lost.
 */
fun setupLines(config: ConfigResponse?, want: ReviewWanted): List<Pair<String, String>> {
    val s = taskshStatus(config)
    val lines = mutableListOf<Pair<String, String>>()
    if (s.uda != Part.Ok) lines += "uda.reviewed.type" to "date"
    if (s.uda == Part.Missing || config?.config?.udas?.get("reviewed")?.label.isNullOrEmpty()) lines += "uda.reviewed.label" to "Reviewed"
    if (s.report != Part.Ok) lines += "report.$REVIEW_REPORT.columns" to "uuid"
    if (s.report == Part.Missing) lines += "report.$REVIEW_REPORT.description" to REVIEW_DESCRIPTION
    if (s.report != Part.Ok || s.sort != want.sort) lines += "report.$REVIEW_REPORT.sort" to want.sort
    if (s.report != Part.Ok || s.filter != want.filter) lines += "report.$REVIEW_REPORT.filter" to want.filter
    return lines
}

/** One `config` line as the arguments to send: no questions asked, as tasksh runs it. */
fun configArgs(line: Pair<String, String>) = listOf("rc.confirmation:no", "rc.verbose:nothing", "config", line.first, line.second)

/**
 * `review` or `review 10` (to stop after ten), as tasksh reads it: the word may be cut to three letters or more, and `task` may
 * come before it. Anything else is not the review command and goes to Taskwarrior as it was typed. The limit, or null.
 */
fun parseReview(line: String): Int? {
    val words = splitWords(line.trim()).toMutableList()
    if (words.firstOrNull() == "task") words.removeAt(0)
    val cmd = words.getOrNull(0)
    val count = words.getOrNull(1)
    if (cmd == null || cmd.length < 3 || !"review".startsWith(cmd.lowercase()) || words.size > 2) return null
    if (count == null) return 0
    return if (count.isNotEmpty() && count.all { it in '0'..'9' }) count.toIntOrNull() else null
}

enum class ReviewKey { Reviewed, Skip, Edit, Modify, Complete, Delete, Quit }

/** What an answer at the review menu means: Enter or `r` mark it reviewed; null is a word the menu does not know. */
fun menuKey(input: String): ReviewKey? = when (input.trim().lowercase()) {
    "", "r" -> ReviewKey.Reviewed
    "s" -> ReviewKey.Skip
    "e" -> ReviewKey.Edit
    "m" -> ReviewKey.Modify
    "c" -> ReviewKey.Complete
    "d" -> ReviewKey.Delete
    "q" -> ReviewKey.Quit
    else -> null
}

// tasksh's own words (src/review.cpp), so a session reads the same here.
const val REVIEW_MENU = "(Enter) Mark as reviewed, (s)kip, (e)dit, (m)odify, (c)omplete, (d)elete, (q)uit"
const val REVIEW_MODIFY_PROMPT = "Enter modification args [example: +tag -tag /teh/the/ project:X]"
const val REVIEW_NOTHING = "There are no tasks needing review."
const val REVIEW_INTRO =
    "The review process is important for keeping your list accurate, so you are working on the right tasks. " +
        "For each task you are shown, look at the metadata. Determine whether the task needs to be changed (enter 'e' to " +
        "edit), or whether it is accurate ('enter' or 'r' to mark as reviewed). You may skip a task ('s') but a skipped " +
        "task is not considered reviewed. You may stop at any time, and resume later right where you left off."

fun reviewEnd(reviewed: Int, total: Int) = "End of review. $reviewed out of $total tasks reviewed."
fun notRecognized(word: String) = "Command '$word' is not recognized."

enum class SetupPart(val prefix: String) { Uda("uda.reviewed."), Report("report.$REVIEW_REPORT.") }

/** The question before defining one part, in tasksh's words (and what is there now, where tasksh would say nothing). */
fun setupQuestion(part: SetupPart, s: TaskshStatus): String = when (part) {
    SetupPart.Uda ->
        if (s.uda == Part.Missing) "Tasksh needs to define a 'reviewed' UDA of type 'date' for all tasks.  Ok to proceed?"
        else "The 'reviewed' UDA is of type '${s.udaType}' now. Tasksh needs it to be of type 'date'.  Ok to change it?"
    SetupPart.Report ->
        if (s.report == Part.Missing) "Tasksh needs to define a '_reviewed' report to identify tasks needing review.  Ok to proceed?"
        else "The '_reviewed' report has the columns '${s.columns.joinToString(", ")}' now. Tasksh needs it to have only 'uuid'.  Ok to replace them?"
}

/** The `config` lines for one part of the setup. */
fun partLines(lines: List<Pair<String, String>>, part: SetupPart) = lines.filter { it.first.startsWith(part.prefix) }
