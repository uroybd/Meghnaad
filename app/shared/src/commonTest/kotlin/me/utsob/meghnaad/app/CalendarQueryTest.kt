package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class CalendarQueryTest {
    @Test
    fun knowsTheWordsCalendarReads() {
        for (w in listOf("due", "du", "y", "Y", "3", "12", "2027", "march", "mar", "DEC", "sept")) assertTrue(isCalendarWord(w), w)
        for (w in listOf("project:Work", "+next", "/milk/", "-waiting", "Work", "d", "x", "(", "and")) assertFalse(isCalendarWord(w), w)
    }

    @Test
    fun splitsAQueryIntoItsFilterAndItsMonths() {
        assertEquals(listOf("project:Work", "+next"), calendarFilterWords("project:Work march 2027 +next"))
        assertEquals(emptyList(), calendarFilterWords("due"))
        assertEquals("project:Work 5 2028", withMonths("project:Work march 2027", listOf("5", "2028")))
        assertEquals("", withMonths("march 2027", emptyList()))
        assertEquals("'/buy milk/' y", withMonths("'/buy milk/' 3 2027", listOf("y")))
    }

    @Test
    fun movesByMonthsAcrossYears() {
        assertEquals(YearMonth(2027, 1), shiftMonth(YearMonth(2026, 10), 3))
        assertEquals(YearMonth(2025, 12), shiftMonth(YearMonth(2026, 10), -10))
        assertEquals(YearMonth(2025, 12), shiftMonth(YearMonth(2026, 1), -1))
        assertEquals(YearMonth(2027, 1), shiftMonth(YearMonth(2026, 12), 1))
        assertEquals(YearMonth(2026, 6), shiftMonth(YearMonth(2026, 6), 0))
    }

    @Test
    fun stepsByWhatIsShownAndKeepsAWholeYearAWholeYear() {
        assertEquals(listOf("1", "2027"), stepWords(YearMonth(2026, 10), 3, 1))
        assertEquals(listOf("7", "2026"), stepWords(YearMonth(2026, 10), 3, -1))
        assertEquals(listOf("1", "2027", "y"), stepWords(YearMonth(2026, 1), 12, 1))
        assertEquals(listOf("1", "2025", "y"), stepWords(YearMonth(2026, 1), 12, -1))
        // calendar.monthsperline=2: two months at a time.
        assertEquals(listOf("12", "2026"), stepWords(YearMonth(2026, 10), 2, 1))
    }
}
