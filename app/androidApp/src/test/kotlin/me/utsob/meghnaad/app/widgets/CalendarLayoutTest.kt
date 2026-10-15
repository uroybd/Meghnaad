package me.utsob.meghnaad

import org.junit.Assert.assertEquals
import org.junit.Test

class CalendarLayoutTest {
    @Test
    fun aSmallWidgetIsTheMonthAlone() {
        assertEquals(CalendarLayout.MONTH, CalendarLayout.choose(180, 180))
        assertEquals(CalendarLayout.MONTH, CalendarLayout.choose(250, 200))
        assertEquals(CalendarLayout.MONTH, CalendarLayout.choose(0, 0))
        // Wide enough for two columns, but not high enough to show a month beside anything.
        assertEquals(CalendarLayout.MONTH, CalendarLayout.choose(400, 150))
    }

    @Test
    fun aWideOneHasTheDayBesideTheMonth() {
        assertEquals(CalendarLayout.WIDE, CalendarLayout.choose(320, 240))
        assertEquals(CalendarLayout.WIDE, CalendarLayout.choose(400, 250))
    }

    @Test
    fun aTallOneHasTheDayUnderTheMonth() {
        assertEquals(CalendarLayout.TALL, CalendarLayout.choose(190, 360))
        assertEquals(CalendarLayout.TALL, CalendarLayout.choose(250, 500))
    }

    @Test
    fun whereBothFitTheLongerSideDecides() {
        assertEquals(CalendarLayout.WIDE, CalendarLayout.choose(400, 380))
        assertEquals(CalendarLayout.TALL, CalendarLayout.choose(330, 420))
        assertEquals(CalendarLayout.WIDE, CalendarLayout.choose(360, 360))
    }
}
