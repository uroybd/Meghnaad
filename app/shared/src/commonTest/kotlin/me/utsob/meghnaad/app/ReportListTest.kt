package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

// The same cases as web/src/lib/reportList.test.ts.
class ReportListTest {
    private fun r(name: String) = ReportMeta(name)
    private val all = listOf(r("next"), r("_reviewed"), r("list"), r("_unique"), r("my_report"))
    private fun names(l: List<ReportMeta>) = l.map { it.name }

    @Test
    fun hiddenReportsAreTheOnesWhoseNamesStartWithAnUnderscore() {
        assertTrue(isHiddenReport("_reviewed"))
        for (n in listOf("next", "my_report", "a_", "")) assertFalse(isHiddenReport(n), n)
    }

    @Test
    fun theyAreLeftOutOfWhatIsOfferedUnlessAllAreAskedFor() {
        assertEquals(listOf("next", "list", "my_report"), names(offeredReports(all, false, "next")))
        assertEquals(listOf("next", "_reviewed", "list", "_unique", "my_report"), names(offeredReports(all, true, "next")))
    }

    @Test
    fun theyStayOfferedWhileOneOfThemIsTheReportInUse() {
        assertEquals(listOf("next", "_reviewed", "list", "my_report"), names(offeredReports(all, false, "_reviewed")))
    }

    @Test
    fun theListIsLeftAsItIsWhenThereAreNone() {
        val plain = listOf(r("next"), r("list"))
        assertEquals(plain, offeredReports(plain, false, "next"))
    }
}
