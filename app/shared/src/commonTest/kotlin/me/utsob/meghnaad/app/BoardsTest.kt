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
    fun theEngineKeepsATidyDateRangeAndNamesOneItDrops() {
        val n = normalizeBoards(
            Json.parseToJsonElement(
                """[{"id":"b","name":"B","ranges":[{"name":" week ","start":" sow ","end":"eow"},{"name":"Bad","start":"sow","end":"eow"}]}]""",
            ),
        )
        assertEquals(listOf(DateRange("week", "sow", "eow")), n.boards[0].ranges)
        assertEquals(1, n.errors.size)
        // Written out and read again it is the same, and none is not written.
        assertEquals(n.boards, normalizeBoards(boardsToJson(n.boards)).boards)
        assertTrue("ranges" !in boardsToJson(listOf(Board("b", "B"))).toString())
    }

    @Test
    fun aDateRangeIsWrittenIntoAFilterAsTheMomentsItMeans() {
        val ranges = listOf(DateRange("daterange", "sow", "eow"))
        // 2026-10-07 12:30 UTC, a Wednesday, weeks from Monday.
        val z = Zone(0, true, 1_791_376_200L)
        assertEquals(
            "( end.after:20261005T000000Z and end.before:20261011T235959Z )",
            substituteRanges("( end.after:\$daterange.start and end.before:\$daterange.end )", ranges, z),
        )
        assertEquals("description~foo\$", substituteRanges("description~foo\$", ranges, z))
        assertEquals(
            "this board has no date range called 'nope'",
            runCatching { substituteRanges("due:\$nope.start", ranges, z) }.exceptionOrNull()?.message?.substringAfter("message: ")?.trim('"'),
        )
        val got = resolveRanges(ranges, z).single()
        assertEquals(1_791_158_400L to 1_791_763_199L, got.start to got.end)
    }

    @Test
    fun keepIsANumberOrTheCountOfARangeAndIsWrittenAsOne() {
        val ranges = listOf(DateRange("daterange", "sow", "eow"))
        val z = Zone(0, true, 1_791_376_200L)
        assertEquals(7, keepOf("7", ranges, z))
        assertEquals(7, keepOf(" \$daterange.day_count ", ranges, z))
        assertEquals(168, keepOf("\$daterange.hour_count", ranges, z))
        assertNull(keepOf(null, ranges, z))
        assertNull(keepOf("0", ranges, z))
        assertTrue(runCatching { keepOf("\$nope.day_count", ranges, z) }.isFailure)
        // Read from a file and written back: a number stays a number, a count stays text.
        val n = normalizeBoards(
            Json.parseToJsonElement(
                """[{"id":"b","name":"B","ranges":[{"name":"week","start":"sow","end":"eow"}],"sections":[
                    {"id":"a","type":"bar","x":0,"y":0,"w":6,"h":4,"by":{"time":{"field":"end","step":"day"}},"limit":"${'$'}week.day_count"},
                    {"id":"c","type":"bar","x":6,"y":0,"w":6,"h":4,"by":{"field":"project"},"limit":5}]}]""",
            ),
        )
        assertEquals(listOf("\$week.day_count", "5"), n.boards[0].sections.map { it.limit })
        val out = boardsToJson(n.boards).toString()
        assertTrue("\"limit\":5" in out && "\"limit\":\"\$week.day_count\"" in out, out)
    }

    @Test
    fun aHeatmapIsGridsOfRowsByColumnsAndSeriesAreTheirColours() {
        val z = Zone(0, true, 1_791_376_200L)
        val tasks = """[{"end":"20261005T090000Z"},{"end":"20261005T150000Z"},{"end":"20261006T090000Z"},{"end":"20261012T090000Z"}]"""
        val by = By(time = TimeBy("end", "week"))
        val rows = Rows(time = PartBy("end", "weekday"))
        val two = listOf(SeriesInput("A", tasks, Measure("count"), null), SeriesInput("B", tasks, Measure("count"), null))
        val apart = layHeatmap(two, by, rows, false, z)
        assertEquals(listOf("A", "B"), apart.map { it.label })
        assertEquals(listOf("2026-10-05", "2026-10-12"), apart[0].labels.map { it.label })
        assertEquals(listOf("Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"), apart[0].datasets.map { it.label })
        assertEquals(listOf(2.0, 1.0), apart[0].datasets[0].data)
        // Stacked they are one grid, added up.
        val piled = layHeatmap(two, by, rows, true, z)
        assertEquals(1, piled.size)
        assertEquals(listOf(4.0, 2.0), piled[0].datasets[0].data)
        // The rows may be the values of an attribute.
        val byValue = layHeatmap(two.take(1), by, Rows(field = "project"), false, z)
        assertEquals(1, byValue.size)
        assertEquals(listOf("(none)"), byValue[0].datasets.map { it.label })
        // With no rows a series is a row of one grid, and stacked the series are one row.
        val flat = layHeatmap(two, by, null, false, z)
        assertEquals(listOf("A", "B"), flat.single().datasets.map { it.label })
        val line = layHeatmap(two, by, null, true, z).single()
        assertEquals(listOf("A + B"), line.datasets.map { it.label })
        // Three tasks in the first week and one in the second, in each of the two series.
        assertEquals(listOf(6.0, 2.0), line.datasets[0].data)
        assertEquals("weekday of end", describeRows(rows))
        assertEquals("the value of project", describeRows(Rows(field = "project")))
        assertNull(describeRows(null))
        // They are written as the engine reads them, and a section that is not a heatmap may not have them.
        val board = Board("b", "B", sections = listOf(sec("h", "heatmap") { it.copy(by = by, rows = rows) }))
        val back = normalizeBoards(boardsToJson(listOf(board)))
        assertEquals(emptyList(), back.errors)
        assertEquals(rows, back.boards[0].sections[0].rows)
        val bad = Board("b", "B", sections = listOf(sec("h", "bar") { it.copy(by = By(field = "project"), rows = rows) }))
        assertEquals(1, normalizeBoards(boardsToJson(listOf(bad))).errors.size)
    }

    @Test
    fun theColumnsOfEveryGridAreOrderedTogetherSoTheyLineUp() {
        fun grid(label: String, vararg values: Double) = BoardChart(
            labels = listOf("a", "b", "c").map { ChartLabel(it, it) },
            datasets = listOf(Dataset("row", values.toList(), values.map { it.toInt() })),
            label = label,
        )
        val section = sec("h", "heatmap") { it.copy(by = By(field = "project"), sort = "value-") }
        // By what the grids add up to: b (9), then c (4), then a (3).
        val ordered = orderGrids(listOf(grid("x", 1.0, 5.0, 4.0), grid("y", 2.0, 4.0, 0.0)), section, null)
        assertEquals(listOf(listOf("b", "c", "a"), listOf("b", "c", "a")), ordered.map { g -> g.labels.map { it.key } })
        assertEquals(listOf(4.0, 0.0, 2.0), ordered[1].datasets[0].data)
        assertEquals("y", ordered[1].label)
        assertEquals(2, orderGrids(ordered, section, 2)[0].labels.size)
        assertEquals(emptyList(), orderGrids(emptyList(), section, null))
    }

    @Test
    fun aChartMayCoverARangeOfTheBoardOrOneOfItsOwn() {
        val ranges = listOf(DateRange("week", "sow", "eow"))
        val named = By(time = TimeBy("end", "day", AxisRange.Named("week")))
        val own = By(time = TimeBy("end", "day", AxisRange.Own("2026-10-05", "2026-10-06T23:59")))
        // Written as the engine reads them (a name, or two dates), and read again the same.
        for (by in listOf(named, own)) {
            val board = Board("b", "B", ranges = ranges, sections = listOf(sec("a", "line") { it.copy(by = by) }))
            val back = normalizeBoards(boardsToJson(listOf(board)))
            assertEquals(emptyList(), back.errors)
            assertEquals(by, back.boards[0].sections[0].by)
        }
        assertTrue("\"range\":\"week\"" in boardsToJson(listOf(Board("b", "B", ranges = ranges, sections = listOf(sec("a", "line") { it.copy(by = named) })))).toString())
        // A range the board has not is refused.
        val bad = Board("b", "B", sections = listOf(sec("a", "line") { it.copy(by = named) }))
        assertEquals(1, normalizeBoards(boardsToJson(listOf(bad))).errors.size)

        // Wednesday 2026-10-07: a burndown over the week has a day for each of its seven days, and nothing after today.
        val z = Zone(0, true, 1_791_376_200L)
        val tasks = """[{"entry":"20260930T100000Z","end":"20261006T100000Z"},{"entry":"20261006T150000Z"},{"entry":"20261007T090000Z"}]"""
        val pending = SeriesInput("Pending", tasks, Measure("count"), StateSpan("entry", "end"))
        val c = layChart(listOf(pending), named, z, ranges)
        assertEquals(7, c.labels.size)
        assertEquals(listOf(1.0, 1.0, 2.0, null, null, null, null), c.datasets[0].data)
        assertEquals(listOf("2026-10-05", "2026-10-06"), layChart(listOf(pending), own, z).labels.map { it.label })
        // The step the axis is resolved to keeps the range.
        assertEquals(By(time = TimeBy("end", "day", AxisRange.Named("week"))), resolveBy(By(time = TimeBy("end", "\$week.lg_group.name", AxisRange.Named("week"))), ranges, z))
        assertEquals("end, per day, over Week", describeBy(named))
        assertEquals("end, per day, over 2026-10-05 to 2026-10-06T23:59", describeBy(own))
    }

    @Test
    fun aChartMayBeCutByTheLargestGroupOfARange() {
        val z = Zone(0, true, 1_791_376_200L)
        val week = listOf(DateRange("daterange", "sow", "eow"))
        assertEquals(mapOf("daterange" to "day"), largestGroups(week, z))
        assertEquals(mapOf("m" to "week"), largestGroups(listOf(DateRange("m", "som", "eom"), DateRange("x", "later", "eom")), z))
        assertEquals(7, keepOf("\$daterange.lg_group.count", week, z))
        val by = By(time = TimeBy("end", "\$daterange.lg_group.name"))
        assertEquals(By(time = TimeBy("end", "day")), resolveBy(by, week, z))
        assertEquals(By(time = TimeBy("end", "week")), resolveBy(By(time = TimeBy("end", "week")), week, z))
        assertEquals(By(field = "project"), resolveBy(By(field = "project"), week, z))
        // Kept as typed in the file, and read back the same.
        val n = normalizeBoards(
            Json.parseToJsonElement(
                """[{"id":"b","name":"B","ranges":[{"name":"daterange","start":"sow","end":"eow"}],"sections":[
                    {"id":"a","type":"bar","x":0,"y":0,"w":6,"h":4,"by":{"time":{"field":"end","step":"${'$'}daterange.lg_group.name"}}}]}]""",
            ),
        )
        assertEquals(by, n.boards[0].sections[0].by)
    }

    @Test
    fun theDatesTypedOnThePageTakeThePlaceOfTheBoardsOwn() {
        val b = Board("b", "B", filter = "a", ranges = listOf(DateRange("sprint_week", "sow", "eow")))
        assertEquals(b, viewedBoard(b, null, null))
        val v = viewedBoard(b, "x", mapOf("sprint_week" to DateRange("sprint_week", "som", "eom")))
        assertEquals("x", v.filter)
        assertEquals(listOf(DateRange("sprint_week", "som", "eom")), v.ranges)
        assertEquals("Sprint week", rangeLabel("sprint_week"))
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
        val keyed = orderChart(c, by.copy(sort = "key-", limit = "2"))
        assertEquals(listOf("d", "c"), keyed.labels.map { it.key })
        assertEquals(listOf(1.0, null), keyed.datasets[0].data)
        // A date stays in time order unless asked.
        val time = by.copy(by = By(time = TimeBy("due", "day")))
        assertEquals(listOf("b", "a", "c", "d"), orderChart(c, time).labels.map { it.key })
        // Keep takes the latest periods of a date in time order, and the first of any other order.
        assertEquals(listOf("c", "d"), orderChart(c, time.copy(limit = "2")).labels.map { it.key })
        assertEquals(listOf("d", "c"), orderChart(c, time.copy(limit = "2", sort = "key-")).labels.map { it.key })
        assertEquals(listOf("a", "b"), orderChart(c, time.copy(limit = "2", sort = "key+")).labels.map { it.key })
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
