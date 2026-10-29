package me.utsob.meghnaad

import android.graphics.Bitmap
import android.graphics.Color
import androidx.test.core.app.ApplicationProvider
import me.utsob.meghnaad.app.BoardChart
import me.utsob.meghnaad.app.ChartLabel
import me.utsob.meghnaad.app.Dataset
import me.utsob.meghnaad.app.Section
import me.utsob.meghnaad.app.Series
import me.utsob.meghnaad.app.By
import me.utsob.meghnaad.app.TimeBy
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.io.File

/** The pictures of a board's cards, drawn for real (Android's graphics on the JVM) and written out so they can be looked at. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35])
class BoardRendererTest {
    private val context get() = ApplicationProvider.getApplicationContext<android.content.Context>()
    private val out = File(System.getProperty("board.widget.out") ?: "build/board-widgets").also { it.mkdirs() }

    private fun palette(dark: Boolean) = Palette(context, WidgetLook(if (dark) "dark" else "light"))

    private fun chart(labels: List<String>, vararg series: Pair<String, List<Double?>>) = BoardChart(
        "number",
        labels.mapIndexed { i, l -> ChartLabel("$i", l) },
        series.map { (name, data) -> Dataset(name, data, data.map { 1 }) },
    )

    private fun section(type: String, stacked: Boolean? = null, by: By? = By(field = "project"), series: List<Series> = emptyList()) =
        Section("s", "Card", 0, 0, 12, 4, type, by = by, stacked = stacked, series = series)

    /** How many pixels are not transparent. */
    private fun drawn(b: Bitmap): Int {
        var n = 0
        for (y in 0 until b.height step 2) for (x in 0 until b.width step 2) if (Color.alpha(b.getPixel(x, y)) > 0) n++
        return n
    }

    private fun save(name: String, b: Bitmap, dark: Boolean) {
        // A picture is transparent over the widget's own background: this one is put on a plain one to be looked at.
        val flat = Bitmap.createBitmap(b.width, b.height, Bitmap.Config.ARGB_8888)
        android.graphics.Canvas(flat).apply {
            drawColor(palette(dark).background)
            drawBitmap(b, 0f, 0f, null)
        }
        File(out, "$name-${if (dark) "dark" else "light"}.png").outputStream().use { flat.compress(Bitmap.CompressFormat.PNG, 100, it) }
    }

    private val projects = listOf("Home", "Work", "Play", "Garden", "Money")
    private val weeks = (1..8).map { "2026-09-${(it * 3).toString().padStart(2, '0')}" }

    @Test
    fun aNumberIsDrawnLargeWithItsTasksUnderIt() {
        for (dark in listOf(false, true)) {
            val b = BoardRenderer.number("12", 4, 480, 300, palette(dark))
            assertTrue(drawn(b) > 100)
            save("number", b, dark)
        }
        // A long figure is made to fit.
        assertTrue(drawn(BoardRenderer.number("1,234,567.89", 3, 300, 200, palette(false))) > 100)
    }

    @Test
    fun everyKindOfChartIsDrawn() {
        val one = chart(projects, "Tasks" to listOf(5.0, 3.0, 8.0, 1.0, 2.0))
        val two = chart(projects, "Done" to listOf(5.0, 3.0, 8.0, 1.0, null), "Open" to listOf(2.0, 4.0, 1.0, 6.0, 3.0))
        val time = chart(weeks, "Done" to listOf(1.0, 2.0, 3.0, 5.0, 4.0, 6.0, 8.0, 9.0), "Failed" to listOf(0.0, 1.0, 1.0, 2.0, 1.0, 1.0, 2.0, 1.0))
        val timeBy = By(time = TimeBy("end", "week"))
        val cases = listOf(
            "bar-horizontal" to (one to section("bar")),
            "bar-grouped" to (two to section("bar")),
            "bar-stacked" to (two to section("bar", stacked = true)),
            "bar-time-stacked" to (time to section("bar", stacked = true, by = timeBy)),
            "bar-time-grouped" to (time to section("bar", by = timeBy)),
            "line" to (time to section("line", by = timeBy)),
            "area-stacked" to (time to section("area", stacked = true, by = timeBy)),
            "pie" to (one to section("pie")),
            "donut" to (one to section("donut")),
            "polar" to (one to section("polar")),
            "radar" to (two to section("radar")),
            "heatmap" to (two to section("heatmap")),
            "heatmap-time" to (time to section("heatmap", by = timeBy)),
        )
        for ((name, pair) in cases) {
            for (dark in listOf(false, true)) {
                val b = BoardRenderer.chart(pair.first, pair.second, 640, 400, palette(dark))
                assertEquals(640, b.width)
                assertTrue("$name is blank", drawn(b) > 200)
                save(name, b, dark)
            }
        }
    }

    @Test
    fun aSeriesColourIsUsedWhereItIsGiven() {
        val c = chart(projects, "A" to listOf(5.0, 3.0, 8.0, 1.0, 2.0))
        val named = section("bar", series = listOf(Series("A", color = "#ff0000")))
        val b = BoardRenderer.chart(c, named, 400, 300, palette(false))
        var red = 0
        for (y in 0 until b.height) for (x in 0 until b.width) if (b.getPixel(x, y) == Color.rgb(255, 0, 0)) red++
        assertTrue("no red bars", red > 100)
    }

    @Test
    fun theValuesOfAUdaAreInTheColoursTheTaskrcGivesThem() {
        val c = chart(projects.take(3), "Tasks" to listOf(5.0, 3.0, 8.0))
        val red = androidx.compose.ui.graphics.Color(0xFFFF0000)
        fun count(b: android.graphics.Bitmap): Int {
            var n = 0
            for (y in 0 until b.height) for (x in 0 until b.width) if (b.getPixel(x, y) == Color.rgb(255, 0, 0)) n++
            return n
        }
        for (type in listOf("bar", "pie", "donut", "polar")) {
            val plain = BoardRenderer.chart(c, section(type), 400, 300, palette(false))
            val own = BoardRenderer.chart(c, section(type), 400, 300, palette(false), listOf(red, null, null))
            assertEquals("$type has red without being given any", 0, count(plain))
            assertTrue("$type has no red slice or bar", count(own) > 50)
        }
    }

    @Test
    fun aHeatmapWithRowsIsAGridForEachSeriesInItsColour() {
        fun grid(label: String) = chart(weeks, *listOf("Mon", "Tue", "Wed").map { it to weeks.indices.map { i -> (i * 2 % 5).toDouble() } }.toTypedArray()).copy(label = label)
        val section = section("heatmap", by = By(time = TimeBy("end", "week")), series = listOf(Series("A", color = "#ff0000"), Series("B", color = "#0000ff")))
        val grids = listOf(grid("A"), grid("B"))
        val b = BoardRenderer.chart(grids[0], section, 600, 400, palette(false), grids = grids)
        save("heatmap-grids", b, false)
        // The first series' grid is above the second's: red in the top half, blue in the bottom.
        fun count(top: Boolean, test: (Int) -> Boolean): Int {
            val rows = if (top) 0 until b.height / 2 else b.height / 2 until b.height
            return rows.sumOf { y -> (0 until b.width).count { x -> test(b.getPixel(x, y)) } }
        }
        val isRed = { px: Int -> Color.red(px) > 200 && Color.green(px) < 150 && Color.blue(px) < 150 }
        val isBlue = { px: Int -> Color.blue(px) > 200 && Color.red(px) < 150 && Color.green(px) < 150 }
        assertTrue("no red grid above", count(true, isRed) > 100)
        assertTrue("no blue grid below", count(false, isBlue) > 100)
        // A strong cell of each: the first series' grid (A) is the upper one, in its colour, and the second's the lower, in its.
        assertTrue("the upper grid is not red", isRed(b.getPixel(250, 60)))
        assertTrue("the lower grid is not blue", isBlue(b.getPixel(250, 260)))
    }

    @Test
    fun nothingOnTheAxisAndTooManyLabelsAreBothFine() {
        assertEquals(0, drawn(BoardRenderer.chart(BoardChart(), section("bar"), 300, 200, palette(false))))
        val many = chart((1..120).map { "w$it" }, "n" to (1..120).map { it.toDouble() })
        val b = BoardRenderer.chart(many, section("bar", by = By(time = TimeBy("end", "week"))), 600, 300, palette(false))
        assertTrue(drawn(b) > 200)
        assertNotNull(b)
        save("many", b, false)
    }
}
