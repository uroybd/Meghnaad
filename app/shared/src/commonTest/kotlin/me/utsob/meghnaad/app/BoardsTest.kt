package me.utsob.meghnaad.app

import kotlinx.serialization.json.Json
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

// The meaning of a board is the engine's (tc_core::board, tested there); these check that the app's side carries it across:
// the shapes, the nulls and the defaults, and the view logic it ports from web/src/lib/boards.ts.
class BoardsTest {
    private fun sec(id: String, type: String = "number", x: Int = 0, y: Int = 0, w: Int = 12, h: Int = 2, extra: (Section) -> Section = { it }) =
        extra(Section(id = id, title = id.uppercase(), x = x, y = y, w = w, h = h, type = type))

    private val zone = Zone(0, true, 1_792_000_000L)

    @Test
    fun theEngineTidiesBoardsAndNamesWhatItDrops() {
        val n = normalizeBoards(
            Json.parseToJsonElement(
                """[{"id":"week","name":"Week","sections":[{"id":"s","type":"number","x":0,"y":5,"w":3,"h":2}]},
                    {"id":"Bad Id","name":"x"}]""",
            ),
        )
        assertEquals(listOf("week"), n.boards.map { it.id })
        // Floated up to the first free row, with the default measure.
        assertEquals(0, n.boards[0].sections[0].y)
        assertEquals(Measure("count"), n.boards[0].sections[0].measure)
        assertEquals(1, n.errors.size)
    }

    @Test
    fun boardsWrittenOutAreReadBackTheSame() {
        val board = Board(
            "week", "Week", "What moved", "project:Home",
            listOf(
                sec("a", "bar") { it.copy(by = By(time = TimeBy("end", "week")), stacked = true, series = listOf(Series("A", "+x", color = "red"), Series("B", span = StateSpan("end")))) },
                sec("b", "number") { it.copy(measure = Measure("sum", "points")) },
            ),
        )
        val back = normalizeBoards(boardsToJson(listOf(board)))
        assertEquals(emptyList(), back.errors)
        assertEquals(board.sections.map { it.id }, back.boards[0].sections.map { it.id })
        assertEquals(board.sections[0].series, back.boards[0].sections[0].series)
        assertEquals(Measure("sum", "points"), back.boards[0].sections[1].measure)
        // Nothing that is a default is written.
        val text = boardsToJson(listOf(Board("x", "X", sections = listOf(sec("n"))))).toString()
        assertTrue("\"series\"" !in text && "\"description\"" !in text, text)
    }

    @Test
    fun aPhoneReadsSectionsTopToBottomLeftToRight() {
        val out = stacked(listOf(sec("c", y = 4), sec("b", x = 6, w = 6), sec("a", w = 6)))
        assertEquals(listOf("a", "b", "c"), out.map { it.id })
    }

    @Test
    fun aSlugIsOneWordAndNotInUse() {
        assertNull(slugProblem("q4-review_2", listOf("week")))
        assertTrue(slugProblem("", emptyList())!!.contains("needs"))
        assertTrue(slugProblem("Week", emptyList())!!.contains("lower case"))
        assertTrue(slugProblem("week", listOf("week"))!!.contains("already"))
    }

    @Test
    fun aSectionRunsTheBoardsFilterThenItsOwnThenASeriesEachInParentheses() {
        val s = sec("x") { it.copy(filter = "+a or +b") }
        val board = Board("b", "B", filter = "project:Home", sections = listOf(s))
        assertEquals("( project:Home ) ( +a or +b ) ( outcome:failed )", effectiveFilter(board, s, Series("f", "outcome:failed")))
        assertEquals("( +a or +b )", effectiveFilter(Board("b", "B"), s))
        // A section can leave the board's filter out.
        assertEquals("( +a or +b )", effectiveFilter(board, s.copy(ignoreBoardFilter = true)))
        assertEquals("", effectiveFilter(Board("b", "B", filter = "  "), sec("y")))
    }

    @Test
    fun aChartWithoutSeriesIsOneSeriesNamedAfterIt() {
        val s = sec("x", "bar") { it.copy(title = "Open", measure = Measure("sum", "points")) }
        assertEquals(listOf(Series("Open", measure = Measure("sum", "points"))), seriesOf(s))
        assertEquals(listOf("a", "b"), seriesOf(s.copy(series = listOf(Series("a"), Series("b")))).map { it.label })
    }

    private fun chart(vararg rows: Pair<String, List<Double?>>) = BoardChart(
        "number",
        rows.map { ChartLabel(it.first, it.first) },
        (0..1).map { d -> Dataset("s$d", rows.map { it.second.getOrNull(d) }, rows.map { 1 }) },
    )

    @Test
    fun theLargestTotalComesFirstAndNothingToMeasureLast() {
        val c = chart("b" to listOf(2.0, 1.0), "a" to listOf(5.0, 0.0), "c" to listOf(null, null), "d" to listOf(1.0, 1.0))
        val by = sec("x", "bar") { it.copy(by = By(field = "p")) }
        assertEquals(listOf("a", "b", "d", "c"), orderChart(c, by).labels.map { it.key })
        assertEquals(listOf("d", "b", "a", "c"), orderChart(c, by.copy(sort = "value+")).labels.map { it.key })
        val keyed = orderChart(c, by.copy(sort = "key-", limit = 2))
        assertEquals(listOf("d", "c"), keyed.labels.map { it.key })
        assertEquals(listOf(1.0, null), keyed.datasets[0].data)
        // A date stays in time order unless asked.
        val time = by.copy(by = By(time = TimeBy("due", "day")))
        assertEquals(listOf("b", "a", "c", "d"), orderChart(c, time).labels.map { it.key })
        // Keep takes the latest periods of a date in time order, and the first of any other order.
        assertEquals(listOf("c", "d"), orderChart(c, time.copy(limit = 2)).labels.map { it.key })
        assertEquals(listOf("d", "c"), orderChart(c, time.copy(limit = 2, sort = "key-")).labels.map { it.key })
        assertEquals(listOf("a", "b"), orderChart(c, time.copy(limit = 2, sort = "key+")).labels.map { it.key })
    }

    @Test
    fun aNumberAndAChartAreWorkedOutByTheEngine() {
        val tasks = """[{"project":"Home","points":3},{"project":"Home","points":4},{"project":"Work","points":1}]"""
        val sum = aggregateNumber(tasks, sec("n") { it.copy(measure = Measure("sum", "points")) }, zone)
        assertEquals(8.0, sum.total)
        assertEquals(3, sum.tasks)

        val inputs = listOf(
            SeriesInput("Points", tasks, Measure("sum", "points"), null),
            SeriesInput("Tasks", tasks, Measure("count"), null),
        )
        val c = layChart(inputs, By(field = "project"), zone)
        assertEquals(listOf("Home", "Work"), c.labels.map { it.label })
        assertEquals(listOf(7.0, 1.0), c.datasets[0].data)
        assertEquals(listOf(2.0, 1.0), c.datasets[1].data)
    }

    @Test
    fun aSeriesOfStatesCountsWhatIsOpenAtTheEndOfEachWeek() {
        val tasks = """[{"entry":"20261006T100000Z","start":"20261008T100000Z","end":"20261014T100000Z"},{"entry":"20261013T100000Z"}]"""
        val now = exportedStampEpoch("20261015T120000Z")!!
        val z = Zone(0, true, now)
        val c = layChart(
            listOf(
                SeriesInput("Pending", tasks, Measure("count"), StateSpan("entry", "end", "start")),
                SeriesInput("Started", tasks, Measure("count"), StateSpan("start", "end")),
                SeriesInput("Done", tasks, Measure("count"), StateSpan("end")),
            ),
            By(time = TimeBy("end", "week")),
            z,
        )
        assertEquals(listOf("2026-10-05", "2026-10-12"), c.labels.map { it.label })
        assertEquals(listOf(listOf(0.0, 1.0), listOf(1.0, 0.0), listOf(0.0, 1.0)), c.datasets.map { it.data })
    }

    @Test
    fun theZoneStartsTheWeekOnMondayOnlyWhenTheTaskrcSaysSo() {
        assertTrue(boardZone(mapOf("weekstart" to " Monday "), 3600, 5.9).weekStartsMonday)
        assertEquals(false, boardZone(mapOf("weekstart" to "sunday"), 0, 5.0).weekStartsMonday)
        assertEquals(false, boardZone(null, 0, 5.0).weekStartsMonday)
        assertEquals(5L, boardZone(null, 0, 5.9).now)
    }

    @Test
    fun valuesAreWrittenForAScreen() {
        assertEquals("12", formatBoardValue(12.0, "number"))
        assertEquals("3.14", formatBoardValue(3.14159, "number"))
        assertEquals("2h", formatBoardValue(7200.0, "duration"))
        assertEquals("–", formatBoardValue(null, "number"))
    }

    @Test
    fun whatACardAsksIsSaidInWords() {
        assertEquals("Count of tasks", describeMeasure(Measure("count")))
        assertEquals("Sum of points", describeMeasure(Measure("sum", "points")))
        assertEquals("Largest of urgency", describeMeasure(Measure("max", "urgency")))
        assertEquals("end, per week", describeBy(By(time = TimeBy("end", "week"))))
        assertEquals("project", describeBy(By(field = "project")))
        assertNull(describeBy(null))
        assertEquals("open at the end of each period, from entry until end, unless start has passed", describeSpan(StateSpan("entry", "end", "start")))
    }

    @Test
    fun anExportIsReadAsAnArrayOrOneTaskToALine() {
        assertEquals(2, parseExportedTasks("""[{"id":1},{"id":2}]""").size)
        assertEquals(2, parseExportedTasks("{\"id\":1}\n{\"id\":2}\n").size)
        assertEquals(0, parseExportedTasks("[\n]\n").size)
        assertEquals(0, parseExportedTasks("").size)
        assertEquals(2, parseExportedTasks("{\"id\":1}\nnot json\n{\"id\":3}").size)
    }

    @Test
    fun aDateIsReadAsExportWritesItAndNothingElseIsOne() {
        assertEquals(1_791_376_200L, exportedStampEpoch("20261007T123000Z"))
        assertEquals(0L, exportedStampEpoch("19700101T000000Z"))
        for (v in listOf("2026-10-07", "20261007", "Home", "")) assertNull(exportedStampEpoch(v), v)
    }

    @Test
    fun aCellShowsDatesInTheUsersFormatListsJoinedAndNothingAsADash() {
        val t = Json.parseToJsonElement("""{"id":0,"description":"x","tags":["a","b"],"due":"20261007T123000Z","project":null}""") as ExportedTask
        assertEquals("–", exportedCell(t, "id") { "d" })
        assertEquals("a, b", exportedCell(t, "tags") { "d" })
        assertEquals("<1791376200>", exportedCell(t, "due") { "<$it>" })
        assertEquals("–", exportedCell(t, "project") { "d" })
        assertEquals("–", exportedCell(t, "nope") { "d" })
        assertEquals("x", exportedCell(t, "description") { "d" })
    }
}
