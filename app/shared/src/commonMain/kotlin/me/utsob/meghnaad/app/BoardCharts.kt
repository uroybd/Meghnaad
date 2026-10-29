package me.utsob.meghnaad.app

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
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
import androidx.compose.foundation.layout.wrapContentWidth
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
import io.github.koalaplot.core.bar.BarScope
import io.github.koalaplot.core.bar.DefaultBar
import io.github.koalaplot.core.bar.GroupedHorizontalBarPlot
import io.github.koalaplot.core.bar.GroupedVerticalBarPlot
import io.github.koalaplot.core.bar.StackedHorizontalBarPlot
import io.github.koalaplot.core.bar.StackedVerticalBarPlot
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

/** The colour of the bar of series [d] at label [i]: with one series, the taskrc's colour for the label's value when it has one; else the series'. */
fun barColor(chart: BoardChart, series: List<Color>, labelColors: List<Color?>, d: Int, i: Int): Color =
    (if (chart.datasets.size == 1) labelColors.getOrNull(i) else null) ?: series[d]

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

/** Every how-many-th label of [count] fits along an axis [width] wide when the longest takes [chars] characters of axis text. */
fun labelSkip(count: Int, chars: Int, width: Dp): Int {
    val each = chars * 5.5f + 10f
    val room = (width.value / each).toInt().coerceAtLeast(1)
    return ((count + room - 1) / room).coerceAtLeast(1)
}

/**
 * How strong each cell of a heatmap is (a row for each series, a column for each label): 0 to 1 over the range of the values
 * (counted from zero), or null where there is nothing to measure (heatmapConfig in boardChart.ts).
 */
fun heatStrengths(chart: BoardChart): List<List<Float?>> {
    val values = chart.datasets.flatMap { it.data }.filterNotNull()
    val low = minOf(0.0, values.minOrNull() ?: 0.0)
    val high = maxOf(0.0, values.maxOrNull() ?: 0.0)
    return chart.datasets.map { d ->
        chart.labels.indices.map { i -> d.data.getOrNull(i)?.let { v -> if (high == low) 0f else ((v - low) / (high - low)).toFloat() } }
    }
}

/** The opacity of a cell of that strength: from a pale wash to the full colour. */
fun heatAlpha(strength: Float): Float = (0x26 + strength * (0xff - 0x26)) / 255f

/** What a tap on label [i] of series [d] says: the label in full, and the value (with the series' name when there are several). */
fun tipText(chart: BoardChart, i: Int, d: Int): String {
    val value = formatBoardValue(chart.datasets.getOrNull(d)?.data?.getOrNull(i), chart.unit)
    val series = if (chart.datasets.size > 1) "${chart.datasets[d].label}: " else ""
    return "${chart.labels[i].label} · $series$value"
}

@OptIn(ExperimentalKoalaPlotApi::class, ExperimentalLayoutApi::class)
@Composable
fun BoardChartView(chart: BoardChart, section: Section, dark: Boolean, modifier: Modifier = Modifier, height: Dp? = null, labelColors: List<Color?> = emptyList()) {
    val colors = MaterialTheme.colorScheme
    if (chart.labels.isEmpty()) {
        Dim("Nothing to show.", modifier)
        return
    }
    val stacked = section.stacked == true
    val series = seriesColors(chart, section, dark)
    val round = section.type == "pie" || section.type == "donut" || section.type == "polar"
    val legend = !round && section.type != "heatmap" && chart.datasets.size > 1
    // A chart in a card that has a size fills what the legend leaves; one without is as tall as it likes.
    val plot = height?.let { (it - if (legend) 28.dp else 0.dp).coerceAtLeast(96.dp) }
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        // A legend names the series; a round chart's names its slices instead.
        if (legend) Legend(chart.datasets.map { it.label }, series)
        when (section.type) {
            "bar" -> if (section.by?.time != null) VerticalBars(chart, series, stacked, plot) else HorizontalBars(chart, series, stacked, plot, labelColors)
            "line", "area" -> Lines(chart, series, stacked, section.type == "area", plot)
            "radar" -> Radar(chart, series, plot)
            "heatmap" -> Heatmap(chart, series.firstOrNull() ?: defaultSeriesColor(0, dark), plot)
            else -> Slices(chart, section, dark, plot, labelColors)
        }
    }
}

/**
 * A heatmap: a row for each series and a column for each label, a cell as strong as its value is large, in the colour of the first series.
 * Koala Plot has no plot for it, so the grid is laid out here (the one chart that is: the rules for charts allow it for this). A tap on a
 * cell says its row, its column and its value, as a tap on a bar does.
 */
@Composable
private fun Heatmap(chart: BoardChart, base: Color, plot: Dp?) = BoxWithConstraints {
    val strengths = heatStrengths(chart)
    var picked by remember(chart) { mutableStateOf<Pair<Int, Int>?>(null) }
    val rowChars = minOf(14, chart.datasets.maxOfOrNull { it.label.length } ?: 1)
    val labelRoom = (rowChars * 7 + 12).dp
    val rowHeight = plot?.let { ((it - 24.dp) / chart.datasets.size.coerceAtLeast(1)).coerceIn(20.dp, 96.dp) } ?: 30.dp
    // A column's label is a week's date in full, as far as there is room for it (the skip leaves the room).
    val chars = widest(chart, 12)
    val skip = labelSkip(chart.labels.size, chars, maxWidth - labelRoom)
    val none = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.4f)
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(2.dp)) {
        chart.datasets.forEachIndexed { d, ds ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                AxisText(ds.label.take(rowChars), Modifier.width(labelRoom))
                chart.labels.indices.forEach { i ->
                    val strength = strengths[d][i]
                    Box(
                        Modifier.weight(1f).height(rowHeight).padding(1.dp)
                            .background(if (strength == null) none else base.copy(alpha = heatAlpha(strength)), RoundedCornerShape(2.dp))
                            .clickable { picked = if (picked == d to i) null else d to i }
                            .semantics { contentDescription = tipText(chart, i, d) },
                    )
                }
            }
        }
        // The labels of the columns, every skip-th, centred under their cell.
        Row {
            Spacer(Modifier.width(labelRoom))
            chart.labels.indices.forEach { i ->
                Box(Modifier.weight(1f), Alignment.Center) {
                    if (i % skip == 0) AxisText(chart.labels[i].label.take(chars), Modifier.wrapContentWidth(Alignment.CenterHorizontally, unbounded = true))
                }
            }
        }
        picked?.let { (d, i) -> ChartTip(tipText(chart, i, d)) }
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

/**
 * The labels along an axis of categories: the label at each place, every [skip]-th. Typed, so the library's overloads are told
 * apart. A label is given only the room of its own category, which would cut it short; with [spread] it takes what it needs
 * (centred on its place), and [skip] leaves the room.
 */
private fun indexLabels(chart: BoardChart, skip: Int, width: Int, spread: Boolean = false): @Composable (Int) -> Unit = { i ->
    if (i % skip == 0) AxisText(chart.labels[i].label.take(width), if (spread) Modifier.wrapContentWidth(Alignment.CenterHorizontally, unbounded = true) else Modifier)
}

/** The widest label, in characters, as far as [cap]. */
private fun widest(chart: BoardChart, cap: Int) = minOf(cap, chart.labels.maxOfOrNull { it.label.length } ?: 1)

/** What shows over a bar or a point when it is tapped (or hovered): the label in full and the value. */
@Composable
private fun ChartTip(text: String) {
    Box(Modifier.background(MaterialTheme.colorScheme.inverseSurface, RoundedCornerShape(4.dp)).padding(horizontal = 8.dp, vertical = 4.dp)) {
        Text(text, fontSize = 12.sp, color = MaterialTheme.colorScheme.inverseOnSurface)
    }
}

/** The labels along an axis of values. */
private fun valueLabels(chart: BoardChart): @Composable (Float) -> Unit = { v -> AxisText(formatBoardValue(v.toDouble(), chart.unit)) }

@Composable
private fun AxisText(text: String, modifier: Modifier = Modifier) =
    Text(text, modifier, fontSize = 10.sp, maxLines = 1, color = MaterialTheme.colorScheme.onSurfaceVariant)

@OptIn(ExperimentalKoalaPlotApi::class)
@Composable
private fun VerticalBars(chart: BoardChart, series: List<Color>, stacked: Boolean, plot: Dp?) = BoxWithConstraints {
    val at = chart.labels.indices.toList()
    val top = chartTop(chart, stacked)
    val chars = widest(chart, 24)
    val skip = labelSkip(at.size, chars, maxWidth - AXIS_ROOM)
    XYGraph(
        xAxisModel = CategoryAxisModel(at),
        yAxisModel = rememberFloatLinearAxisModel(0f..top),
        xAxisLabels = indexLabels(chart, skip, chars, spread = true),
        yAxisLabels = valueLabels(chart),
        modifier = Modifier.fillMaxWidth().height(plot ?: 240.dp),
    ) {
        if (stacked) {
            StackedVerticalBarPlot {
                chart.datasets.forEachIndexed { d, ds ->
                    series({ _, i, _ -> TippedBar(series[d], tipText(chart, i, d)) }) { at.forEach { i -> item(i, (ds.data.getOrNull(i) ?: 0.0).toFloat()) } }
                }
            }
        } else {
            GroupedVerticalBarPlot {
                chart.datasets.forEachIndexed { d, ds ->
                    series({ _, i, _ -> TippedBar(series[d], tipText(chart, i, d)) }) { at.forEach { i -> item(i, 0f, (ds.data.getOrNull(i) ?: 0.0).toFloat()) } }
                }
            }
        }
    }
}

/** Room the value axis takes beside the plot. */
private val AXIS_ROOM = 44.dp

/** A bar in its series' colour that shows [tip] when tapped (or hovered). */
@OptIn(ExperimentalKoalaPlotApi::class)
@Composable
private fun BarScope.TippedBar(color: Color, tip: String) = DefaultBar(brush = SolidColor(color), hoverElement = { ChartTip(tip) })

/** Bars that lie down, for a category: its names are long. */
@OptIn(ExperimentalKoalaPlotApi::class)
@Composable
private fun HorizontalBars(chart: BoardChart, series: List<Color>, stacked: Boolean, plot: Dp?, labelColors: List<Color?> = emptyList()) {
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
                    series({ _, k, _ -> TippedBar(barColor(chart, series, labelColors, d, at[k]), tipText(chart, at[k], d)) }) { at.forEach { i -> item((ds.data.getOrNull(i) ?: 0.0).toFloat(), i) } }
                }
            }
        } else {
            GroupedHorizontalBarPlot {
                chart.datasets.forEachIndexed { d, ds ->
                    series({ _, k, _ -> TippedBar(barColor(chart, series, labelColors, d, at[k]), tipText(chart, at[k], d)) }) { at.forEach { i -> item(i, 0f, (ds.data.getOrNull(i) ?: 0.0).toFloat()) } }
                }
            }
        }
    }
}

@OptIn(ExperimentalKoalaPlotApi::class)
@Composable
private fun Lines(chart: BoardChart, series: List<Color>, stacked: Boolean, filled: Boolean, plot: Dp?) = BoxWithConstraints {
    val at = chart.labels.indices.toList()
    val top = chartTop(chart, stacked)
    val sums = if (stacked) stackedValues(chart) else emptyList()
    val chars = widest(chart, 24)
    val skip = labelSkip(at.size, chars, maxWidth - AXIS_ROOM)
    XYGraph(
        xAxisModel = CategoryAxisModel(at),
        yAxisModel = rememberFloatLinearAxisModel(0f..top),
        xAxisLabels = indexLabels(chart, skip, chars, spread = true),
        yAxisLabels = valueLabels(chart),
        modifier = Modifier.fillMaxWidth().height(plot ?: 240.dp),
    ) {
        chart.datasets.forEachIndexed { d, ds ->
            // Nothing to measure leaves a gap in the data, not a zero.
            val points = at.mapNotNull { i ->
                ds.data.getOrNull(i)?.let { v -> Point(i, (if (stacked) sums[i][d] else v).toFloat()) }
            }
            val line = LineStyle(SolidColor(series[d]), 2.dp)
            val dot: @Composable HoverableElementAreaScope.(Point<Int, Float>) -> Unit = { p ->
                Symbol(shape = CircleShape, size = 8.dp, fillBrush = SolidColor(series[d]), modifier = Modifier.hoverableElement { ChartTip(tipText(chart, p.x, d)) })
            }
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

/** A pie or a donut: one series, its slices the labels, each in the colour the taskrc gives its value (a UDA's) or else the palette in turn, with a list of what they are. */
@OptIn(ExperimentalKoalaPlotApi::class)
@Composable
private fun Slices(chart: BoardChart, section: Section, dark: Boolean, plot: Dp?, labelColors: List<Color?>) {
    val data = chart.datasets.firstOrNull() ?: return
    val values = chart.labels.indices.map { (data.data.getOrNull(it) ?: 0.0).coerceAtLeast(0.0) }
    val total = values.sum()
    if (total <= 0.0) {
        Dim("Nothing to show.")
        return
    }
    val slice = chart.labels.indices.map { labelColors.getOrNull(it) ?: defaultSeriesColor(it, dark) }
    val pie: @Composable (Modifier, Dp) -> Unit = { modifier, room ->
        // A section a quarter of a phone's row wide leaves less than the library's smallest pie, and it throws if its
        // smallest is larger than its largest.
        val side = room.coerceAtLeast(24.dp)
        Box(modifier.height(side)) {
            PieChart(
                values = values.map { it.toFloat() },
                slice = { i -> DefaultSlice(color = slice[i]) },
                holeSize = if (section.type == "donut") 0.55f else 0f,
                minPieDiameter = minOf(64.dp, side),
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
