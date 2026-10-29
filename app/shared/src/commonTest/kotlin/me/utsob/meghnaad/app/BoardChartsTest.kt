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
}
