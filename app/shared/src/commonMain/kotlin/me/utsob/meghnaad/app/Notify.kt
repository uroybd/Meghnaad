package me.utsob.meghnaad.app

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// When should the app tell you something? Pure logic (web/src/lib/notify.ts), so it can be tested. Each notice is identified by a
// key (task + kind + the time it is about), and is shown at most once, so asking again and restarting never repeat it.

/**
 * What the reminders and the timer read of a task, which is all that `_remind` sends (a full row costs a list of virtual tags, an
 * urgency and a colour for every task).
 */
@Serializable
data class Reminder(
    val uuid: String,
    val status: String,
    val description: String,
    val due: Long? = null,
    val wait: Long? = null,
    val scheduled: Long? = null,
    val start: Long? = null,
    @SerialName("active_seconds") val activeSeconds: Long? = null,
    /** The task's `remind_in` UDA: times before `due` and `scheduled` to remind, as typed (`10,1h`). */
    @SerialName("remind_in") val remindIn: String? = null,
)

@Serializable
data class NotifySettings(
    val enabled: Boolean = false,
    /** Minutes before a timed due or scheduled date to give a heads-up (0 = none), for a task without `remind_in`. */
    val leadMinutes: Int = 15,
    /** Local hour of day to notify about tasks due on a date with no time (all-day). */
    val allDayHour: Int = 9,
    /** Notify when a task's `wait` date passes and it reappears. */
    val waitOver: Boolean = true,
)

enum class NoticeKind { Soon, Scheduled, Due, AllDay, Wait, Digest }

class Notice(
    /** Dedup key. */
    val key: String,
    val kind: NoticeKind,
    val title: String,
    val body: String,
    /** Task to open when tapped (none for a digest). */
    val uuid: String?,
)

private const val HOUR = 3600L
private const val DAY = 86400L

/** Don't announce "due now" for something that became due long ago; the digest covers those. */
private const val FRESH = HOUR

private fun localDay(ts: Long, tz: Int) = Math.floorDiv(ts + tz, DAY)

fun isAllDay(due: Long, tz: Int) = Math.floorMod(due + tz, DAY) == 0L

private fun until(seconds: Long): String {
    val m = Math.round(seconds / 60.0)
    return if (m <= 1) "in a minute" else if (m < 60) "in $m minutes" else "in ${Math.round(m / 60.0)} hours"
}

/**
 * The times before `due` and `scheduled` a task's `remind_in` asks for, in seconds, latest first (the engine reads it:
 * minutes, or Taskwarrior durations; `tc_core::helpers::remind_offsets`). Null means the setting applies; an empty list, no
 * reminders before those dates.
 */
fun remindOffsets(value: String?): List<Long>? {
    if (value == null) return null
    val out = callHelper("remind_offsets", kotlinx.serialization.json.JsonPrimitive(value))
    return (out as? kotlinx.serialization.json.JsonArray)?.map { (it as kotlinx.serialization.json.JsonPrimitive).content.toLong() }
}

/** Of the moments [offsets] before [at], the closest one to [at] that has come (as its offset), or null. */
fun reached(offsets: List<Long>, at: Long, now: Long): Long? {
    if (now >= at) return null
    return offsets.filter { now >= at - it }.minOrNull()
}

/** "today", "tomorrow" or "in 3 days" for a date with no time. */
private fun dayWord(date: Long, now: Long, tz: Int): String {
    val d = localDay(date, tz) - localDay(now, tz)
    return if (d <= 0) "today" else if (d == 1L) "tomorrow" else "in $d days"
}

/** The task's own offsets, or the setting's lead, and whether they are the task's own. */
private fun offsetsFor(r: Reminder, s: NotifySettings, offsetsOf: (String?) -> List<Long>?): Pair<List<Long>, Boolean> {
    val own = offsetsOf(r.remindIn)
    return (own ?: if (s.leadMinutes > 0) listOf(s.leadMinutes * 60L) else emptyList()) to (own != null)
}

/**
 * Where the offsets of a date count back from: the date itself, or the hour for a date with no time, and then only with the
 * task's own times (that hour's notice is the setting's). Null when there is nothing to count back from.
 */
private fun countFrom(date: Long, own: Boolean, s: NotifySettings, tz: Int): Long? =
    if (!isAllDay(date, tz)) date else if (own) date + s.allDayHour * HOUR else null

/**
 * Notices that should be shown right now.
 * [rows] are pending tasks (anything else is ignored); [seen] the keys already shown (the caller adds the keys of what it shows);
 * [tz] is the viewer's UTC offset in seconds.
 */
fun dueNotices(
    rows: List<Reminder>,
    now: Long,
    s: NotifySettings,
    seen: Set<String>,
    tz: Int,
    offsetsOf: (String?) -> List<Long>? = ::remindOffsets,
): List<Notice> {
    val out = mutableListOf<Notice>()
    fun add(n: Notice) {
        if (n.key !in seen) out.add(n)
    }
    var overdue = 0

    for (r in rows) {
        if (r.status != "pending") continue
        val wait = r.wait
        val waiting = wait != null && wait > now
        // The task's own times before its dates, or the setting's lead.
        val (offsets, own) = offsetsFor(r, s, offsetsOf)
        fun before(kind: NoticeKind, word: String, date: Long) {
            val at = countFrom(date, own, s, tz) ?: return
            val o = reached(offsets, at, now) ?: return
            val title = "$word ${if (isAllDay(date, tz)) dayWord(date, now, tz) else until(date - now)}"
            add(Notice("${r.uuid}:${kind.name.lowercase()}:$date:$o", kind, title, r.description, r.uuid))
        }
        if (!waiting) {
            r.due?.let { before(NoticeKind.Soon, "Due", it) }
            r.scheduled?.let { before(NoticeKind.Scheduled, "Scheduled", it) }
        }
        val due = r.due
        if (due != null && !waiting) {
            if (isAllDay(due, tz)) {
                // A date without a time: remind at the chosen hour on that day, not at midnight.
                val at = due + s.allDayHour * HOUR
                if (now >= at && now < due + DAY) add(Notice("${r.uuid}:allday:$due", NoticeKind.AllDay, "Due today", r.description, r.uuid))
                if (now >= due + DAY) overdue++
            } else {
                if (now >= due && now < due + FRESH) add(Notice("${r.uuid}:due:$due", NoticeKind.Due, "Due now", r.description, r.uuid))
                if (now >= due + FRESH) overdue++
            }
        }
        if (s.waitOver && wait != null && now >= wait && now < wait + FRESH) {
            add(Notice("${r.uuid}:wait:$wait", NoticeKind.Wait, "Back on your list", r.description, r.uuid))
        }
    }

    // One digest per day for what is already overdue, instead of a notification per task.
    if (overdue > 0) {
        add(Notice("digest:${localDay(now, tz)}", NoticeKind.Digest, "$overdue overdue ${if (overdue == 1) "task" else "tasks"}", "Open the list to catch up.", null))
    }
    return out
}

/**
 * What is shown by an alarm, with the app closed: the same notices, except that the overdue digest waits for the chosen hour of
 * the day (an open page shows it as soon as it is looked at; nobody wants the phone to buzz at midnight).
 */
fun noticesToPost(
    rows: List<Reminder>,
    now: Long,
    s: NotifySettings,
    seen: Set<String>,
    tz: Int,
    offsetsOf: (String?) -> List<Long>? = ::remindOffsets,
): List<Notice> {
    val hour = Math.floorMod(now + tz, DAY) / HOUR
    return dueNotices(rows, now, s, seen, tz, offsetsOf).filter { it.kind != NoticeKind.Digest || hour >= s.allDayHour }
}

/**
 * The soonest moment after [now] at which [noticesToPost] has something new to say, or null when nothing ever will (until the
 * tasks change): when to set the alarm for.
 */
fun nextTrigger(rows: List<Reminder>, now: Long, s: NotifySettings, seen: Set<String>, tz: Int): Long? {
    // The task's own times are read once here, not again for every moment tried below.
    val read = rows.mapNotNull { it.remindIn }.distinct().associateWith(::remindOffsets)
    val offsetsOf = { v: String? -> v?.let { read[it] } }
    val moments = sortedSetOf<Long>()
    for (r in rows) {
        if (r.status != "pending") continue
        val (offsets, own) = offsetsFor(r, s, offsetsOf)
        for (date in listOfNotNull(r.due, r.scheduled)) {
            countFrom(date, own, s, tz)?.let { at -> offsets.forEach { moments.add(at - it) } }
        }
        r.due?.let { due -> moments.add(if (isAllDay(due, tz)) due + s.allDayHour * HOUR else due) }
        r.wait?.let { moments.add(it) }
    }
    // The overdue digest, at the chosen hour on this day and the next.
    val day = localDay(now, tz)
    for (k in 0..1) moments.add((day + k) * DAY - tz + s.allDayHour * HOUR)
    return moments.firstOrNull { it > now && noticesToPost(rows, it, s, seen, tz, offsetsOf).isNotEmpty() }
}

/** Drop dedup keys older than [maxAge] so the stored set doesn't grow forever. */
fun prune(seen: Map<String, Long>, now: Long, maxAge: Long = 3 * DAY): Map<String, Long> = seen.filterValues { now - it < maxAge }

/**
 * The tasks whose notifications should be taken away, of those [shown]: each that is no longer pending or waiting among [rows]
 * (done or deleted, here or on another device, or left with no date to remind of).
 */
fun noticesToClear(shown: Collection<String>, rows: List<Reminder>): Set<String> {
    val open = rows.filter { it.status == "pending" || it.status == "waiting" }.map { it.uuid }.toSet()
    return shown.filterNot { it in open }.toSet()
}
