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
    val start: Long? = null,
    @SerialName("active_seconds") val activeSeconds: Long? = null,
)

@Serializable
data class NotifySettings(
    val enabled: Boolean = false,
    /** Minutes before a timed due date to give a heads-up (0 = none). */
    val leadMinutes: Int = 15,
    /** Local hour of day to notify about tasks due on a date with no time (all-day). */
    val allDayHour: Int = 9,
    /** Notify when a task's `wait` date passes and it reappears. */
    val waitOver: Boolean = true,
)

enum class NoticeKind { Soon, Due, AllDay, Wait, Digest }

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
 * Notices that should be shown right now.
 * [rows] are pending tasks (anything else is ignored); [seen] the keys already shown (the caller adds the keys of what it shows);
 * [tz] is the viewer's UTC offset in seconds.
 */
fun dueNotices(rows: List<Reminder>, now: Long, s: NotifySettings, seen: Set<String>, tz: Int): List<Notice> {
    val out = mutableListOf<Notice>()
    fun add(n: Notice) {
        if (n.key !in seen) out.add(n)
    }
    var overdue = 0

    for (r in rows) {
        if (r.status != "pending") continue
        val wait = r.wait
        val waiting = wait != null && wait > now
        val due = r.due
        if (due != null && !waiting) {
            if (isAllDay(due, tz)) {
                // A date without a time: remind at the chosen hour on that day, not at midnight.
                val at = due + s.allDayHour * HOUR
                if (now >= at && now < due + DAY) add(Notice("${r.uuid}:allday:$due", NoticeKind.AllDay, "Due today", r.description, r.uuid))
                if (now >= due + DAY) overdue++
            } else {
                val lead = s.leadMinutes * 60L
                if (lead > 0 && now >= due - lead && now < due) {
                    add(Notice("${r.uuid}:soon:$due", NoticeKind.Soon, "Due ${until(due - now)}", r.description, r.uuid))
                }
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
fun noticesToPost(rows: List<Reminder>, now: Long, s: NotifySettings, seen: Set<String>, tz: Int): List<Notice> {
    val hour = Math.floorMod(now + tz, DAY) / HOUR
    return dueNotices(rows, now, s, seen, tz).filter { it.kind != NoticeKind.Digest || hour >= s.allDayHour }
}

/**
 * The soonest moment after [now] at which [noticesToPost] has something new to say, or null when nothing ever will (until the
 * tasks change): when to set the alarm for.
 */
fun nextTrigger(rows: List<Reminder>, now: Long, s: NotifySettings, seen: Set<String>, tz: Int): Long? {
    val moments = sortedSetOf<Long>()
    for (r in rows) {
        if (r.status != "pending") continue
        r.due?.let { due ->
            if (isAllDay(due, tz)) moments.add(due + s.allDayHour * HOUR) else {
                if (s.leadMinutes > 0) moments.add(due - s.leadMinutes * 60L)
                moments.add(due)
            }
        }
        r.wait?.let { moments.add(it) }
    }
    // The overdue digest, at the chosen hour on this day and the next.
    val day = localDay(now, tz)
    for (k in 0..1) moments.add((day + k) * DAY - tz + s.allDayHour * HOUR)
    return moments.firstOrNull { it > now && noticesToPost(rows, it, s, seen, tz).isNotEmpty() }
}

/** Drop dedup keys older than [maxAge] so the stored set doesn't grow forever. */
fun prune(seen: Map<String, Long>, now: Long, maxAge: Long = 3 * DAY): Map<String, Long> = seen.filterValues { now - it < maxAge }
