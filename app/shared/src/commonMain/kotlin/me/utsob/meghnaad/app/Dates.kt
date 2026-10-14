package me.utsob.meghnaad.app

import kotlin.math.abs
import kotlin.math.roundToLong

// Date and time helpers: web/src/lib/dates.ts and dateformat.ts. All in a zone given as its offset east of UTC in seconds
// (the engine takes the same), with no platform date classes, so what they print is the same everywhere.

/** A moment in a zone, as the parts a calendar reads. */
class Civil(
    val year: Int,
    /** 0 = January. */
    val month0: Int,
    val day: Int,
    /** 0 = Sunday. */
    val weekday: Int,
    /** 0 = the first of January. */
    val dayOfYear0: Int,
    val hour: Int,
    val minute: Int,
    val second: Int,
)

/** Days since 1970-01-01 of a civil date (Howard Hinnant's algorithm). */
internal fun daysFromCivil(year: Int, month: Int, day: Int): Long {
    val y = if (month <= 2) year - 1 else year
    val era = y.toLong().floorDiv(400)
    val yoe = y - era * 400
    val doy = (153 * (if (month > 2) month - 3 else month + 9) + 2) / 5 + day - 1
    val doe = yoe * 365 + yoe / 4 - yoe / 100 + doy
    return era * 146_097 + doe - 719_468
}

/** The parts of [epoch] (seconds) in the zone [tz] seconds east of UTC. */
fun civil(epoch: Long, tz: Int): Civil {
    val t = epoch + tz
    val days = t.floorDiv(86_400L)
    val secondOfDay = t.mod(86_400L).toInt()
    val z = days + 719_468
    val era = z.floorDiv(146_097L)
    val doe = z - era * 146_097
    val yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365
    val doy = doe - (365 * yoe + yoe / 4 - yoe / 100)
    val mp = (5 * doy + 2) / 153
    val day = (doy - (153 * mp + 2) / 5 + 1).toInt()
    val month = (if (mp < 10) mp + 3 else mp - 9).toInt()
    val year = (yoe + era * 400 + if (month <= 2) 1 else 0).toInt()
    return Civil(
        year = year,
        month0 = month - 1,
        day = day,
        weekday = (days + 4).mod(7L).toInt(),
        dayOfYear0 = (days - daysFromCivil(year, 1, 1)).toInt(),
        hour = secondOfDay / 3600,
        minute = secondOfDay % 3600 / 60,
        second = secondOfDay % 60,
    )
}

private fun pad(n: Int, width: Int = 2) = n.toString().padStart(width, '0')

/** `2026-10-16` for epoch seconds in the zone [offset] seconds east of UTC. */
fun isoDate(seconds: Long, offset: Int): String = civil(seconds, offset).let { "${pad(it.year, 4)}-${pad(it.month0 + 1)}-${pad(it.day)}" }

class DateParts(
    /** `YYYY-MM-DD`, or empty. */
    val date: String,
    /** `HH:MM`, or empty. */
    val time: String,
) {
    override fun equals(other: Any?) = other is DateParts && other.date == date && other.time == time

    override fun hashCode() = date.hashCode() * 31 + time.hashCode()

    override fun toString() = "DateParts($date, $time)"
}

/** Epoch seconds → the local date and (only if not midnight) time. */
fun toParts(epoch: Long?, tz: Int): DateParts {
    if (epoch == null) return DateParts("", "")
    val c = civil(epoch, tz)
    val hasTime = c.hour != 0 || c.minute != 0 || c.second != 0
    return DateParts("${pad(c.year, 4)}-${pad(c.month0 + 1)}-${pad(c.day)}", if (hasTime) "${pad(c.hour)}:${pad(c.minute)}" else "")
}

/** Date and optional time → the wire string the engine's date parser takes; empty means "no value". */
fun fromParts(p: DateParts): String = if (p.date.isEmpty()) "" else if (p.time.isNotEmpty()) "${p.date}T${p.time}" else p.date

fun isValidDate(s: String): Boolean {
    val m = Regex("^(\\d{4})-(\\d{2})-(\\d{2})$").find(s) ?: return false
    val (y, mo, d) = m.groupValues.drop(1).map { it.toInt() }
    if (mo !in 1..12 || d < 1) return false
    val c = civil(daysFromCivil(y, mo, d) * 86_400L, 0)
    return c.year == y && c.month0 == mo - 1 && c.day == d
}

fun isValidTime(s: String): Boolean = Regex("^([01]\\d|2[0-3]):[0-5]\\d$").matches(s)

/** With a taskrc `dateformat` pattern, exactly that; otherwise the date, plus the time only when there is one. */
fun formatMoment(epoch: Long?, tz: Int, fmt: DateFmt? = null): String {
    if (epoch != null && fmt != null) return formatPattern(epoch, fmt, tz)
    val p = toParts(epoch, tz)
    if (p.date.isEmpty()) return ""
    return if (p.time.isNotEmpty()) "${p.date} ${p.time}" else p.date
}

/** Date and time with seconds, for journal sessions: 2026-10-07 12:30:05. */
fun formatStamp(epoch: Long?, tz: Int): String {
    if (epoch == null) return ""
    val c = civil(epoch, tz)
    return "${pad(c.year, 4)}-${pad(c.month0 + 1)}-${pad(c.day)} ${pad(c.hour)}:${pad(c.minute)}:${pad(c.second)}"
}

/** Exact elapsed time for time tracking: 1h 12m, 45s, 2d 3h. */
fun formatSeconds(total: Long): String {
    val s = maxOf(0L, total)
    val d = s / 86_400
    val h = s % 86_400 / 3_600
    val m = s % 3_600 / 60
    val sec = s % 60
    return when {
        d > 0 -> "${d}d ${h}h"
        h > 0 -> "${h}h ${m}m"
        m > 0 -> "${m}m ${sec}s"
        else -> "${sec}s"
    }
}

/** A number with [digits] decimals, rounded as `toFixed` does. */
fun fixed(x: Double, digits: Int): String {
    var scale = 1L
    repeat(digits) { scale *= 10 }
    val r = (abs(x) * scale).roundToLong()
    val whole = r / scale
    val frac = (r % scale).toString().padStart(digits, '0')
    val sign = if (x < 0 && r != 0L) "-" else ""
    return if (digits == 0) "$sign$whole" else "$sign$whole.$frac"
}

/** `2026-10-07T12:30:00Z`, for the `iso` column format. */
fun isoStamp(epoch: Long): String = formatStamp(epoch, 0).replace(' ', 'T') + "Z"
