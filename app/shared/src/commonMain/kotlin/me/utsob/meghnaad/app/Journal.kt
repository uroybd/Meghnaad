package me.utsob.meghnaad.app

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.builtins.ListSerializer

// `journal.time` writes an annotation when a task is started or stopped ("Started task", "Stopped task"). Those are
// bookkeeping, not notes: the detail view hides them and shows the sessions they describe instead (web/src/lib/journal.ts).

/** Notes you wrote, without the journal's start and stop markers. */
fun visibleNotes(notes: List<Note>, journal: Journal?): List<Note> =
    if (journal == null) notes else notes.filter { it.text != journal.start && it.text != journal.stop }

/** How many notes are journal markers (for "n hidden" hints). */
fun hiddenCount(notes: List<Note>, journal: Journal?): Int = notes.size - visibleNotes(notes, journal).size

/** A task's tracked work: its sessions (the running one last, with no end) and what they add up to, in seconds. */
@Serializable
data class TimeLog(val sessions: List<Session> = emptyList(), val total: Long = 0)

/**
 * The time log of a task, from the start and stop notes `journal.time` wrote, as of [now] (seconds): the engine's own pairing of them
 * (`tc_core::helpers::time_log`), which the web and the terminal app draw from too (web/src/lib/journal.ts `timeLog`). None without the
 * journal, or if the helper can't be reached.
 */
fun timeLog(annotations: List<Note>, start: Long?, journal: Journal?, now: Long): TimeLog {
    val task = buildJsonObject {
        put("annotations", engineJson.encodeToJsonElement(ListSerializer(Note.serializer()), annotations))
        put("start", start?.let(::JsonPrimitive) ?: JsonNull)
    }
    val markers = journal?.let { engineJson.encodeToJsonElement(Journal.serializer(), it) } ?: JsonNull
    return runCatching { engineJson.decodeFromJsonElement(TimeLog.serializer(), callHelper("time_log", task, markers, JsonPrimitive(now))) }
        .getOrDefault(TimeLog())
}
