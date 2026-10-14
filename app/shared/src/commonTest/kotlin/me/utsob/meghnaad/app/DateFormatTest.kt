package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

/** Writing a date is the engine's (these call it); which pattern applies where is the app's. */
class DateFormatTest {
    private val wed = 1_791_376_200L // 2026-10-07 12:30:00 UTC, a Wednesday (reference values from strftime)
    private val jan1y2026 = 1_767_225_600L // Thursday
    private val jan1y2027 = 1_798_761_600L // Friday, ISO week 53 of 2026

    private fun f(pattern: String, ws: Int = 1) = DateFmt(pattern, ws)

    @Test
    fun formatsTaskwarriorTokensPaddedAndUnpadded() {
        assertEquals("2026-10-07", formatPattern(wed, f("Y-M-D"), 0))
        assertEquals("7/10/26", formatPattern(wed, f("d/m/y"), 0))
        assertEquals("2026-10-07 12:30:00", formatPattern(wed, f("Y-M-D H:N:S"), 0))
        assertEquals("12:30:0", formatPattern(wed, f("h:n:s"), 0))
    }

    @Test
    fun namesDaysAndMonths() {
        assertEquals("Wed Wednesday Oct October", formatPattern(wed, f("a A b B"), 0))
        assertEquals("3", formatPattern(wed, f("w"), 0))
    }

    @Test
    fun copiesAnythingThatIsNotATokenAndTokensAreCaseSensitive() {
        assertEquals("[2026] @ 12.30", formatPattern(wed, f("[Y] @ H.N"), 0))
        assertEquals("Z-q", formatPattern(wed, f("Z-q"), 0))
    }

    @Test
    fun countsDaysAndWeeksLikeStrftime() {
        assertEquals("280 280", formatPattern(wed, f("j J"), 0))
        assertEquals("1 001", formatPattern(jan1y2026, f("j J"), 0))
        assertEquals("41 41", formatPattern(wed, f("V v", 1), 0)) // %V
        assertEquals("40", formatPattern(wed, f("V", 0), 0)) // %U
        assertEquals("00", formatPattern(jan1y2026, f("V", 0), 0)) // before the first Sunday
        assertEquals("01", formatPattern(jan1y2026, f("V", 1), 0))
        assertEquals("53", formatPattern(jan1y2027, f("V", 1), 0)) // ISO week of the old year
    }

    @Test
    fun usesTheViewerZone() {
        // 23:30 UTC on the 7th is already the 8th at UTC+2.
        assertEquals("2026-10-08 01:30", formatPattern(wed + 11 * 3600, f("Y-M-D H:N"), 2 * 3600))
    }

    private val settings = mapOf(
        "dateformat" to "D/M/Y",
        "dateformat.report" to "Y-M-D",
        "dateformat.info" to "Y-M-D H:N",
        "dateformat.annotation" to "D.M.",
    )

    @Test
    fun aReportUsesItsOwnThenDateformatReportThenDateformat() {
        assertEquals("Y", formatFor(FormatKind.Report, settings, "Y")?.pattern)
        assertEquals("Y-M-D", formatFor(FormatKind.Report, settings)?.pattern)
        assertEquals("D/M/Y", formatFor(FormatKind.Report, mapOf("dateformat" to "D/M/Y"))?.pattern)
    }

    @Test
    fun infoAndAnnotations() {
        assertEquals("Y-M-D H:N", formatFor(FormatKind.Info, settings)?.pattern)
        assertEquals("D.M.", formatFor(FormatKind.Annotation, settings)?.pattern)
        assertEquals("D/M/Y", formatFor(FormatKind.Annotation, mapOf("dateformat" to "D/M/Y"))?.pattern)
        // In the detail view an unset annotation format follows the detail view's own.
        assertEquals("Y-M-D H:N", formatFor(FormatKind.InfoNote, mapOf("dateformat.info" to "Y-M-D H:N"))?.pattern)
        assertEquals("D.M.", formatFor(FormatKind.InfoNote, settings)?.pattern)
    }

    @Test
    fun nothingConfiguredMeansTheBuiltInDisplay() {
        assertNull(formatFor(FormatKind.Report, emptyMap()))
        assertNull(formatFor(FormatKind.Info, null))
    }

    @Test
    fun weekStart() {
        assertEquals(0, weekStartOf(mapOf("weekstart" to "Sunday")))
        assertEquals(1, weekStartOf(mapOf("weekstart" to "monday")))
        assertEquals(0, weekStartOf(emptyMap())) // Sunday, as in Taskwarrior, unless the taskrc says otherwise
        assertEquals(0, formatFor(FormatKind.Report, mapOf("dateformat" to "V", "weekstart" to "sunday"))?.weekstart)
    }

    @Test
    fun formatMomentKeepsTheBuiltInDisplayWithoutAPattern() {
        assertEquals("2026-10-07 12:30", formatMoment(wed, 0))
        assertEquals("07/10/2026", formatMoment(wed, 0, f("D/M/Y")))
        assertEquals("", formatMoment(null, 0, f("D/M/Y")))
    }
}
