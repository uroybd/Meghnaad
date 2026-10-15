package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

// The behaviour is the engine's (tc_core::sortspec, tested there with the cases that were here). These check that the app's
// side carries keys and answers across: the shapes, the nulls and the defaults.
class SortSpecTest {
    private fun k(column: String, desc: Boolean = false, brk: Boolean = false) = SortKey(column, desc, brk)

    @Test
    fun aSpecRoundTripsWithBreakMarkersAndAColumnFormatIsStripped() {
        val spec = "start-,due+,project+/,urgency-"
        assertEquals(spec, serializeSort(parseSort(spec)))
        assertEquals(k("project", false, true), parseSort(spec)[2])
        assertEquals(listOf(k("due")), parseSort("due.relative+"))
        for (s in listOf("none", "random", "due", null)) assertEquals(emptyList(), parseSort(s), s)
    }

    @Test
    fun aClickAnswersKeysAndWhetherToGoBackToTheDefault() {
        val asc = clickSort(listOf(k("due"), k("urgency", true)), "due")
        assertEquals("due-,urgency-", serializeSort(asc.keys))
        assertEquals(false, asc.reset)
        assertTrue(clickSort(asc.keys, "due").reset)
        assertEquals("due+,project+", serializeSort(clickSort(listOf(k("due")), "project", shift = true).keys))
    }

    @Test
    fun theSortTokenInAFilterStringIsAddedFoundAndRemoved() {
        var f = withSortOverride("project:Work", "next", "due-", "urgency-")
        assertEquals("project:Work rc.report.next.sort:due-", f)
        assertEquals("due-", sortOverride(f, "next"))
        assertNull(sortOverride(f, "list"))
        f = withSortOverride(f, "next", null, null)
        assertEquals("project:Work", f)
        assertEquals("", withSortOverride("", "next", "urgency-", "urgency-"))
    }

    @Test
    fun aHeaderReportsItsPlaceAndGroupingIsToggled() {
        val keys = parseSort("due+,project-")
        val s = sortState(keys, "due.relative")!!
        assertEquals(false, s.desc)
        assertEquals(1, s.rank)
        assertNull(sortState(parseSort("due+"), "due")!!.rank)
        assertNull(sortState(keys, "tags"))
        val own = parseSort("urgency-,project-")
        assertEquals("urgency-,project-/", serializeSort(toggleGroup(own, "project", own)))
        assertEquals("project+/,urgency-", serializeSort(toggleGroup(parseSort("urgency-"), "project")))
        val grouped = parseSort("project+/,outcome+/,due+")
        assertEquals("project", groupColumn(grouped))
        assertEquals("project+,outcome+,due+", serializeSort(clearGroups(grouped)))
        assertNull(groupColumn(clearGroups(grouped)))
    }
}
