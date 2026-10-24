package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

// The same cases as web/src/lib/holidayClash.test.ts. What is a holiday, and what a date word means, is the engine's
// (tc_core::clash, tested there); these check that the app carries it across, positions in UTF-16 units and all.
class HolidayClashTest {
    // 2026-10-18 12:00 UTC.
    private val now = 1_792_324_800L
    private val config = ConfigBody(
        settings = mapOf(
            "holiday.xmas.name" to "Christmas Day",
            "holiday.xmas.date" to "20261225",
            "holiday.today.name" to "Founders day",
            "holiday.today.date" to "20261018",
        ),
    )
    private val commands = setOf("add", "modify", "list", "done")

    @Test
    fun aDateOnAHolidayIsFoundWhateverItIsWrittenAs() {
        assertEquals(listOf("Christmas Day"), holidayClash("due", "2026-12-25", config, now, 0)?.names)
        assertNotNull(holidayClash("scheduled", "2026-12-25T10:30", config, now, 0))
        assertNull(holidayClash("due", "2026-12-26", config, now, 0))
        assertNull(holidayClash("due", "", config, now, 0))
        assertEquals("due (eod) is 2026-10-18, a holiday: Founders day", holidayClash("due", "eod", config, now, 0)?.message)
    }

    @Test
    fun nothingIsAskedOfTheEngineWhenNoHolidayIsDefined() {
        assertNull(holidayClash("due", "2026-12-25", ConfigBody(), now, 0))
        assertTrue(holidayMarks("add x due:2026-12-25", commands, false, ConfigBody(), now, 0).isEmpty())
    }

    @Test
    fun theDateWordOfACommandLineIsMarkedAndAFilterIsNot() {
        val line = "add 🎉 Party due:2026-12-25 +fun"
        val marks = holidayMarks(line, commands, false, config, now, 0)
        assertEquals(listOf("2026-12-25"), marks.map { line.substring(it.start, it.end) })
        assertEquals("due", marks.single().attr)
        assertTrue(holidayMarks("list due:2026-12-25", commands, false, config, now, 0).isEmpty())
        assertEquals(1, holidayMarks("Party due:2026-12-25", commands, true, config, now, 0).size)
    }
}
