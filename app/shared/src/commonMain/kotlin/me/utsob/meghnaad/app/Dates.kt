package me.utsob.meghnaad.app

/** `2026-10-16` for epoch seconds, in the time zone [offset] seconds east of UTC. (Days to civil date: Howard Hinnant's.) */
fun isoDate(seconds: Long, offset: Int): String {
    val days = (seconds + offset).floorDiv(86_400L) + 719_468
    val era = days.floorDiv(146_097L)
    val dayOfEra = days - era * 146_097
    val yearOfEra = (dayOfEra - dayOfEra / 1_460 + dayOfEra / 36_524 - dayOfEra / 146_096) / 365
    val dayOfYear = dayOfEra - (365 * yearOfEra + yearOfEra / 4 - yearOfEra / 100)
    val mp = (5 * dayOfYear + 2) / 153
    val day = dayOfYear - (153 * mp + 2) / 5 + 1
    val month = if (mp < 10) mp + 3 else mp - 9
    val year = yearOfEra + era * 400 + if (month <= 2) 1 else 0
    return "${year.toString().padStart(4, '0')}-${month.toString().padStart(2, '0')}-${day.toString().padStart(2, '0')}"
}
