package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

// Ported from web/src/lib/notify.test.ts, with the alarm's `nextTrigger` and `noticesToPost` added.
class NotifyTest {
    private val day = 86400L

    // 2026-10-07T12:30:00Z, as everywhere else.
    private val now = 1_791_376_200L
    private val midnight = now - (now % day) // 00:00Z today
    private val on = NotifySettings(enabled = true)

    private fun row(uuid: String = "u1", status: String = "pending", due: Long? = null, wait: Long? = null) =
        Reminder(uuid, status, "Pay rent", due, wait)

    private fun kinds(rows: List<Reminder>, at: Long = now, seen: Set<String> = emptySet(), s: NotifySettings = on, tz: Int = 0) =
        dueNotices(rows, at, s, seen, tz).map { it.kind }

    @Test
    fun headsUpInsideTheLeadWindowOnce() {
        val due = now + 10 * 60
        assertEquals(listOf(NoticeKind.Soon), kinds(listOf(row(due = due))))
        assertEquals(emptyList(), kinds(listOf(row(due = due)), now - 16 * 60)) // too early
        val n = dueNotices(listOf(row(due = due)), now, on, emptySet(), 0).first()
        assertEquals("Due in 10 minutes", n.title)
        assertEquals(emptyList(), kinds(listOf(row(due = due)), now, setOf(n.key))) // already shown
    }

    @Test
    fun dueNowAtTheDueTimeButNotForSomethingLongOverdue() {
        assertEquals(listOf(NoticeKind.Due), kinds(listOf(row(due = now - 60))))
        assertEquals(listOf(NoticeKind.Digest), kinds(listOf(row(due = now - 2 * 3600))))
    }

    @Test
    fun aLeadOfZeroTurnsTheHeadsUpOff() {
        assertEquals(emptyList(), kinds(listOf(row(due = now + 60)), now, emptySet(), on.copy(leadMinutes = 0)))
    }

    @Test
    fun isKeyedByTheDueTimeSoMovingTheDueDateNotifiesAgain() {
        val a = dueNotices(listOf(row(due = now + 300)), now, on, emptySet(), 0).first()
        val b = dueNotices(listOf(row(due = now + 600)), now, on, setOf(a.key), 0).first()
        assertNotEquals(a.key, b.key)
    }

    @Test
    fun allDayDatesAreDetectedInTheViewersZone() {
        assertTrue(isAllDay(midnight, 0))
        assertFalse(isAllDay(midnight, 19800)) // midnight UTC is 05:30 in IST
        assertTrue(isAllDay(midnight - 19800, 19800)) // IST midnight
    }

    @Test
    fun allDayDatesRemindAtTheChosenMorningHourNotAtMidnight() {
        fun t(h: Int) = midnight + h * 3600L
        assertEquals(emptyList(), kinds(listOf(row(due = midnight)), t(1)))
        assertEquals(listOf(NoticeKind.AllDay), kinds(listOf(row(due = midnight)), t(9)))
        assertEquals(listOf(NoticeKind.AllDay), kinds(listOf(row(due = midnight)), t(23)))
        assertEquals(emptyList(), kinds(listOf(row(due = midnight)), t(9), emptySet(), on.copy(allDayHour = 14)))
    }

    @Test
    fun allDayDatesBecomePartOfTheOverdueDigestTheNextDay() {
        assertEquals(listOf(NoticeKind.Digest), kinds(listOf(row(due = midnight - day))))
    }

    @Test
    fun tellsYouWhenAWaitingTaskComesBack() {
        assertEquals(listOf(NoticeKind.Wait), kinds(listOf(row(wait = now - 60))))
        assertEquals(emptyList(), kinds(listOf(row(wait = now + 3600))))
        assertEquals(emptyList(), kinds(listOf(row(wait = now - 60)), now, emptySet(), on.copy(waitOver = false)))
    }

    @Test
    fun aTaskThatIsStillWaitingIsNotAnnouncedAsDue() {
        assertEquals(emptyList(), kinds(listOf(row(due = now - 60, wait = now + 3600))))
    }

    @Test
    fun theOverdueDigestIsOneNotificationPerDayHoweverManyTasks() {
        val rows = listOf(row("a", due = now - 5 * day), row("b", due = now - 9 * day), row("c"))
        val n = dueNotices(rows, now, on, emptySet(), 0).first()
        assertEquals(NoticeKind.Digest, n.kind)
        assertEquals("2 overdue tasks", n.title)
        assertNull(n.uuid)
        assertEquals(emptyList(), dueNotices(rows, now, on, setOf(n.key), 0))
        // Tomorrow it can fire again.
        assertEquals(listOf(NoticeKind.Digest), dueNotices(rows, now + day, on, setOf(n.key), 0).map { it.kind })
    }

    @Test
    fun usesTheSingularForOneTask() {
        assertEquals("1 overdue task", dueNotices(listOf(row(due = now - day)), now, on, emptySet(), 0).first().title)
    }

    @Test
    fun ignoresCompletedAndDeletedTasks() {
        assertEquals(emptyList(), kinds(listOf(row(status = "completed", due = now - 60), row(status = "deleted", due = now + 60))))
    }

    @Test
    fun forgetsOldKeys() {
        assertEquals(mapOf("fresh" to now - 3600), prune(mapOf("old" to now - 4 * day, "fresh" to now - 3600), now))
    }

    // ---- The alarm

    @Test
    fun theAlarmHoldsTheDigestUntilTheChosenHour() {
        val rows = listOf(row(due = now - 5 * day))
        // 12:30 is after 09:00: shown. At 06:00 it waits.
        assertEquals(1, noticesToPost(rows, now, on, emptySet(), 0).size)
        assertEquals(0, noticesToPost(rows, midnight + 6 * 3600, on, emptySet(), 0).size)
        // Seen at a time of day when it can be shown: the same notice in an open page shows at any hour.
        assertEquals(1, dueNotices(rows, midnight + 6 * 3600, on, emptySet(), 0).size)
    }

    @Test
    fun theNextTriggerIsTheSoonestMomentWithSomethingToSay() {
        // Due in an hour: the heads-up is 15 minutes before it.
        val due = now + 3600
        assertEquals(due - 15 * 60, nextTrigger(listOf(row(due = due)), now, on, emptySet(), 0))
        // Inside the heads-up (and shown): the next is the due time.
        val soon = dueNotices(listOf(row(due = now + 600)), now, on, emptySet(), 0).first().key
        assertEquals(now + 600, nextTrigger(listOf(row(due = now + 600)), now, on, setOf(soon), 0))
        // An all-day date: that morning's hour.
        val tomorrow = midnight + day
        assertEquals(tomorrow + 9 * 3600, nextTrigger(listOf(row(due = tomorrow)), now, on, emptySet(), 0))
        // A waiting task: when it is back.
        assertEquals(now + 7200, nextTrigger(listOf(row(wait = now + 7200)), now, on, emptySet(), 0))
    }

    @Test
    fun theNextTriggerIsTheDigestHourWhenSomethingIsOverdue() {
        val rows = listOf(row(due = now - 5 * day))
        val today = dueNotices(rows, now, on, emptySet(), 0).first().key
        // Today's digest was shown: tomorrow at the chosen hour.
        assertEquals(midnight + day + 9 * 3600, nextTrigger(rows, now, on, setOf(today), 0))
    }

    @Test
    fun nothingScheduledWhenNothingWillEverBeSaid() {
        assertNull(nextTrigger(emptyList(), now, on, emptySet(), 0))
        assertNull(nextTrigger(listOf(row(status = "completed", due = now + 60)), now, on, emptySet(), 0))
    }
}
