package me.utsob.meghnaad.app

// The sentences of `task info`'s change history, as Taskwarrior words them (CmdInfo.cpp): web/src/lib/history.ts.

private fun ucFirst(s: String) = s.replaceFirstChar { it.uppercase() }

/** Date properties hold epoch seconds; anything that is not a number is shown as it is. */
private fun value(c: HistoryChange, v: String?, show: (Long) -> String): String {
    if (v == null) return ""
    val n = Regex("^\\s*[-+]?\\d+").find(v)?.value?.trim()?.toLongOrNull() ?: 0L
    return if (c.date && v != "" && n != 0L) show(n) else v
}

/** One change as a sentence. [show] renders epoch seconds in the user's date format. */
fun describeChange(c: HistoryChange, show: (Long) -> String): String {
    val prop = ucFirst(c.prop)
    return when (c.kind) {
        "set" -> "$prop set to '${value(c, c.value, show)}'."
        "changed" -> "$prop changed from '${value(c, c.old, show)}' to '${value(c, c.value, show)}'."
        "deleted" -> if (c.duration != null) "$prop deleted (duration: ${c.duration})." else "$prop deleted."
        "note_added" -> "Annotation of '${c.value}' added."
        "note_changed" -> "Annotation changed to '${c.value}'."
        "note_deleted" -> "Annotation '${c.old}' deleted."
        "tag_added" -> "Tag '${c.prop}' added."
        "tag_deleted" -> "Tag '${c.prop}' deleted."
        "dep_added" -> "Dependency on '${c.prop}' added."
        "dep_deleted" -> "Dependency on '${c.prop}' deleted."
        else -> "$prop ${c.kind}."
    }
}
