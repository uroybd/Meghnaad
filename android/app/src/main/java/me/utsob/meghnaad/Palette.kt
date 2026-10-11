package me.utsob.meghnaad

import android.content.Context

/** The colours the drawn widgets use: the text colours follow the phone's light or dark theme. */
class Palette(context: Context) {
    val text = context.getColor(R.color.widget_text)
    val dim = context.getColor(R.color.widget_dim)
    val line = (dim and 0x00FFFFFF) or 0x40000000
    val pending = 0xFFE5534B.toInt()
    val started = 0xFFD29922.toInt()
    val done = 0xFF3FB950.toInt()
    val accent = 0xFF2F81F7.toInt()
    val overdue = 0xFFE5534B.toInt()
    val dueToday = 0xFFD29922.toInt()
    val due = 0xFF58A6FF.toInt()
}
