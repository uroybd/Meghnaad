package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

// The editing logic ported from web/src/lib/boards.ts, with its tests.
class BoardsEditTest {
    private fun sec(id: String, y: Int = 0, h: Int = 2, type: String = "number") = Section(id = id, title = id, x = 0, y = y, w = 12, h = h, type = type)

    @Test
    fun anIdIsOneLowerCaseWordMadeUnique() {
        assertEquals("weekly-review", freshId("Weekly Review!", emptyList()))
        assertEquals("weekly-review-3", freshId("Weekly Review!", listOf("weekly-review", "weekly-review-2")))
        assertEquals("board", freshId("???", emptyList()))
        assertEquals(Board("my-week", "My week"), newBoard("  My week ", emptyList()))
    }

    @Test
    fun aNewSectionGoesUnderTheOthersWithWhatItNeeds() {
        val board = Board("b", "B", sections = listOf(sec("number", 0, 2), sec("bar", 2, 5, "bar")))
        val n = newSection(board, "number")
        assertEquals(listOf("number-2", 7, 2), listOf(n.id, n.y, n.h))
        assertEquals(By(time = TimeBy("entry", "week")), newSection(board, "line").by)
        assertEquals(By(field = "project"), newSection(board, "donut").by)
        assertEquals("next", newSection(board, "table").report)
        assertNull(newSection(board, "number").by)
    }

    @Test
    fun theSectionsAreLaidOneUnderTheOtherInTheOrderGiven() {
        val out = relayout(listOf(sec("a", 9, 2), sec("b", 4, 3), sec("c", 0, 1)))
        assertEquals(listOf(0, 2, 5), out.map { it.y })
        assertTrue(out.all { it.x == 0 && it.w == 12 })
    }

    @Test
    fun aSectionMovesUpOrDownAndStopsAtTheEnds() {
        val three = listOf(sec("a", 0, 2), sec("b", 2, 3), sec("c", 5, 1))
        assertEquals(listOf("b", "a", "c"), reorder(three, "b", -1).map { it.id })
        assertEquals(listOf("a", "c", "b"), reorder(three, "b", 1).map { it.id })
        assertEquals(listOf(0, 3, 5), reorder(three, "b", -1).map { it.y })
        assertEquals(three, reorder(three, "a", -1))
        assertEquals(three, reorder(three, "c", 1))
        assertEquals(three, reorder(three, "zz", 1))
    }

    @Test
    fun aChangeOfTypeGivesTheSectionWhatTheNewTypeNeedsAndTakesWhatItCannotHave() {
        val number = sec("n")
        assertEquals(By(field = "project"), withType(number, "bar").by)
        assertEquals(By(time = TimeBy("entry", "week")), withType(number, "area").by)
        assertEquals("next", withType(number, "table").report)
        val chart = sec("c", type = "bar").copy(by = By(field = "tags"), stacked = true, series = listOf(Series("a"), Series("b")))
        val pie = withType(chart, "pie")
        assertEquals(emptyList(), pie.series)
        assertNull(pie.stacked)
        // A chart that has something to group by keeps it.
        assertEquals(By(field = "tags"), pie.by)
        assertEquals(2, withType(chart, "radar").series.size)
    }

    @Test
    fun whichSeriesMayHaveSeveral() {
        assertTrue(hasManySeries("bar") && hasManySeries("line") && hasManySeries("area") && hasManySeries("radar"))
        assertTrue(!hasManySeries("pie") && !hasManySeries("donut") && !hasManySeries("polar") && !hasManySeries("number") && !hasManySeries("table"))
    }

    @Test
    fun aSpanIsOneOfTaskBurndownsStatesOrCustomOrNone() {
        assertEquals("period", spanKind(Series("a")))
        for (p in SPAN_PRESETS) assertEquals(p.id, spanKind(Series("a", span = p.span)))
        assertEquals("custom", spanKind(Series("a", span = StateSpan("scheduled", "end"))))
        assertNull(spanFor("period"))
        assertEquals(StateSpan("entry", "end", "start"), spanFor("pending"))
        assertEquals(StateSpan("entry"), spanFor("custom"))
        assertNull(spanFor("nope"))
    }

    @Test
    fun theFieldsToPickFromAreTaskwarriorsAndTheUsers() {
        fun uda(name: String, type: String) = UdaDef(name, type, null, emptyList(), null, null)
        val udas = listOf(uda("points", "numeric"), uda("effort", "duration"), uda("outcome", "string"), uda("when", "date"))
        assertEquals(listOf("urgency", "points", "effort"), measurableFields(udas))
        assertEquals(listOf("project", "tags", "priority", "status", "points", "outcome"), groupableFields(udas))
        assertEquals("when", dateFields(udas).last())
        assertTrue("end" in dateFields(emptyList()) && "when" !in dateFields(emptyList()))
    }
}
