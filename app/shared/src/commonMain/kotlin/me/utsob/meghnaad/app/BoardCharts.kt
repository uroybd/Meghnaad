package me.utsob.meghnaad.app

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.koalaplot.core.Symbol
import io.github.koalaplot.core.bar.GroupedHorizontalBarPlot
import io.github.koalaplot.core.bar.GroupedVerticalBarPlot
import io.github.koalaplot.core.bar.StackedHorizontalBarPlot
import io.github.koalaplot.core.bar.StackedVerticalBarPlot
import io.github.koalaplot.core.bar.horizontalSolidBar
import io.github.koalaplot.core.bar.verticalSolidBar
import io.github.koalaplot.core.line.AreaBaseline
import io.github.koalaplot.core.line.AreaPlot
import io.github.koalaplot.core.line.LinePlot
import io.github.koalaplot.core.pie.DefaultSlice
import io.github.koalaplot.core.pie.PieChart
import io.github.koalaplot.core.polar.PolarGraph
import io.github.koalaplot.core.polar.PolarPlotSeries
import io.github.koalaplot.core.polar.PolarPoint
import io.github.koalaplot.core.polar.rememberCategoryAngularAxisModel
import io.github.koalaplot.core.polar.rememberFloatRadialAxisModel
import io.github.koalaplot.core.style.AreaStyle
import io.github.koalaplot.core.style.LineStyle
import io.github.koalaplot.core.util.ExperimentalKoalaPlotApi
import io.github.koalaplot.core.util.HoverableElementAreaScope
import io.github.koalaplot.core.xygraph.CategoryAxisModel
import io.github.koalaplot.core.xygraph.Point
import io.github.koalaplot.core.xygraph.XYGraph
import io.github.koalaplot.core.xygraph.rememberFloatLinearAxisModel
import kotlin.math.floor
import kotlin.math.log10
import kotlin.math.pow

// A board section's chart (web/src/lib/BoardChart.svelte): the engine's labels and datasets drawn by Koala Plot, a Compose
// Multiplatform chart library, so the phone and the desktop preview draw them with the same code. The web draws the same data with
// Chart.js. A polar area chart, which Koala Plot has no plot for, is drawn as a pie.

/** The top of an axis for values up to [max]: 1, 2 or 5 times a power of ten. */
fun niceTop(max: Double): Float {
    if (max <= 0.0 || max.isNaN()) return 1f
    val e = 10.0.pow(floor(log10(max)))
    val f = max / e
    return ((if (f <= 1) 1 else if (f <= 2) 2 else if (f <= 5) 5 else 10) * e).toFloat()
}

/** The angle each slice of a pie sweeps, in degrees, for these values: nothing (or less) sweeps nothing, and the rest add up to 360. */
fun pieSweeps(values: List<Double>): List<Float> {
    val total = values.sumOf { it.coerceAtLeast(0.0) }
    return values.map { if (total <= 0.0) 0f else (it.coerceAtLeast(0.0) / total * 360.0).toFloat() }
}

/** The colours of a chart's series (or, for a round chart, of its slices), by position. */
fun seriesColors(chart: BoardChart, section: Section, dark: Boolean): List<Color> =
    chart.datasets.indices.map { i -> chartColor(section.series.getOrNull(i)?.color, dark) ?: defaultSeriesColor(i, dark) }

/** What each series adds up to at each label, from the bottom (or the left) up: the heights of stacked bars and lines. */
fun stackedValues(chart: BoardChart): List<List<Double>> =
    chart.labels.indices.map { i ->
        var sum = 0.0
        chart.datasets.map { d -> sum += d.data.getOrNull(i) ?: 0.0; sum }
    }

/** The top of the value axis: the tallest stack when stacked, else the largest value. */
fun chartTop(chart: BoardChart, stacked: Boolean): Float {
    val tallest = if (stacked) stackedValues(chart).maxOfOrNull { it.lastOrNull() ?: 0.0 } else chart.datasets.maxOfOrNull { d -> d.data.maxOfOrNull { it ?: 0.0 } ?: 0.0 }
    return niceTop(tallest ?: 0.0)
}

@OptIn(ExperimentalKoalaPlotApi::class, ExperimentalLayoutApi::class)
@Composable
fun BoardChartView(chart: BoardChart, section: Section, dark: Boolean, modifier: Modifier = Modifier, height: Dp? = null) {
    val colors = MaterialTheme.colorScheme
    if (chart.labels.isEmpty()) {
        Dim("Nothing to show.", modifier)
        return
    }
    val stacked = section.stacked == true
    val series = seriesColors(chart, section, dark)
    val round = section.type == "pie" || section.type == "donut" || section.type == "polar"
    val legend = !round && chart.datasets.size > 1
    // A chart in a card that has a size fills what the legend leaves; one without is as tall as it likes.
    val plot = height?.let { (it - if (legend) 28.dp else 0.dp).coerceAtLeast(96.dp) }
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        // A legend names the series; a round chart's names its slices instead.
        if (legend) Legend(chart.datasets.map { it.label }, series)
        when (section.type) {
            "bar" -> if (section.by?.time != null) VerticalBars(chart, series, stacked, plot) else HorizontalBars(chart, series, stacked, plot)
            "line", "area" -> Lines(chart, series, stacked, section.type == "area", plot)
            "radar" -> Radar(chart, series, plot)
            else -> Slices(chart, section, dark, plot)
        }
    }
}

@Composable
private fun Legend(labels: List<String>, colors: List<Color>) {
    @OptIn(ExperimentalLayoutApi::class)
    FlowRow(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
        labels.forEachIndexed { i, label ->
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                Box(Modifier.size(10.dp).background(colors[i], RoundedCornerShape(2.dp)))
                Text(label, fontSize = 12.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

/** The labels along an axis of categories: the label at each place, every [skip]-th. Typed, so the library's overloads are told apart. */
private fun indexLabels(chart: BoardChart, skip: Int, width: Int): @Composable (Int) -> Unit = { i ->
    if (i % skip == 0) AxisText(chart.labels[i].label.take(width))
}

/** The labels along an axis of values. */
private fun valueLabels(chart: BoardChart): @Composable (Float) -> Unit = { v -> AxisText(formatBoardValue(v.toDouble(), chart.unit)) }

@Composable
private fun AxisText(text: String) = Text(text, fontSize = 10.sp, maxLines = 1, color = MaterialTheme.colorScheme.onSurfaceVariant)

/** Every n-th label of a long axis, so they never run together. */
private fun every(count: Int) = maxOf(1, (count + 5) / 6)

@OptIn(ExperimentalKoalaPlotApi::class)
@Composable
private fun VerticalBars(chart: BoardChart, series: List<Color>, stacked: Boolean, plot: Dp?) {
    val at = chart.labels.indices.toList()
    val top = chartTop(chart, stacked)
    val skip = every(at.size)
    XYGraph(
        xAxisModel = CategoryAxisModel(at),
        yAxisModel = rememberFloatLinearAxisModel(0f..top),
        xAxisLabels = indexLabels(chart, skip, 10),
        yAxisLabels = valueLabels(chart),
        modifier = Modifier.fillMaxWidth().height(plot ?: 240.dp),
    ) {
        if (stacked) {
            StackedVerticalBarPlot {
                chart.datasets.forEachIndexed { d, ds ->
                    series(verticalSolidBar(series[d])) { at.forEach { i -> item(i, (ds.data.getOrNull(i) ?: 0.0).toFloat()) } }
                }
            }
        } else {
            GroupedVerticalBarPlot {
                chart.datasets.forEachIndexed { d, ds ->
                    series(verticalSolidBar(series[d])) { at.forEach { i -> item(i, 0f, (ds.data.getOrNull(i) ?: 0.0).toFloat()) } }
                }
            }
        }
    }
}

/** Bars that lie down, for a category: its names are long. */
@OptIn(ExperimentalKoalaPlotApi::class)
@Composable
private fun HorizontalBars(chart: BoardChart, series: List<Color>, stacked: Boolean, plot: Dp?) {
    // The first label at the top: the axis runs upwards.
    val at = chart.labels.indices.toList().reversed()
    val top = chartTop(chart, stacked)
    val perLabel = if (stacked) 1 else chart.datasets.size
    XYGraph(
        xAxisModel = rememberFloatLinearAxisModel(0f..top),
        yAxisModel = CategoryAxisModel(at),
        xAxisLabels = valueLabels(chart),
        yAxisLabels = indexLabels(chart, 1, 14),
        modifier = Modifier.fillMaxWidth().height(maxOf((60 + at.size * 26 * perLabel).coerceIn(160, 600).dp, plot ?: 0.dp)),
    ) {
        if (stacked) {
            StackedHorizontalBarPlot {
                chart.datasets.forEachIndexed { d, ds ->
                    series(horizontalSolidBar(series[d])) { at.forEach { i -> item((ds.data.getOrNull(i) ?: 0.0).toFloat(), i) } }
                }
            }
        } else {
            GroupedHorizontalBarPlot {
                chart.datasets.forEachIndexed { d, ds ->
                    series(horizontalSolidBar(series[d])) { at.forEach { i -> item(i, 0f, (ds.data.getOrNull(i) ?: 0.0).toFloat()) } }
                }
            }
        }
    }
}

@OptIn(ExperimentalKoalaPlotApi::class)
@Composable
private fun Lines(chart: BoardChart, series: List<Color>, stacked: Boolean, filled: Boolean, plot: Dp?) {
    val at = chart.labels.indices.toList()
    val top = chartTop(chart, stacked)
    val sums = if (stacked) stackedValues(chart) else emptyList()
    val skip = every(at.size)
    XYGraph(
        xAxisModel = CategoryAxisModel(at),
        yAxisModel = rememberFloatLinearAxisModel(0f..top),
        xAxisLabels = indexLabels(chart, skip, 10),
        yAxisLabels = valueLabels(chart),
        modifier = Modifier.fillMaxWidth().height(plot ?: 240.dp),
    ) {
        chart.datasets.forEachIndexed { d, ds ->
            // Nothing to measure leaves a gap in the data, not a zero.
            val points = at.mapNotNull { i ->
                ds.data.getOrNull(i)?.let { v -> Point(i, (if (stacked) sums[i][d] else v).toFloat()) }
            }
            val line = LineStyle(SolidColor(series[d]), 2.dp)
            val dot: @Composable HoverableElementAreaScope.(Point<Int, Float>) -> Unit = { Symbol(shape = CircleShape, size = 6.dp, fillBrush = SolidColor(series[d])) }
            if (filled) {
                // A stacked band lies between the line under it and its own; the first, and any area that is not stacked, to the axis.
                val under = if (stacked && d > 0) AreaBaseline.ArbitraryLine(points.map { p -> Point(p.x, sums[p.x][d - 1].toFloat()) }) else AreaBaseline.ConstantLine<Int, Float>(0f)
                AreaPlot(points, under, AreaStyle(SolidColor(series[d]), alpha = 0.25f), lineStyle = line, symbol = dot)
            } else {
                LinePlot(points, lineStyle = line, symbol = dot)
            }
        }
    }
}

@OptIn(ExperimentalKoalaPlotApi::class)
@Composable
private fun Radar(chart: BoardChart, series: List<Color>, plot: Dp?) {
    val at = chart.labels.indices.toList()
    val top = chartTop(chart, false)
    PolarGraph(
        radialAxisModel = rememberFloatRadialAxisModel(listOf(0f, top / 2, top)),
        angularAxisModel = rememberCategoryAngularAxisModel(at),
        radialAxisLabels = { AxisText(formatBoardValue(it.toDouble(), chart.unit)) },
        angularAxisLabels = { AxisText(chart.labels[it].label.take(12)) },
        modifier = Modifier.fillMaxWidth().height(plot ?: 280.dp),
    ) {
        chart.datasets.forEachIndexed { d, ds ->
            PolarPlotSeries(
                data = at.map { i -> PolarPoint((ds.data.getOrNull(i) ?: 0.0).toFloat(), i) },
                lineStyle = LineStyle(SolidColor(series[d]), 2.dp),
                areaStyle = AreaStyle(SolidColor(series[d]), alpha = 0.25f),
            )
        }
    }
}

/** A pie or a donut: one series, its slices the labels, each in the palette in turn, with a list of what they are. */
@OptIn(ExperimentalKoalaPlotApi::class)
@Composable
private fun Slices(chart: BoardChart, section: Section, dark: Boolean, plot: Dp?) {
    val data = chart.datasets.firstOrNull() ?: return
    val values = chart.labels.indices.map { (data.data.getOrNull(it) ?: 0.0).coerceAtLeast(0.0) }
    val total = values.sum()
    if (total <= 0.0) {
        Dim("Nothing to show.")
        return
    }
    val slice = chart.labels.indices.map { defaultSeriesColor(it, dark) }
    val pie: @Composable (Modifier, Dp) -> Unit = { modifier, side ->
        Box(modifier.height(side)) {
            PieChart(
                values = values.map { it.toFloat() },
                slice = { i -> DefaultSlice(color = slice[i]) },
                holeSize = if (section.type == "donut") 0.55f else 0f,
                minPieDiameter = 64.dp,
                maxPieDiameter = side,
            )
        }
    }
    val names: @Composable (Modifier) -> Unit = { modifier ->
        Column(modifier, verticalArrangement = Arrangement.spacedBy(3.dp)) {
            chart.labels.forEachIndexed { i, l ->
                if (values[i] > 0.0) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                        Box(Modifier.size(10.dp).background(slice[i], RoundedCornerShape(2.dp)))
                        Text(
                            "${l.label} · ${formatBoardValue(data.data[i], chart.unit)} (${(values[i] / total * 100).toInt()}%)",
                            fontSize = 12.sp,
                            color = MaterialTheme.colorScheme.onSurface,
                        )
                    }
                }
            }
        }
    }
    // Side by side where there is room; in a narrow card the names go under the pie, which is as wide as the card.
    BoxWithConstraints {
        val room = maxWidth
        if (room < 300.dp) {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                pie(Modifier.fillMaxWidth(), minOf(plot ?: 180.dp, room))
                names(Modifier.fillMaxWidth())
            }
        } else {
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                pie(Modifier.weight(1f), plot ?: 180.dp)
                names(Modifier.weight(1f))
            }
        }
    }
}
