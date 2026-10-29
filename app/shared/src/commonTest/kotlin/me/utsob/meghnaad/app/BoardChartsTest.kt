package me.utsob.meghnaad.app

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

// What a chart is drawn from (web/src/lib/colors.ts and boardChart.ts): the colours a series takes, and the numbers behind the axes.
class BoardChartsTest {
    @Test
    fun aColourNameIsTheBasicColourInThePalette() {
        assertEquals(paletteColor(1, false), namedColor("red", false))
        assertEquals(paletteColor(12, true), namedColor(" Bright Blue ", true))
        assertNotEquals(namedColor("red", true), namedColor("red", false))
        assertNull(namedColor("puce", false))
    }

    @Test
    fun blackAndWhiteAreNotOfferedBecauseTheyWouldVanishOnOnePage() {
        assertEquals(12, CHART_COLOR_NAMES.size)
        assertTrue("black" !in CHART_COLOR_NAMES && "white" !in CHART_COLOR_NAMES)
        for (n in CHART_COLOR_NAMES) assertTrue(namedColor(n, false) != null, n)
    }

    @Test
    fun aValueIsReadWithOrWithoutAnOpacityAndNothingElseIsOne() {
        assertEquals(Color(0xFF2DA44E), hexColor("#2da44e"))
        assertEquals(Color(0xFFAABBCC), hexColor("#abc"))
        assertEquals(Color(0x402DA44E), hexColor("#2da44e40"))
        for (bad in listOf("2da44e", "#abcd", "#ggg", "", "red")) assertNull(hexColor(bad), bad)
    }

    @Test
    fun aChartColourIsANameOrAValueAndNothingElse() {
        assertEquals(paletteColor(2, false), chartColor("green", false))
        assertEquals(Color(0xFF2DA44E), chartColor("#2da44e", true))
        assertNull(chartColor("puce", false))
        assertNull(chartColor(null, false))
        assertNull(chartColor("  ", false))
    }

    @Test
    fun aSeriesTakesTheDefaultsInTurnStartingWithBlueAndRoundAgainAfterTheLast() {
        assertEquals(namedColor("blue", false), defaultSeriesColor(0, false))
        assertEquals("red", defaultSeriesName(1))
        assertEquals(defaultSeriesColor(0, false), defaultSeriesColor(9, false))
    }

    private fun chart(vararg rows: List<Double?>) = BoardChart(
        "number",
        rows.indices.map { ChartLabel("$it", "L$it") },
        (0..1).map { d -> Dataset("s$d", rows.map { it.getOrNull(d) }) },
    )

    @Test
    fun anAxisTopIsOneTwoOrFiveTimesAPowerOfTen() {
        assertEquals(1f, niceTop(0.0))
        assertEquals(1f, niceTop(Double.NaN))
        assertEquals(1f, niceTop(0.7))
        assertEquals(2f, niceTop(1.3))
        assertEquals(5f, niceTop(3.0))
        assertEquals(10f, niceTop(7.0))
        assertEquals(50f, niceTop(41.0))
        assertEquals(200f, niceTop(120.0))
    }

    @Test
    fun stackedBarsAddUpAndNothingToMeasureAddsNothing() {
        val c = chart(listOf(4.0, 3.0), listOf(1.0, null), listOf(null, null))
        assertEquals(listOf(listOf(4.0, 7.0), listOf(1.0, 1.0), listOf(0.0, 0.0)), stackedValues(c))
        // The top is the tallest stack when stacked, and the largest value when not.
        assertEquals(10f, chartTop(c, stacked = true))
        assertEquals(5f, chartTop(c, stacked = false))
    }

    @Test
    fun aSeriesColourIsItsOwnOrThePalettes() {
        val c = chart(listOf(1.0, 1.0))
        val section = Section("x", "X", 0, 0, 6, 4, "bar", series = listOf(Series("a", color = "red"), Series("b")))
        assertEquals(listOf(namedColor("red", false), defaultSeriesColor(1, false)), seriesColors(c, section, false))
        // A section without series has the palette's blue.
        assertEquals(listOf(defaultSeriesColor(0, true), defaultSeriesColor(1, true)), seriesColors(c, section.copy(series = emptyList()), true))
    }

    @Test
    fun theSlicesOfAPieAddUpToAFullCircleAndNothingSweepsNothing() {
        val sweeps = pieSweeps(listOf(1.0, 1.0, 2.0, 0.0, -3.0))
        assertEquals(listOf(90f, 90f, 180f, 0f, 0f), sweeps)
        assertEquals(360f, sweeps.sum())
        assertEquals(listOf(0f, 0f), pieSweeps(listOf(0.0, 0.0)))
        assertEquals(emptyList(), pieSweeps(emptyList()))
    }

    @Test
    fun anAxisKeepsOnlyAsManyLabelsAsFitWithoutRunningTogether() {
        // Ten dates of ten characters, as on a phone: a label takes about 65 dp, so a 330 dp axis has room for five of them.
        assertEquals(2, labelSkip(10, 10, 330.dp))
        assertEquals(1, labelSkip(5, 10, 330.dp))
        assertEquals(3, labelSkip(3, 10, 40.dp))
        assertEquals(10, labelSkip(10, 10, 40.dp))
        assertEquals(1, labelSkip(0, 10, 330.dp))
    }

    @Test
    fun aTapSaysTheLabelInFullAndTheValueWithTheSeriesWhenThereAreSeveral() {
        val one = BoardChart("number", listOf(ChartLabel("a", "2026-10-12")), listOf(Dataset("Pending", listOf(4.0))))
        assertEquals("2026-10-12 · 4", tipText(one, 0, 0))
        val two = one.copy(datasets = listOf(Dataset("Succeeded", listOf(1.0)), Dataset("Failed", listOf(2.0))))
        assertEquals("2026-10-12 · Failed: 2", tipText(two, 0, 1))
    }

    private val grid = BoardChart(
        "number",
        listOf(ChartLabel("a", "Home"), ChartLabel("b", "Work"), ChartLabel("c", "(none)")),
        listOf(Dataset("s0", listOf(1.0, 3.0, null)), Dataset("s1", listOf(2.0, 3.0, null))),
    )

    @Test
    fun aHeatmapCellIsAsStrongAsItsValueIsLargeOverTheRangeFromZero() {
        val strengths = heatStrengths(grid)
        // A row for each series and a column for each label; the largest value is full, and a gap stays a gap.
        assertEquals(listOf(listOf(1f / 3f, 1f, null), listOf(2f / 3f, 1f, null)), strengths)
        assertEquals(listOf(listOf(0f, 0f), listOf(0f, 0f)), heatStrengths(BoardChart("number", grid.labels.take(2), listOf(Dataset("a", listOf(0.0, 0.0)), Dataset("b", listOf(0.0, 0.0))))))
        assertEquals(emptyList(), heatStrengths(BoardChart()))
        // A value short of its column's label has nothing to measure.
        assertEquals(listOf(listOf(1f, null, null)), heatStrengths(grid.copy(datasets = listOf(Dataset("s", listOf(5.0))))))
    }

    @Test
    fun aHeatmapCellGoesFromAPaleWashToTheFullColour() {
        assertEquals(0x26 / 255f, heatAlpha(0f))
        assertEquals(1f, heatAlpha(1f))
        assertTrue(heatAlpha(0.5f) > heatAlpha(0.25f))
    }

    @Test
    fun theValuesOfAUdaTakeTheColoursTheTaskrcGivesThem() {
        val labels = listOf("H", "M", "L", "", "X").map { ChartLabel(it, it.ifEmpty { "(none)" }) }
        val colors = mapOf(
            "uda.priority.H" to Resolved(fg = 9),
            "uda.priority.M" to Resolved(bg = 4),
            "uda.priority.none" to Resolved(fg = 2),
            // A rule for every value of a UDA, and a project's, are not the colour of a value.
            "uda.priority" to Resolved(fg = 5),
            "project.Home" to Resolved(fg = 6),
        )
        val page = Color.White
        val got = valueColors("priority", labels, colors, false, page)
        assertEquals(paletteColor(9, false), got[0])
        assertEquals(paletteColor(4, false), got[1])
        // A task with none of it takes the `none` rule's; a value with no rule keeps the chart's own colour.
        assertEquals(paletteColor(2, false), got[3])
        assertEquals(listOf<Color?>(null, null), listOf(got[2], got[4]))
        assertTrue(valueColors("project", labels, colors, false, page).all { it == null })
        assertTrue(valueColors(null, labels, colors, false, page).all { it == null })
        assertTrue(valueColors("priority", labels, emptyMap(), false, page).all { it == null })
        // What the page would swallow is moved toward readable.
        val white = valueColors("priority", listOf(ChartLabel("H", "H")), mapOf("uda.priority.H" to Resolved(fg = 15)), false, page)[0]!!
        assertTrue(contrast(white, page) >= 1.8)
    }

    @Test
    fun aBarOfOneSeriesHasItsValuesColourAndSeveralSeriesKeepTheirOwn() {
        val one = BoardChart(labels = listOf(ChartLabel("H", "H"), ChartLabel("M", "M")), datasets = listOf(Dataset("n", listOf(1.0, 2.0))))
        val series = listOf(Color.Blue, Color.Green)
        val own = listOf<Color?>(Color.Red, null)
        assertEquals(Color.Red, barColor(one, series, own, 0, 0))
        assertEquals(Color.Blue, barColor(one, series, own, 0, 1))
        val two = one.copy(datasets = one.datasets + Dataset("m", listOf(3.0, 4.0)))
        assertEquals(Color.Green, barColor(two, series, own, 1, 0))
    }

    @Test
    fun colorsAreMixedInTheProportionsOfTheirWeights() {
        val red = Color(0xFFFF0000)
        val blue = Color(0xFF0000FF)
        assertEquals(red, blendColors(listOf(red, blue), listOf(5.0, 0.0)))
        assertEquals(blue, blendColors(listOf(red, blue), listOf(0.0, 2.0)))
        assertEquals(Color(0.75f, 0f, 0.25f), blendColors(listOf(red, blue), listOf(3.0, 1.0)))
        // With no weight, the first; nothing to mix is grey.
        assertEquals(red, blendColors(listOf(red, blue), listOf(0.0, 0.0)))
        assertEquals(Color.Gray, blendColors(emptyList(), emptyList()))
    }

    @Test
    fun aHeatmapCellIsInTheColourOfItsSeriesAndAStackedCellInTheirs() {
        val red = Color(0xFFFF0000)
        val blue = Color(0xFF0000FF)
        val labels = listOf(ChartLabel("a", "a"), ChartLabel("b", "b"))
        val rows = BoardChart(labels = labels, datasets = listOf(Dataset("A", listOf(1.0, 2.0)), Dataset("B", listOf(3.0, 4.0))))
        // A row for each series: each row in its series' colour. Otherwise, the first.
        assertEquals(blue, heatCellColor(rows, 1, 0, listOf(red, blue), rowsAreSeries = true))
        assertEquals(red, heatCellColor(rows, 1, 0, listOf(red, blue), rowsAreSeries = false))
        // A stacked cell: the colours of what it is made of, in the proportions of what each had.
        val stacked = BoardChart(
            labels = labels,
            datasets = listOf(Dataset("A / B", listOf(4.0, 2.0), parts = listOf(Share("A", listOf(3.0, 0.0)), Share("B", listOf(1.0, 2.0))))),
        )
        assertEquals(Color(0.75f, 0f, 0.25f), heatCellColor(stacked, 0, 0, listOf(red, blue), false))
        assertEquals(blue, heatCellColor(stacked, 0, 1, listOf(red, blue), false))
        // And a tap says what each had.
        assertEquals("a · 4 (A 3, B 1)", tipText(stacked, 0, 0))
        assertEquals("b · 2 (A 0, B 2)", tipText(stacked, 1, 0))
        assertEquals("a · A: 1", tipText(rows, 0, 0))
    }

    @Test
    fun theColoursOfAHeatmapFollowItsSeriesAndItsRows() {
        val series = listOf(Series("A", color = "#ff0000"), Series("B", color = "#0000ff"))
        val grid = BoardChart(datasets = listOf(Dataset("x", emptyList()), Dataset("y", emptyList())))
        val red = Color(0xFFFF0000)
        val blue = Color(0xFF0000FF)
        fun section(rows: Rows?, stacked: Boolean?) = Section("h", "H", 0, 0, 6, 4, "heatmap", series = series, rows = rows, stacked = stacked)
        val rows = Rows(time = PartBy("end", "weekday"))
        // With no rows the series are the rows, each in its colour; stacked they are one row, mixed.
        assertTrue(heatRowsAreSeries(section(null, null)))
        assertEquals(listOf(red, blue), heatColors(section(null, null), listOf(grid), 0, false))
        assertTrue(!heatRowsAreSeries(section(null, true)))
        // With rows each series is a grid of its own colour, or stacked all of them are in one.
        assertEquals(listOf(blue), heatColors(section(rows, null), listOf(grid, grid), 1, false))
        assertEquals(listOf(red, blue), heatColors(section(rows, true), listOf(grid), 0, false))
    }
}
