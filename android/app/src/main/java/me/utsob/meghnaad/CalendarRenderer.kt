package me.utsob.meghnaad

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import org.json.JSONObject

/** Draws one month of the engine's `calendar` answer: the days, today in a circle, and a dot on the days with something due. */
object CalendarRenderer {
    fun draw(month: JSONObject, weekdays: List<String>, width: Int, height: Int, palette: Palette): Bitmap {
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)
        val weeks = month.getJSONArray("weeks")
        val rows = weeks.length().coerceAtLeast(1)
        val cellW = width / 7f
        val head = (height / (rows + 1.4f)).coerceAtMost(cellW) * 0.7f
        val cellH = (height - head) / rows
        val size = minOf(cellW, cellH) * 0.42f

        val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            textAlign = Paint.Align.CENTER
            textSize = size * 0.8f
            color = palette.dim
        }
        for ((i, name) in weekdays.withIndex()) canvas.drawText(name, (i + 0.5f) * cellW, head * 0.7f, paint)

        paint.textSize = size
        val fill = Paint(Paint.ANTI_ALIAS_FLAG)
        for (w in 0 until weeks.length()) {
            val days = weeks.getJSONObject(w).getJSONArray("days")
            for (d in 0 until days.length()) {
                if (days.isNull(d)) continue
                val day = days.getJSONObject(d)
                val cx = (d + 0.5f) * cellW
                val cy = head + (w + 0.5f) * cellH
                val today = day.optBoolean("today")
                if (today) {
                    fill.color = palette.accent
                    canvas.drawCircle(cx, cy - size * 0.1f, size * 1.05f, fill)
                }
                paint.color = when {
                    today -> 0xFFFFFFFF.toInt()
                    day.optBoolean("weekend") || day.optBoolean("holiday") -> palette.dim
                    else -> palette.text
                }
                paint.isFakeBoldText = today
                canvas.drawText(day.getInt("day").toString(), cx, cy + size * 0.3f, paint)
                dotColor(day.optString("due"), palette)?.let {
                    fill.color = if (today) 0xFFFFFFFF.toInt() else it
                    canvas.drawCircle(cx, cy + size * 1.05f, size * 0.17f, fill)
                }
            }
        }
        return bitmap
    }

    private fun dotColor(state: String, palette: Palette): Int? = when (state) {
        "overdue" -> palette.overdue
        "due-today" -> palette.dueToday
        "due" -> palette.due
        else -> null
    }

    /** How many days have something due, and how many of them are overdue, for the line under the month. */
    fun caption(month: JSONObject): String {
        var due = 0
        var overdue = 0
        val weeks = month.getJSONArray("weeks")
        for (w in 0 until weeks.length()) {
            val days = weeks.getJSONObject(w).getJSONArray("days")
            for (d in 0 until days.length()) {
                if (days.isNull(d)) continue
                when (days.getJSONObject(d).optString("due")) {
                    "overdue" -> { overdue++; due++ }
                    "due-today", "due" -> due++
                }
            }
        }
        return when {
            due == 0 -> "Nothing due this month"
            overdue > 0 -> "$due days with something due · $overdue overdue"
            else -> "$due days with something due"
        }
    }
}
