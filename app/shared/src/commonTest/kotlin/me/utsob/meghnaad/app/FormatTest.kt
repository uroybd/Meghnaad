package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

// What each format shows is the engine's (`tc_core::cells`, tested there); these check that the app hands it the table and
// gets the cells back as it draws them.
class FormatTest {
    private val now = 1_791_376_200L // 2026-10-07T12:30:00Z
    private val day = 86_400L

    private fun row(
        description: String = "Buy milk",
        status: String = "pending",
        project: String? = null,
        priority: String? = null,
        tags: List<String> = emptyList(),
        annotations: List<Note> = emptyList(),
        start: Long? = null,
        end: Long? = null,
        due: Long? = null,
        depends: List<String> = emptyList(),
        pendingDeps: Int = 0,
        recur: String? = null,
        blocked: Boolean = false,
        waiting: Boolean = false,
        extra: Map<String, String> = emptyMap(),
        urgency: Double = 4.5,
        id: Int? = 3,
    ) = Row(
        uuid = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
        status = status,
        description = description,
        project = project,
        priority = priority,
        tags = tags,
        annotations = annotations,
        entry = now - 3 * day,
        start = start,
        end = end,
        due = due,
        depends = depends,
        pendingDeps = pendingDeps,
        recur = recur,
        blocked = blocked,
        waiting = waiting,
        extra = extra,
        urgency = urgency,
        id = id,
    )

    private fun col(spec: String, kind: String = "string"): Column {
        val name = spec.substringBefore('.')
        val format = spec.substringAfter('.', "").ifEmpty { null }
        return Column(spec = spec, name = name, format = format, label = name, kind = kind)
    }

    private val ctx = Ctx(now = now, tz = 0)

    private fun cell(c: Column, r: Row, x: Ctx) = formatCells(listOf(c), listOf(r), x)[0][0]

    @Test
    fun answersACellForEveryRowAndColumnInOrder() {
        val out = formatCells(listOf(col("id"), col("description")), listOf(row(), row(id = null, description = "Other")), ctx)
        assertEquals(listOf(listOf("3", "Buy milk"), listOf("aaaaaaaa", "Other")), out.map { r -> r.map { it.text } })
        assertEquals(emptyList(), formatCells(listOf(col("id")), emptyList(), ctx))
    }

    @Test
    fun keepsWhatACellCarriesBesidesItsText() {
        assertEquals(Cell("aaaaaaaa", cls = "dim", uuid = row().uuid), cell(col("id"), row(id = null), ctx))
        assertEquals(listOf("Home", "Kitchen"), cell(col("project"), row(project = "Home.Kitchen"), ctx).segments)
        assertEquals(listOf("a", "b"), cell(col("tags"), row(tags = listOf("a", "b")), ctx).chips)
        assertEquals(listOf(NoteCell("2026-10-07 12:30", "n", now)), cell(col("description"), row(annotations = listOf(Note(now, "n"))), ctx).notes)
        assertNull(cell(col("description"), row(), ctx).notes)
    }

    @Test
    fun writesDatesInThePatternAndTheZoneItIsGiven() {
        val due = now + 2 * day
        assertEquals("2026-10-09 12:30", cell(col("due"), row(due = due), ctx).text)
        assertEquals("2d", cell(col("due.relative"), row(due = due), ctx).text)
        val withDates = Ctx(now = now, tz = 3600, reportDates = DateFmt("D.M.Y H:N", 1), annotationDates = DateFmt("M/D", 1))
        val r = row(due = now, annotations = listOf(Note(now, "called")))
        assertEquals("07.10.2026 13:30", cell(col("due"), r, withDates).text)
        assertEquals(listOf("10/07" to "called"), cell(Column("description", "description", null, "description", "description"), r, withDates).notes?.map { it.at to it.text })
    }

    @Test
    fun usesTheOffsetEachMomentHad() {
        // An hour more from the middle of the year on, as with summer time.
        val summer = now - 30 * day
        val zone = Ctx(now = now, tz = 0, offsetAt = { if (it >= summer) 7200 else 3600 })
        val out = formatCells(listOf(col("due")), listOf(row(due = summer - 3600), row(due = summer)), zone)
        assertEquals(listOf("2026-09-07 12:30", "2026-09-07 14:30"), out.map { it[0].text })
    }

    @Test
    fun theSettingsReachTheEngine() {
        val custom = Ctx(
            now = now,
            tz = 0,
            indicators = Indicators(active = ">>", tag = "#", dependency = "DEP"),
            recurIndicator = "↻",
            journal = Journal("Started task", "Stopped task"),
            udas = mapOf("ship" to UdaDef(name = "ship", type = "date")),
        )
        assertEquals(">>", cell(col("start.active"), row(start = now), custom).text)
        assertEquals("#", cell(col("tags.indicator"), row(tags = listOf("x")), custom).text)
        assertEquals("DEP", cell(col("depends.indicator"), row(pendingDeps = 1), custom).text)
        assertEquals("↻", cell(col("recur.indicator"), row(recur = "weekly"), custom).text)
        val noted = row(annotations = listOf(Note(now, "Started task"), Note(now + 5, "call Sam first")))
        assertEquals(listOf("call Sam first"), cell(col("description"), noted, custom).notes?.map { it.text })
        assertEquals("2026-10-07 12:30", cell(col("ship", "date"), row(extra = mapOf("ship" to now.toString())), custom).text)
    }

    @Test
    fun theTableMemoIsKeyedByTheResultAndTheSettings() {
        val result = ReportResult(report = "next", columns = listOf(col("id")), rows = listOf(row()))
        assertTrue(cellsOf(result, ctx) === cellsOf(result, ctx.copy()))
        assertEquals("3", cellsOfRow(result, ctx, result.rows[0])[0].text)
        // A row that is not the result's is still answered.
        assertEquals("3", cellsOfRow(result, ctx, row())[0].text)
    }

    @Test
    fun urgencyIsColouredByHowPressingItIs() {
        assertEquals(
            listOf(UrgencyLevel.Low, UrgencyLevel.Mid, UrgencyLevel.High, UrgencyLevel.Crit),
            listOf(4.99, 5.0, 10.0, 15.0).map(::urgencyLevel),
        )
        assertEquals("urg-crit", cell(col("urgency"), row(urgency = 15.0), ctx).cls)
    }

    @Test
    fun aRowCarriesItsStatesAsClasses() {
        assertEquals("done completed", rowClass(row(status = "completed")))
        assertEquals("blocked active", rowClass(row(start = now, blocked = true)))
        assertEquals("", rowClass(row()))
    }

    @Test
    fun aColumnWithNothingInItIsLeftOutUnlessPrintEmptyColumnsIsOn() {
        val cols = listOf(col("description"), col("priority"), col("project"))
        val rows = listOf(row(), row(description = "b", project = "Home"))
        val result = ReportResult(report = "next", columns = cols, rows = rows)
        // The priority is empty in every row; the project is in one.
        assertEquals(listOf(0, 2), shownColumns(result, ctx))
        assertEquals(listOf(true, false), tableOf(result, ctx).empty.slice(1..2))
        assertEquals(listOf(0, 1, 2), shownColumns(result.copy(printEmptyColumns = true), ctx))
        // A table with no rows keeps its columns.
        assertEquals(listOf(0, 1, 2), shownColumns(result.copy(rows = emptyList()), ctx))
    }
}
