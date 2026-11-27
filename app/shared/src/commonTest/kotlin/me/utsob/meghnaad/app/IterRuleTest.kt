package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class IterRuleTest {
    @Test
    fun aCalendarRuleRoundTripsThroughTheEngine() {
        val r = parseRule("FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,FR;UNTIL=20271231")
        assertEquals(CalendarRule("weekly", 2, listOf("MO", "FR"), until = "2027-12-31"), r)
        assertEquals("FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,FR;UNTIL=20271231", buildRule(r!!))
        // What Kotlin leaves out (an interval of 1) the engine takes as the default.
        assertEquals("FREQ=YEARLY", buildRule(CalendarRule("yearly")))
        assertNull(parseRule("monthly"))
        assertEquals("Every 2 weeks on Monday and Friday, until 2027-12-31", describeIter("FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,FR;UNTIL=20271231"))
    }

    @Test
    fun aNewRuleStartsFromTheTasksDay() {
        assertEquals(listOf("FR"), defaultRule("2026-12-25").days)
        assertEquals("2026-12-25", anchorOf(listOf("eom", "2026-12-25T10:00"), "2026-11-27"))
        assertEquals("2026-11-27", anchorOf(listOf("", ""), "2026-11-27"))
        val monthly = CalendarRule("monthly", monthDay = 25)
        assertEquals(listOf("On day 25", "On the fourth Friday", "On the last Friday"), monthOptions(monthly, "2026-12-25").map { it.label })
        val last = monthOptions(monthly, "2026-12-25").last().apply(monthly)
        assertEquals("FREQ=MONTHLY;BYDAY=-1FR", buildRule(last))
        assertEquals("n-1FR", monthKey(last))
    }
}
