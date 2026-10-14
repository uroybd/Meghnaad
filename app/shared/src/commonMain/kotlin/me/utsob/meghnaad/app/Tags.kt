package me.utsob.meghnaad.app

// Grouping tasks by tag for the Tags page. A task with two tags appears under both (web/src/lib/tags.ts).

class TagEntry(
    val name: String,
    /** The tasks with this tag, most urgent first. */
    val tasks: MutableList<Row> = mutableListOf(),
    /** Of those, how many are pending and past due. */
    var overdue: Int = 0,
)

/**
 * One entry per tag, by name, from the tasks given (the page asks for those its filter matches, finished ones included when the
 * filter allows them, so a tag stays listed after its last task is done). [extra] is a tag to list even when no task has it (a
 * chip tapped in a report of finished tasks), so that showing "its entry" always has an entry to show.
 */
fun buildTags(rows: List<Row>, now: Long, extra: String? = null): List<TagEntry> {
    val by = linkedMapOf<String, TagEntry>()
    fun entry(name: String) = by.getOrPut(name) { TagEntry(name) }
    for (r in rows) {
        for (t in r.tags) {
            val e = entry(t)
            e.tasks += r
            if (r.status == "pending" && r.due != null && r.due < now) e.overdue++
        }
    }
    if (extra != null) entry(extra)
    for (e in by.values) e.tasks.sortByDescending { it.urgency }
    return by.values.sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.name })
}
