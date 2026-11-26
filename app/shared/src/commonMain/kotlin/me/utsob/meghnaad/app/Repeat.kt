package me.utsob.meghnaad.app

import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp

// How a task repeats, as the task form offers it: not at all, as an iterative task (Taskwarrior 3.6.0's `iter`), or as a
// recurring one (`recur`). One mode at a time: switching clears the other mode's fields in the same `modify`, which `task`
// takes (`iter: iter_type: recur:weekly` ends a series and makes the task the template of a recurring one). Who wants to
// keep them has the command line. The web's is web/src/lib/repeat.ts.

enum class RepeatMode(val label: String) {
    None("Doesn't repeat"),
    Iterative("Iterative"),
    Recurring("Recurring"),
}

/** The fields of both modes. */
data class RepeatFields(val recur: String, val iter: String, val iterType: String)

/** The mode a task's fields say it is in: a `recur` is recurring (a task can't be both), an `iter` iterative. */
fun modeOf(f: RepeatFields): RepeatMode = when {
    f.recur.isNotBlank() -> RepeatMode.Recurring
    f.iter.isNotBlank() -> RepeatMode.Iterative
    else -> RepeatMode.None
}

/**
 * The words of an `add` or `modify` that leave the task in [mode] with the fields [now], given what it had ([was]; all
 * empty for a new task): the chosen mode's fields where they changed, and the other modes' fields cleared where it had
 * them. A mode chosen and not filled in yet (no period, no rule) changes nothing, the others' fields included.
 */
fun repeatChanges(mode: RepeatMode, now: RepeatFields, was: RepeatFields): List<String> {
    if (mode == RepeatMode.Iterative && now.iter.isBlank()) return emptyList()
    if (mode == RepeatMode.Recurring && now.recur.isBlank()) return emptyList()
    val out = mutableListOf<String>()
    fun clear(name: String, had: String) {
        if (had.isNotBlank()) out += "$name:"
    }
    if (mode != RepeatMode.Iterative) {
        clear("iter", was.iter)
        clear("iter_type", was.iterType)
    }
    if (mode != RepeatMode.Recurring) clear("recur", was.recur)
    if (mode == RepeatMode.Iterative) {
        val iter = now.iter.trim()
        if (iter.isNotEmpty() && iter != was.iter.trim()) out += "iter:$iter"
        if (iter.isNotEmpty() && now.iterType.isNotEmpty() && now.iterType != was.iterType) out += "iter_type:${now.iterType}"
    }
    if (mode == RepeatMode.Recurring) {
        val recur = now.recur.trim()
        if (recur.isNotEmpty() && recur != was.recur.trim()) out += "recur:$recur"
    }
    return out
}

/** The mark after a repeating task's title: its Lucide icon and what it says, for a screen reader. */
data class RepeatMarkOf(val icon: String, val words: String)

/**
 * `repeat` for a recurring task (its template or one of its instances), `iteration-cw` for an iterative one (done, it
 * makes the next); none for a task that doesn't repeat. As the web app's `RepeatMark.svelte`.
 */
fun repeatMark(row: Row): RepeatMarkOf? = when {
    row.recur != null ->
        RepeatMarkOf("repeat", if (row.status == "recurring") "Recurring task: repeats ${row.recur}" else "Repeats ${row.recur}")
    row.status == "iterative" && !row.iter.isNullOrEmpty() ->
        RepeatMarkOf("iteration-cw", "Iterative: done, it makes the next (${row.iter})" + (row.iterCount?.let { "; number $it" } ?: ""))
    else -> null
}

/** The mark itself, small and dim, after the title in a report row or a card. */
@androidx.compose.runtime.Composable
fun RepeatMark(row: Row, tint: androidx.compose.ui.graphics.Color) {
    val m = repeatMark(row) ?: return
    LucideIcon(
        m.icon,
        androidx.compose.ui.Modifier.semantics { contentDescription = m.words },
        size = 12.dp,
        tint = tint,
    )
}
