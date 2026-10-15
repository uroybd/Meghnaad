package me.utsob.meghnaad

/** How a calendar widget is laid out, for the room the home screen gave it. */
enum class CalendarLayout {
    /** The month alone. */
    MONTH,

    /** The month, and beside it the tasks of the day that is selected. */
    WIDE,

    /** The month, and under it the tasks of the day that is selected. */
    TALL,
    ;

    companion object {
        /** The room a month needs to be drawn with six weeks, and a day's tasks (two and "show all") need beside or under it. */
        private const val WIDE_MIN_WIDTH = 320
        private const val WIDE_MIN_HEIGHT = 240
        private const val TALL_MIN_WIDTH = 190
        private const val TALL_MIN_HEIGHT = 360

        /** The layout for a widget [widthDp] wide and [heightDp] high; where both fit, the longer side decides. Unknown (0) is the month. */
        fun choose(widthDp: Int, heightDp: Int): CalendarLayout {
            val wide = widthDp >= WIDE_MIN_WIDTH && heightDp >= WIDE_MIN_HEIGHT
            val tall = widthDp >= TALL_MIN_WIDTH && heightDp >= TALL_MIN_HEIGHT
            return when {
                wide && tall -> if (widthDp >= heightDp) WIDE else TALL
                wide -> WIDE
                tall -> TALL
                else -> MONTH
            }
        }
    }
}
