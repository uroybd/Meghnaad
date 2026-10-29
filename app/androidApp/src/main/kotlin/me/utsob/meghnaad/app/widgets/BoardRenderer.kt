package me.utsob.meghnaad

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import me.utsob.meghnaad.app.BoardChart
import me.utsob.meghnaad.app.Section
import me.utsob.meghnaad.app.chartTop
import me.utsob.meghnaad.app.formatBoardValue
import me.utsob.meghnaad.app.heatAlpha
import me.utsob.meghnaad.app.heatStrengths
import me.utsob.meghnaad.app.pieSweeps
import me.utsob.meghnaad.app.seriesColors
import me.utsob.meghnaad.app.chartColor
import me.utsob.meghnaad.app.heatCellColor
import me.utsob.meghnaad.app.heatRowsAreSeries
import me.utsob.meghnaad.app.heatColors
import me.utsob.meghnaad.app.defaultSeriesColor
import me.utsob.meghnaad.app.stackedValues
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin

/**
 * Draws a card of a board (the engine's numbers and chart data) on a canvas: a widget cannot hold a Compose chart, only a picture.
 * It is the same data the screens draw, so a widget agrees with the board it shows; the look is simpler (BoardCharts.kt).
 */
object BoardRenderer {
    /** The widest a chart is cut: more bars than this stop reading as bars. */
    private const val MAX_LABELS = 40

    /** A number: the figure large, and how many tasks it is of under it. */
    fun number(value: String, tasks: Int, width: Int, height: Int, palette: Palette): Bitmap {
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)
        val big = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = palette.text
            isFakeBoldText = true
            textAlign = Paint.Align.CENTER
            // As large as the widget has room for, never wider than it is.
            textSize = min(height * 0.5f, width * 1.6f / max(value.length, 1))
        }
        val small = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = palette.dim
            textAlign = Paint.Align.CENTER
            textSize = (height * 0.1f).coerceIn(16f, 36f)
        }
        val caption = "$tasks ${if (tasks == 1) "task" else "tasks"}"
        val middle = height / 2f
        canvas.drawText(value, width / 2f, middle + big.textSize * 0.2f, big)
        canvas.drawText(caption, width / 2f, middle + big.textSize * 0.2f + small.textSize * 1.6f, small)
        return bitmap
    }

    /** A chart of the card's type. A polar area chart is drawn as a pie, as in the app. */
    fun chart(chart: BoardChart, section: Section, width: Int, height: Int, palette: Palette, labelColors: List<Color?> = emptyList(), grids: List<BoardChart>? = null): Bitmap {
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)
        if (chart.labels.isEmpty()) return bitmap
        val shown = cut(chart)
        val stacked = section.stacked == true
        val colors = seriesColors(shown, section, palette.dark).map { it.toArgb() }
        val text = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = palette.dim
            textSize = (height * 0.075f).coerceIn(16f, 30f)
        }
        // A heatmap with rows is a grid for each series, in that series' colour, one above the other.
        if (grids != null && section.type == "heatmap") {
            val caption = if (grids.size > 1) text.textSize * 1.4f else 0f
            val each = height.toFloat() / grids.size
            grids.forEachIndexed { i, g ->
                val at = i * each
                if (caption > 0f) canvas.drawText(clip(g.label, text, width - 8f), 4f, at + text.textSize, text)
                heatmap(canvas, cut(g), heatColors(section, grids, i, palette.dark), heatRowsAreSeries(section), RectF(0f, at + caption, width.toFloat(), at + each - 4f), text, palette)
            }
            return bitmap
        }
        // A legend names the series of a chart that has several.
        var top = 0f
        if (shown.datasets.size > 1 && section.type !in listOf("pie", "donut", "polar", "heatmap")) top = legend(canvas, shown, colors, text, width)
        val area = RectF(0f, top, width.toFloat(), height.toFloat())
        when (section.type) {
            "bar" -> if (section.by?.time != null) verticalBars(canvas, shown, colors, stacked, area, text, palette) else horizontalBars(canvas, shown, colors, stacked, area, text, palette, labelColors)
            "line", "area" -> lines(canvas, shown, colors, stacked, section.type == "area", area, text, palette)
            "radar" -> radar(canvas, shown, colors, area, text, palette)
            "heatmap" -> heatmap(canvas, shown, seriesColors(shown, section, palette.dark), heatRowsAreSeries(section), area, text, palette)
            else -> slices(canvas, shown, palette, area, text, donut = section.type == "donut", labelColors = labelColors)
        }
        return bitmap
    }

    /**
     * A heatmap: a column for each label and a row for each row of the chart (named at the left), a cell as strong as its value is large, a
     * square, in the colour of its series ([heatCellColor]); the whole of it in the middle of its place (the app draws it the same way,
     * BoardCharts.kt).
     */
    private fun heatmap(canvas: Canvas, chart: BoardChart, colors: List<Color>, rowsAreSeries: Boolean, area: RectF, text: Paint, palette: Palette) {
        val rows = chart.datasets
        if (rows.isEmpty()) return
        val room = min(rows.maxOf { text.measureText(it.label) } + 10f, area.width() * 0.35f)
        val labelsHeight = text.textSize * 1.7f
        val side = minOf((area.width() - room - 2f) / chart.labels.size, (area.height() - labelsHeight - 4f) / rows.size).coerceAtLeast(4f)
        // Up and down the middle of the place, with the labels of the columns under it.
        val top = area.top + ((area.height() - labelsHeight - side * rows.size) / 2f).coerceAtLeast(0f)
        val plot = RectF(area.left + room, top, area.left + room + side * chart.labels.size, top + side * rows.size)
        val strengths = heatStrengths(chart)
        val fill = Paint(Paint.ANTI_ALIAS_FLAG)
        val empty = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            this.color = palette.line
            alpha = 0x66
        }
        val saved = text.textAlign
        text.textAlign = Paint.Align.LEFT
        rows.forEachIndexed { r, d ->
            val name = d.label.take(text.breakText(d.label, true, room - 8f, null))
            canvas.drawText(name, area.left + 2f, plot.top + (r + 0.5f) * side + text.textSize * 0.35f, text)
            for (c in chart.labels.indices) {
                val s = strengths[r][c]
                val paint = if (s == null) empty else fill.apply { this.color = heatCellColor(chart, r, c, colors, rowsAreSeries).toArgb(); alpha = (heatAlpha(s) * 255f).toInt() }
                val x = plot.left + c * side
                val y = plot.top + r * side
                canvas.drawRoundRect(RectF(x + 1f, y + 1f, x + side - 1f, y + side - 1f), 3f, 3f, paint)
            }
        }
        text.textAlign = saved
        bottomLabels(canvas, chart, plot, text, plot.bottom + text.textSize * 1.3f, centred = true)
    }

    /** The last labels, when there are more than will read. */
    private fun cut(chart: BoardChart): BoardChart {
        if (chart.labels.size <= MAX_LABELS) return chart
        val from = chart.labels.size - MAX_LABELS
        return BoardChart(
            chart.unit,
            chart.labels.drop(from),
            chart.datasets.map { it.copy(data = it.data.drop(from), counts = it.counts.drop(from), parts = it.parts.map { p -> p.copy(data = p.data.drop(from)) }) },
        )
    }

    private fun legend(canvas: Canvas, chart: BoardChart, colors: List<Int>, text: Paint, width: Int): Float {
        val swatch = text.textSize * 0.8f
        var x = 4f
        var y = text.textSize
        val fill = Paint()
        for ((i, d) in chart.datasets.withIndex()) {
            val need = swatch + 6f + text.measureText(d.label) + 14f
            if (x + need > width && x > 4f) {
                x = 4f
                y += text.textSize * 1.3f
            }
            fill.color = colors[i]
            canvas.drawRect(x, y - swatch, x + swatch, y, fill)
            canvas.drawText(d.label, x + swatch + 6f, y, text)
            x += need
        }
        return y + text.textSize * 0.6f
    }

    private fun axisFrame(canvas: Canvas, area: RectF, top: Float, unit: String, text: Paint, palette: Palette, labels: Boolean = true): RectF {
        val topText = formatBoardValue(top.toDouble(), unit)
        val left = text.measureText(topText) + 10f
        val plot = RectF(area.left + left, area.top + text.textSize * 0.6f, area.right - 4f, area.bottom - if (labels) text.textSize * 1.7f else 4f)
        val grid = Paint().apply {
            color = palette.line
            strokeWidth = 1.5f
        }
        for (frac in listOf(0f, 0.5f, 1f)) {
            val y = plot.bottom - frac * plot.height()
            canvas.drawLine(plot.left, y, plot.right, y, grid)
            canvas.drawText(formatBoardValue((top * frac).toDouble(), unit), area.left + 2f, y + text.textSize * 0.35f, text)
        }
        return plot
    }

    /** A few labels along the bottom: the first from the left edge, the last to the right edge, the middle between. */
    private fun bottomLabels(canvas: Canvas, chart: BoardChart, plot: RectF, text: Paint, baseline: Float, centred: Boolean) {
        val n = chart.labels.size
        val slot = plot.width() / n
        val saved = text.textAlign
        // The first, the middle and the last, as many as fit side by side (a narrow plot has two, or one).
        val widest = chart.labels.maxOf { text.measureText(it.label) } * 1.25f
        val places = when {
            plot.width() >= widest * 3 -> listOf(0, n / 2, n - 1)
            plot.width() >= widest * 2 -> listOf(0, n - 1)
            else -> listOf(0)
        }
        for (i in places.distinct()) {
            val label = chart.labels[i].label
            when (i) {
                0 -> { text.textAlign = Paint.Align.LEFT; canvas.drawText(label, plot.left, baseline, text) }
                n - 1 -> { text.textAlign = Paint.Align.RIGHT; canvas.drawText(label, plot.right, baseline, text) }
                else -> { text.textAlign = Paint.Align.CENTER; canvas.drawText(label, plot.left + (if (centred) i + 0.5f else i.toFloat()) * slot, baseline, text) }
            }
        }
        text.textAlign = saved
    }

    private fun verticalBars(canvas: Canvas, chart: BoardChart, colors: List<Int>, stacked: Boolean, area: RectF, text: Paint, palette: Palette) {
        val top = chartTop(chart, stacked)
        val plot = axisFrame(canvas, area, top, chart.unit, text, palette)
        val n = chart.labels.size
        val slot = plot.width() / n
        val gap = (slot * 0.18f).coerceAtMost(4f)
        val fill = Paint()
        for (i in 0 until n) {
            val x0 = plot.left + i * slot + gap / 2
            val x1 = plot.left + (i + 1) * slot - gap / 2
            if (stacked) {
                var y = plot.bottom
                for (d in chart.datasets.indices) {
                    val v = chart.datasets[d].data.getOrNull(i) ?: continue
                    if (v <= 0) continue
                    val h = (v / top).toFloat() * plot.height()
                    fill.color = colors[d]
                    canvas.drawRect(x0, y - h, x1, y, fill)
                    y -= h
                }
            } else {
                val w = (x1 - x0) / chart.datasets.size
                for (d in chart.datasets.indices) {
                    val v = chart.datasets[d].data.getOrNull(i) ?: continue
                    if (v <= 0) continue
                    fill.color = colors[d]
                    canvas.drawRect(x0 + d * w, plot.bottom - (v / top).toFloat() * plot.height(), x0 + (d + 1) * w, plot.bottom, fill)
                }
            }
        }
        bottomLabels(canvas, chart, plot, text, area.bottom - text.textSize * 0.35f, centred = true)
    }

    /** Bars that lie down, for a category: the first label at the top. */
    private fun horizontalBars(canvas: Canvas, chart: BoardChart, colors: List<Int>, stacked: Boolean, area: RectF, text: Paint, palette: Palette, labelColors: List<Color?> = emptyList()) {
        val top = chartTop(chart, stacked)
        val n = chart.labels.size
        val names = min(area.width() * 0.35f, chart.labels.maxOf { text.measureText(it.label) } + 10f)
        val plot = RectF(area.left + names, area.top + 2f, area.right - 4f, area.bottom - 2f)
        val slot = plot.height() / n
        val bar = (slot * 0.7f).coerceAtMost(text.textSize * 1.6f)
        val fill = Paint()
        val saved = text.textAlign
        for (i in 0 until n) {
            val mid = plot.top + (i + 0.5f) * slot
            text.textAlign = Paint.Align.RIGHT
            canvas.drawText(clip(chart.labels[i].label, text, names - 8f), area.left + names - 6f, mid + text.textSize * 0.35f, text)
            var x = plot.left
            val each = bar / if (stacked) 1 else chart.datasets.size
            for (d in chart.datasets.indices) {
                val v = chart.datasets[d].data.getOrNull(i) ?: continue
                if (v <= 0) continue
                val w = (v / top).toFloat() * plot.width()
                // One series: the taskrc's colour for this value, when it has one.
                fill.color = if (chart.datasets.size == 1) labelColors.getOrNull(i)?.toArgb() ?: colors[d] else colors[d]
                if (stacked) {
                    canvas.drawRect(x, mid - bar / 2, x + w, mid + bar / 2, fill)
                    x += w
                } else {
                    canvas.drawRect(plot.left, mid - bar / 2 + d * each, plot.left + w, mid - bar / 2 + (d + 1) * each, fill)
                }
            }
            // One series has room for its value at the end of each bar.
            if (chart.datasets.size == 1) {
                chart.datasets[0].data.getOrNull(i)?.takeIf { it > 0 }?.let { v ->
                    text.textAlign = Paint.Align.LEFT
                    canvas.drawText(formatBoardValue(v, chart.unit), plot.left + (v / top).toFloat() * plot.width() + 6f, mid + text.textSize * 0.35f, text)
                }
            }
        }
        text.textAlign = saved
    }

    private fun lines(canvas: Canvas, chart: BoardChart, colors: List<Int>, stacked: Boolean, filled: Boolean, area: RectF, text: Paint, palette: Palette) {
        val top = chartTop(chart, stacked)
        val plot = axisFrame(canvas, area, top, chart.unit, text, palette)
        val n = chart.labels.size
        val x = { i: Int -> if (n == 1) plot.centerX() else plot.left + plot.width() * i / (n - 1) }
        val sums = if (stacked) stackedValues(chart) else emptyList()
        val stroke = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.STROKE
            strokeWidth = 3f
            strokeJoin = Paint.Join.ROUND
        }
        val fill = Paint(Paint.ANTI_ALIAS_FLAG)
        val dot = Paint(Paint.ANTI_ALIAS_FLAG)
        for (d in chart.datasets.indices) {
            // A label with nothing to measure leaves a gap in the line, not a zero.
            val points = (0 until n).mapNotNull { i ->
                chart.datasets[d].data.getOrNull(i)?.let { v -> i to (if (stacked) sums[i][d] else v) }
            }
            if (points.isEmpty()) continue
            val at = { v: Double -> plot.bottom - (v / top).toFloat() * plot.height() }
            val path = Path()
            points.forEachIndexed { k, (i, v) -> if (k == 0) path.moveTo(x(i), at(v)) else path.lineTo(x(i), at(v)) }
            if (filled) {
                // A stacked band lies between the line under it and its own; the first, and any area that is not stacked, to the axis.
                val shape = Path(path).apply {
                    if (stacked && d > 0) {
                        for ((i, _) in points.asReversed()) lineTo(x(i), at(sums[i][d - 1]))
                    } else {
                        lineTo(x(points.last().first), plot.bottom)
                        lineTo(x(points.first().first), plot.bottom)
                    }
                    close()
                }
                fill.color = (colors[d] and 0x00FFFFFF) or 0x40000000
                canvas.drawPath(shape, fill)
            }
            stroke.color = colors[d]
            canvas.drawPath(path, stroke)
            dot.color = colors[d]
            for ((i, v) in points) canvas.drawCircle(x(i), at(v), 4f, dot)
        }
        bottomLabels(canvas, chart, plot, text, area.bottom - text.textSize * 0.35f, centred = false)
    }

    /** A pie or a donut of the first series, each slice a colour of the palette in turn, with what they are beside it. */
    private fun slices(canvas: Canvas, chart: BoardChart, palette: Palette, area: RectF, text: Paint, donut: Boolean = false, labelColors: List<Color?> = emptyList()) {
        val data = chart.datasets.firstOrNull() ?: return
        val values = chart.labels.indices.map { (data.data.getOrNull(it) ?: 0.0).coerceAtLeast(0.0) }
        val sweeps = pieSweeps(values)
        if (sweeps.all { it == 0f }) return
        val side = min(area.height(), area.width() * 0.55f) - 8f
        val ring = RectF(area.left + 4f, area.centerY() - side / 2, area.left + 4f + side, area.centerY() + side / 2)
        val paint = Paint(Paint.ANTI_ALIAS_FLAG)
        var start = -90f
        for ((i, sweep) in sweeps.withIndex()) {
            if (sweep <= 0f) continue
            paint.color = (labelColors.getOrNull(i) ?: defaultSeriesColor(i, palette.dark)).toArgb()
            canvas.drawArc(ring, start, sweep, true, paint)
            start += sweep
        }
        if (donut) {
            // The hole is cut out of the picture, so what is behind the widget shows through it.
            val hole = Paint(Paint.ANTI_ALIAS_FLAG).apply { xfermode = android.graphics.PorterDuffXfermode(android.graphics.PorterDuff.Mode.CLEAR) }
            canvas.drawCircle(ring.centerX(), ring.centerY(), side * 0.28f, hole)
        }
        val total = values.sum()
        var y = area.centerY() - (sweeps.count { it > 0f } * text.textSize * 1.3f) / 2 + text.textSize
        val x = ring.right + 14f
        for ((i, v) in values.withIndex()) {
            if (v <= 0.0) continue
            paint.color = (labelColors.getOrNull(i) ?: defaultSeriesColor(i, palette.dark)).toArgb()
            canvas.drawRect(x, y - text.textSize * 0.8f, x + text.textSize * 0.8f, y, paint)
            val line = "${chart.labels[i].label} ${formatBoardValue(v, chart.unit)} (${(v / total * 100).toInt()}%)"
            canvas.drawText(clip(line, text, area.right - x - text.textSize - 8f), x + text.textSize, y, text)
            y += text.textSize * 1.3f
        }
    }

    /** A radar: a shape for each series, the labels around a circle. */
    private fun radar(canvas: Canvas, chart: BoardChart, colors: List<Int>, area: RectF, text: Paint, palette: Palette) {
        val n = chart.labels.size
        if (n < 3) return slices(canvas, chart, palette, area, text)
        val top = chartTop(chart, false)
        val radius = min(area.width(), area.height()) / 2 - text.textSize * 2f
        val cx = area.centerX()
        val cy = area.centerY()
        val angle = { i: Int -> (-Math.PI / 2 + 2 * Math.PI * i / n) }
        val grid = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.STROKE
            color = palette.line
            strokeWidth = 1.5f
        }
        for (frac in listOf(0.5f, 1f)) {
            val ring = Path()
            for (i in 0 until n) {
                val px = cx + (radius * frac * cos(angle(i))).toFloat()
                val py = cy + (radius * frac * sin(angle(i))).toFloat()
                if (i == 0) ring.moveTo(px, py) else ring.lineTo(px, py)
            }
            ring.close()
            canvas.drawPath(ring, grid)
        }
        val saved = text.textAlign
        for (i in 0 until n) {
            canvas.drawLine(cx, cy, cx + (radius * cos(angle(i))).toFloat(), cy + (radius * sin(angle(i))).toFloat(), grid)
            val lx = cx + ((radius + text.textSize * 0.6f) * cos(angle(i))).toFloat()
            val ly = cy + ((radius + text.textSize * 0.6f) * sin(angle(i))).toFloat() + text.textSize * 0.35f
            text.textAlign = if (cos(angle(i)) > 0.3) Paint.Align.LEFT else if (cos(angle(i)) < -0.3) Paint.Align.RIGHT else Paint.Align.CENTER
            canvas.drawText(clip(chart.labels[i].label, text, area.width() * 0.25f), lx, ly, text)
        }
        text.textAlign = saved
        val line = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.STROKE
            strokeWidth = 3f
        }
        val fill = Paint(Paint.ANTI_ALIAS_FLAG)
        for (d in chart.datasets.indices) {
            val shape = Path()
            for (i in 0 until n) {
                val r = radius * ((chart.datasets[d].data.getOrNull(i) ?: 0.0) / top).toFloat()
                val px = cx + (r * cos(angle(i))).toFloat()
                val py = cy + (r * sin(angle(i))).toFloat()
                if (i == 0) shape.moveTo(px, py) else shape.lineTo(px, py)
            }
            shape.close()
            fill.color = (colors[d] and 0x00FFFFFF) or 0x40000000
            canvas.drawPath(shape, fill)
            line.color = colors[d]
            canvas.drawPath(shape, line)
        }
    }

    private fun clip(text: String, paint: Paint, room: Float): String {
        if (paint.measureText(text) <= room) return text
        var s = text
        while (s.isNotEmpty() && paint.measureText("$s…") > room) s = s.dropLast(1)
        return if (s.isEmpty()) "" else "$s…"
    }
}
