package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class SortSpecTest {
    private fun k(column: String, desc: Boolean = false, brk: Boolean = false) = SortKey(column, desc, brk)

    @Test
    fun parseAndSerializeRoundTripWithBreakMarkers() {
        val spec = "start-,due+,project+/,urgency-"
        assertEquals(spec, serializeSort(parseSort(spec)))
        assertEquals(k("project", false, true), parseSort(spec)[2])
    }

    @Test
    fun noneRandomAndJunkMeanNoKeys() {
        assertEquals(emptyList(), parseSort("none"))
        assertEquals(emptyList(), parseSort("random"))
        assertEquals(emptyList(), parseSort(null))
        assertEquals(emptyList(), parseSort("due")) // no direction: not a valid key
    }

    @Test
    fun aColumnFormatIsStripped() {
        assertEquals(listOf(k("due")), parseSort("due.relative+"))
    }

    @Test
    fun aNewColumnBecomesPrimaryKeepingTheOldKeys() {
        val r = clickSort(listOf(k("due"), k("urgency", true)), "project")
        assertEquals("project+,due+,urgency-", serializeSort(r.keys))
        assertFalse(r.reset)
    }

    @Test
    fun reclickingThePrimaryCyclesAscDescDefault() {
        val asc = clickSort(listOf(k("due"), k("urgency", true)), "due")
        assertEquals("due-,urgency-", serializeSort(asc.keys))
        assertTrue(clickSort(asc.keys, "due").reset)
    }

    @Test
    fun aSecondaryColumnIsPromotedWithoutDuplicating() {
        assertEquals("project+,due+", serializeSort(clickSort(listOf(k("due"), k("project", true)), "project").keys))
    }

    @Test
    fun shiftAppendsATieBreakerOrFlipsIt() {
        val added = clickSort(listOf(k("due")), "project", true)
        assertEquals("due+,project+", serializeSort(added.keys))
        assertEquals("due+,project-", serializeSort(clickSort(added.keys, "project", true).keys))
    }

    @Test
    fun neverMoreThanFourKeys() {
        val r = clickSort(listOf(k("a"), k("b"), k("c"), k("d")), "e")
        assertEquals(4, r.keys.size)
        assertEquals("e", r.keys[0].column)
        assertEquals("due+", serializeSort(clickSort(emptyList(), "due").keys))
    }

    @Test
    fun theSortTokenInAFilterStringForOneReportOnly() {
        var f = withSortOverride("project:Work", "next", "due-", "urgency-")
        assertEquals("project:Work rc.report.next.sort:due-", f)
        assertEquals("due-", sortOverride(f, "next"))
        f = withSortOverride(f, "next", "description+", "urgency-")
        assertEquals("project:Work rc.report.next.sort:description+", f)
        assertNull(sortOverride(f, "list"))
        f = withSortOverride(f, "next", null, "urgency-")
        assertEquals("project:Work", f)
    }

    @Test
    fun anOverrideEqualToTheDefaultIsDropped() {
        assertEquals("", withSortOverride("", "next", "urgency-", "urgency-"))
        assertEquals(
            "rc.report.list.sort:due+ rc.report.next.sort:urgency+",
            withSortOverride("rc.report.list.sort:due+", "next", "urgency+", "urgency-"),
        )
    }

    @Test
    fun reportNamesWithRegexCharactersAreSafe() {
        assertEquals("due+", sortOverride("rc.report.a.b.sort:due+", "a.b"))
        assertNull(sortOverride("rc.report.axb.sort:due+", "a.b"))
    }

    @Test
    fun sortStateGivesDirectionRankAndGrouping() {
        val keys = parseSort("due+,project-")
        val due = sortState(keys, "due.relative")!!
        assertEquals(false to 1, due.desc to due.rank)
        val project = sortState(keys, "project")!!
        assertEquals(true to 2, project.desc to project.rank)
        assertNull(sortState(keys, "tags"))
        assertNull(sortState(parseSort("due+"), "due")!!.rank)
        assertTrue(sortState(parseSort("project+/,due+"), "project")!!.group)
        assertFalse(sortState(parseSort("project+/,due+"), "due")!!.group)
    }

    @Test
    fun groupingTurnsOnAndOffKeepingPlaceAndDirection() {
        val own = parseSort("urgency-,project-,due+")
        assertEquals("urgency-,project-/", serializeSort(toggleGroup(parseSort("urgency-,project-"), "project", own)))
        assertEquals("urgency-,project-", serializeSort(toggleGroup(parseSort("urgency-,project-/"), "project", own)))
        assertEquals("due+/", serializeSort(toggleGroup(parseSort("due.relative+"), "due", own)))
    }

    @Test
    fun endingAGroupingThatAddedTheColumnTakesItOut() {
        val defaults = parseSort("urgency-")
        val grouped = toggleGroup(defaults, "project", defaults)
        assertEquals("project+/,urgency-", serializeSort(grouped))
        assertEquals("urgency-", serializeSort(toggleGroup(grouped, "project", defaults)))
        val own2 = parseSort("project+/,description+")
        assertEquals("project+,description+", serializeSort(toggleGroup(own2, "project", own2)))
    }

    @Test
    fun groupingByAnUnusedColumnSortsByItFirst() {
        assertEquals("project+/,urgency-", serializeSort(toggleGroup(parseSort("urgency-"), "project")))
        assertEquals("outcome+/", serializeSort(toggleGroup(emptyList(), "outcome")))
        assertEquals(4, toggleGroup(parseSort("a+,b+,c+,d+"), "e").size)
    }

    @Test
    fun clearingGroupsAndFindingTheFirst() {
        val keys = parseSort("project+/,outcome+/,due+")
        assertEquals("project", groupColumn(keys))
        assertEquals("project+,outcome+,due+", serializeSort(clearGroups(keys)))
        assertNull(groupColumn(clearGroups(keys)))
    }
}
