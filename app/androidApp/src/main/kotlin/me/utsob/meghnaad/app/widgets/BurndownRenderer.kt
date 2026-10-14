package me.utsob.meghnaad.app.widgets

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import org.json.JSONObject

/** Draws a burndown (the engine's `burndown.*` answer) as stacked bars: finished at the bottom, then started, then pending. */
object BurndownRenderer {
    /** How many bars fit and still read as bars. */
    private const val MAX_BARS = 40

    fun draw(result: JSONObject, width: Int, height: Int, palette: Palette): Bitmap {
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)
        val all = result.getJSONArray("bars")
        val first = (all.length() - MAX_BARS).coerceAtLeast(0)
        val bars = (first until all.length()).map { all.getJSONObject(it) }
        val carryover = result.optLong("carryover_done", 0)
        val labels = result.optJSONArray("y_labels")
        val top = (labels?.optLong(2, 0) ?: 0L).coerceAtLeast(1).toFloat()

        val text = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = palette.dim
            textSize = (height * 0.075f).coerceIn(18f, 34f)
        }
        val left = text.measureText(top.toLong().toString()) + 10f
        val bottom = height - text.textSize * 1.7f
        val area = RectF(left, text.textSize * 0.6f, width - 4f, bottom)

        // The axis: nothing, half, and the top, as Taskwarrior rounds them.
        val grid = Paint().apply {
            color = palette.line
            strokeWidth = 1.5f
        }
        for (i in 0..2) {
            val value = labels?.optLong(i, 0) ?: 0
            val y = area.bottom - (value / top) * area.height()
            canvas.drawLine(area.left, y, area.right, y, grid)
            canvas.drawText(value.toString(), 2f, y + text.textSize * 0.35f, text)
        }

        if (bars.isEmpty()) return bitmap
        val slot = area.width() / bars.size
        val gap = (slot * 0.18f).coerceAtMost(4f)
        val fill = Paint()
        for ((i, bar) in bars.withIndex()) {
            val x0 = area.left + i * slot + gap / 2
            val x1 = area.left + (i + 1) * slot - gap / 2
            var y = area.bottom
            for ((count, color) in listOf(
                (bar.optLong("done", 0) + carryover) to palette.done,
                bar.optLong("started", 0) to palette.started,
                bar.optLong("pending", 0) to palette.pending,
            )) {
                if (count <= 0) continue
                val h = (count / top) * area.height()
                fill.color = color
                canvas.drawRect(x0, y - h, x1, y, fill)
                y -= h
            }
        }

        // A few labels along the bottom: the first from the left edge, the last to the right edge, the middle between.
        val baseline = height - text.textSize * 0.35f
        for (i in listOf(0, bars.size / 2, bars.size - 1).distinct()) {
            val label = bars[i].optString("minor").ifBlank { bars[i].optString("major") }
            when (i) {
                0 -> { text.textAlign = Paint.Align.LEFT; canvas.drawText(label, area.left, baseline, text) }
                bars.size - 1 -> { text.textAlign = Paint.Align.RIGHT; canvas.drawText(label, area.right, baseline, text) }
                else -> { text.textAlign = Paint.Align.CENTER; canvas.drawText(label, area.left + (i + 0.5f) * slot, baseline, text) }
            }
        }
        return bitmap
    }

    /** The line under the chart: what is left, what is done, and when it would be finished at this pace. */
    fun caption(result: JSONObject): String {
        val bars = result.getJSONArray("bars")
        if (bars.length() == 0) return ""
        val last = bars.getJSONObject(bars.length() - 1)
        val done = last.optLong("done", 0) + result.optLong("carryover_done", 0)
        val parts = mutableListOf("${last.optLong("pending", 0)} pending", "$done done")
        result.optJSONObject("completion")?.optString("vague")?.takeIf { it.isNotEmpty() }?.let { parts += "clear in ~$it" }
        return parts.joinToString(" · ")
    }
}
