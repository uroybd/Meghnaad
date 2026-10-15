package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals

// The same cases as web/src/lib/recurrence.test.ts.
class RecurrenceTest {
    // The words are the engine's (tc_core::helpers::describe_recur, tested there with the cases that were here).
    @Test
    fun theEngineDescribesAPeriod() {
        assertEquals("every 3 days", describeRecur("3d"))
        assertEquals("every week", describeRecur("weekly"))
    }

    @Test
    fun noPeriodAndUnknownTextAreHandled() {
        assertEquals("", describeRecur(null))
        assertEquals("", describeRecur(""))
        assertEquals("fortnightish", describeRecur("fortnightish"))
    }

    @Test
    fun onlyExactPresetsMatchSoAnythingElseOpensTheCustomField() {
        for (p in PRESETS) assertEquals(p.value, presetFor(p.value))
        assertEquals("", presetFor("3d"))
        assertEquals("", presetFor("week"))
        assertEquals("", presetFor(null))
    }
}
