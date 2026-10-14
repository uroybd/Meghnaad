package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class DatesTest {
    private val ist = 19_800 // +05:30

    // 2026-12-25T08:30:00Z
    private val xmas0830z = 1_798_187_400L

    @Test
    fun aDateOnlyValueStaysDateOnly() {
        assertEquals("2026-12-25", fromParts(DateParts("2026-12-25", "")))
    }

    @Test
    fun addingATimeMakesItAMoment() {
        assertEquals("2026-12-25T08:30", fromParts(DateParts("2026-12-25", "08:30")))
    }

    @Test
    fun noDateMeansNoValueEvenIfATimeWasTyped() {
        assertEquals("", fromParts(DateParts("", "08:30")))
    }

    @Test
    fun showsTheTimeOnlyWhenItIsNotMidnight() {
        assertEquals(DateParts("2026-12-25", "08:30"), toParts(xmas0830z, 0))
        assertEquals(DateParts("2026-12-25", ""), toParts(1_798_156_800, 0))
    }

    @Test
    fun rendersInTheViewersZoneWhichCanChangeTheDate() {
        // 08:30Z is 14:00 in IST; 20:00Z is already the next day there.
        assertEquals(DateParts("2026-12-25", "14:00"), toParts(xmas0830z, ist))
        assertEquals(DateParts("2026-12-26", "02:00"), toParts(xmas0830z + 12 * 3600, ist))
    }

    @Test
    fun aMomentRoundTripsThroughLocalParts() {
        assertEquals("2026-12-25T14:00", fromParts(toParts(xmas0830z, ist)))
    }

    @Test
    fun formatsForDisplay() {
        assertEquals("2026-12-25 08:30", formatMoment(xmas0830z, 0))
        assertEquals("2026-12-25", formatMoment(1_798_156_800, 0))
        assertEquals("", formatMoment(null, 0))
    }

    @Test
    fun validates() {
        assertFalse(isValidDate("2026-02-29"))
        assertTrue(isValidDate("2028-02-29"))
        assertFalse(isValidDate("2026-13-01"))
        assertTrue(isValidTime("23:59"))
        assertFalse(isValidTime("24:00"))
        assertFalse(isValidTime("9:30"))
    }

    @Test
    fun trackedTimeIsWrittenInTwoUnits() {
        val cases = listOf(0L to "0s", 45L to "45s", 75L to "1m 15s", 3600L to "1h 0m", 4325L to "1h 12m", 90_061L to "1d 1h", -5L to "0s")
        for ((sec, want) in cases) assertEquals(want, formatSeconds(sec), "$sec")
    }

    @Test
    fun aStampShowsSecondsInTheGivenZone() {
        assertEquals("2026-12-25 08:30:05", formatStamp(1_798_187_405, 0))
        assertEquals("2026-12-25 14:00:05", formatStamp(1_798_187_405, 19_800))
        assertEquals("", formatStamp(null, 0))
    }

    @Test
    fun theCivilDateIsRightAcrossLeapYearsAndBeforeTheEpoch() {
        assertEquals("2026-10-16", isoDate(1_792_087_200, 21_600))
        assertEquals("1970-01-01", isoDate(0, 0))
        assertEquals("2000-02-29", isoDate(951_782_400, 0))
        assertEquals("2024-02-28", isoDate(1_709_164_800 - 1, 0))
        assertEquals("2024-02-29", isoDate(1_709_164_800, 0))
        assertEquals("1969-12-31", isoDate(0, -3600))
    }

    @Test
    fun numbersAreRoundedAsToFixedDoes() {
        assertEquals("16.20", fixed(16.2, 2))
        assertEquals("4.50", fixed(4.5, 2))
        assertEquals("2440587.50000", fixed(2_440_587.5, 5))
        assertEquals("-0.50", fixed(-0.5, 2))
    }
}
