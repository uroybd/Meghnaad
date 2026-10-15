package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals

// The same cases as web/src/lib/recurrence.test.ts.
class RecurrenceTest {
    @Test
    fun theCommonPeriodsAreNamed() {
        assertEquals("every day", describeRecur("daily"))
        assertEquals("every weekday", describeRecur("weekdays"))
        assertEquals("every 2 weeks", describeRecur("biweekly"))
        assertEquals("every 3 months", describeRecur("quarterly"))
        assertEquals("every year", describeRecur("Annual"))
    }

    @Test
    fun countsAndUnitsAreRead() {
        assertEquals("every 3 days", describeRecur("3d"))
        assertEquals("every week", describeRecur("1w"))
        assertEquals("every 2 weeks", describeRecur("2 weeks"))
        assertEquals("every 6 months", describeRecur("6mo"))
        assertEquals("every 2 quarters", describeRecur("2q"))
    }

    @Test
    fun isoDurationsAreRead() {
        assertEquals("every 1 month", describeRecur("P1M"))
        assertEquals("every 2 weeks", describeRecur("P2W"))
        assertEquals("every 1 year 2 months", describeRecur("P1Y2M"))
    }

    @Test
    fun unknownTextIsLeftAloneAndEmptyIsHandled() {
        assertEquals("fortnightish", describeRecur("fortnightish"))
        assertEquals("P", describeRecur("P"))
        assertEquals("", describeRecur(null))
        assertEquals("", describeRecur(""))
    }

    @Test
    fun onlyExactPresetsMatchSoAnythingElseOpensTheCustomField() {
        for (p in PRESETS) assertEquals(p.value, presetFor(p.value))
        assertEquals("", presetFor("3d"))
        assertEquals("", presetFor("week"))
        assertEquals("", presetFor(null))
    }
}
