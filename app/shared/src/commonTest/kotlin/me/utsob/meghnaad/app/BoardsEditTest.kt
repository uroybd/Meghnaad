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
        // Where there is room: not beside the full-width sections, but under them.
        assertEquals(listOf("number-2", 7, 2, 3), listOf(n.id, n.y, n.h, n.w))
        assertEquals(By(time = TimeBy("entry", "week")), newSection(board, "line").by)
        assertEquals(By(field = "project"), newSection(board, "donut").by)
        assertEquals("next", newSection(board, "table").report)
        assertNull(newSection(board, "number").by)
    }

    @Test
    fun aCopiedSectionTakesTheFirstFreePlaceAndAnIdAndATitleOfItsOwn() {
        val original = Section(id = "bar", title = "Open", x = 0, y = 0, w = 6, h = 5, type = "bar", by = By(field = "project"), series = listOf(Series(label = "a")))
        val copy = copySection(Board("b", "B", sections = listOf(original)), original)
        assertEquals(listOf("bar-2", "Open copy", 6, 0, 6, 5), listOf(copy.id, copy.title, copy.x, copy.y, copy.w, copy.h))
        assertEquals(original.by, copy.by)
        assertEquals(original.series, copy.series)
        // A copy of a copy is `bar-3`, not `bar-2-2`.
        assertEquals("bar-3", copySection(Board("b", "B", sections = listOf(original, copy)), copy).id)
    }

    @Test
    fun aCopiedBoardHasANameAndAnAddressOfItsOwnAndTheSameSections() {
        val original = Board("week", "My week", filter = "project:Home", sections = listOf(sec("a")))
        val copy = copyBoard(original, listOf(original))
        assertEquals(listOf("my-week-copy", "My week copy", "project:Home"), listOf(copy.id, copy.name, copy.filter))
        assertEquals(original.sections, copy.sections)
        assertEquals("my-week-copy-2", copyBoard(original, listOf(original, copy)).id)
    }

    @Test
    fun tablesAndChartsStackOnAPhoneAndNumbersKeepTheirShare() {
        val home = listOf(
            Section(id = "table", x = 0, y = 0, w = 6, h = 8, type = "table"),
            Section(id = "bar", x = 6, y = 0, w = 6, h = 4, type = "bar"),
            Section(id = "pie", x = 6, y = 4, w = 3, h = 4, type = "pie"),
            Section(id = "done", x = 9, y = 4, w = 3, h = 2, type = "number"),
        )
        // Half the grid or more is a whole row, less is half a row, and a number is its quarter.
        assertEquals(listOf(4, 4, 2, 1), flowed(home, 4).map { it.w })
        assertEquals(listOf(0 to 0, 0 to 8, 0 to 12, 2 to 12), flowed(home, 4).map { it.x to it.y })
    }

    @Test
    fun aPhoneHasSeveralNumbersToARowAndNoSectionNarrowerThanTwoColumns() {
        val row = listOf(
            Section(id = "a", x = 0, y = 0, w = 3, h = 2, type = "number"),
            Section(id = "b", x = 3, y = 0, w = 3, h = 2, type = "number"),
            Section(id = "c", x = 6, y = 0, w = 3, h = 2, type = "number"),
            Section(id = "d", x = 9, y = 0, w = 3, h = 2, type = "number"),
            Section(id = "t", x = 0, y = 2, w = 12, h = 4, type = "table"),
        )
        assertEquals(listOf(0, 1, 2, 3, 0), flowed(row, 4).map { it.x })
        assertEquals(listOf(1, 1, 1, 1, 4), flowed(row, 4).map { it.w })
        // On six columns three fit and the fourth goes under them.
        assertEquals(listOf(0 to 0, 2 to 0, 4 to 0, 0 to 2), flowed(row, 6).take(4).map { it.x to it.y })
        // Resizing is in the phone's columns, kept in the board's twelve, and stops at one column.
        assertEquals(6, resizeFlow(row, "a", 4, 1, 0).first { it.id == "a" }.w)
        assertEquals(MIN_WIDTH, resizeFlow(row, "a", 6, -9, 0).first { it.id == "a" }.w)
        // Only the height changing leaves the width as it was.
        assertEquals(3 to 3, resizeFlow(row, "a", 4, 0, 1).first { it.id == "a" }.let { it.w to it.h })
    }

    @Test
    fun aSectionMovesUpOrDownAndStopsAtTheEnds() {
        val three = listOf(sec("a", 0, 2), sec("b", 2, 3), sec("c", 5, 1))
        assertEquals(listOf("b", "a", "c"), stacked(reorder(three, "b", -1)).map { it.id })
        assertEquals(listOf("a", "c", "b"), stacked(reorder(three, "b", 1)).map { it.id })
        assertEquals(listOf(0, 3, 5), stacked(reorder(three, "b", -1)).map { it.y })
        // The widths are kept.
        assertTrue(reorder(three, "b", -1).all { it.w == 12 })
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
