package me.utsob.meghnaad.app

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonPrimitive

// An iterative task's rule (`iter`) as the Iter picker shows it. Its Taskwarrior mode takes the periods and words `task`
// takes; its Calendar mode builds an RFC 5545 rule (RRULE, `FREQ=…`) the way calendar apps do. The rule is read and
// written by the engine (`tc_core::iter_rule`), as on the web (iterRule.ts) and the terminal.

@Serializable
data class CalendarRule(
    /** `daily`, `weekly`, `monthly` or `yearly`. */
    val freq: String,
    val interval: Int = 1,
    /** Weekly: `MO` … `SU`. */
    val days: List<String> = emptyList(),
    /** Monthly: on this day of the month, or on the `nth` (1 to 4, -1 the last) `weekday`. */
    @SerialName("month_day") val monthDay: Int? = null,
    val nth: Int? = null,
    val weekday: String? = null,
    /** Ends on this day (`2027-12-31`), or after `count` times; neither is never. */
    val until: String? = null,
    val count: Int? = null,
)

@Serializable
data class MonthChoices(val day: Int, val nth: Int, val weekday: String, val last: Boolean)

val WEEKDAYS = listOf("MO", "TU", "WE", "TH", "FR", "SA", "SU")
val DAY_NAMES = listOf("Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday")
private val ORDINALS = listOf("first", "second", "third", "fourth")

/** The rule `value` is, when the Calendar mode can show it; null for anything else (it stays in the Taskwarrior mode). */
fun parseRule(value: String): CalendarRule? =
    callHelper("iter_rule_parse", JsonPrimitive(value)).takeUnless { it is JsonNull }?.let { engineJson.decodeFromJsonElement(CalendarRule.serializer(), it) }

fun buildRule(r: CalendarRule): String =
    (callHelper("iter_rule_build", engineJson.encodeToJsonElement(CalendarRule.serializer(), r)) as JsonPrimitive).content

/** Every week on the weekday of `date` (`2026-12-25`). */
fun defaultRule(date: String): CalendarRule = engineJson.decodeFromJsonElement(CalendarRule.serializer(), callHelper("iter_rule_default", JsonPrimitive(date)))

fun monthChoices(date: String): MonthChoices? =
    callHelper("iter_month_choices", JsonPrimitive(date)).takeUnless { it is JsonNull }?.let { engineJson.decodeFromJsonElement(MonthChoices.serializer(), it) }

/** Any `iter` in words: "Every 2 weeks on Friday", "every 3 days", or as typed. */
fun describeIter(value: String): String = (callHelper("describe_iter", JsonPrimitive(value)) as JsonPrimitive).content

/** "the fourth Friday", "the last Friday". */
fun nthWords(nth: Int, weekday: String): String =
    "the ${if (nth == -1) "last" else ORDINALS.getOrElse(nth - 1) { "first" }} ${DAY_NAMES.getOrElse(WEEKDAYS.indexOf(weekday)) { weekday }}"

/** The day a new rule counts from: the first of `dates` that is a day, else `today`. */
fun anchorOf(dates: List<String>, today: String): String =
    dates.map { it.trim().take(10) }.firstOrNull { Regex("^\\d{4}-\\d{2}-\\d{2}$").matches(it) } ?: today

/** One monthly choice: what it says, and what it sets. */
data class MonthOption(val key: String, val label: String, val apply: (CalendarRule) -> CalendarRule)

/** The monthly choices for the anchor day, and the rule's own first when it is another. */
fun monthOptions(rule: CalendarRule, anchor: String): List<MonthOption> {
    val out = mutableListOf<MonthOption>()
    fun add(key: String, label: String, apply: (CalendarRule) -> CalendarRule) { if (out.none { it.key == key }) out += MonthOption(key, label, apply) }
    rule.monthDay?.let { d -> add("d$d", "On day $d") { it.copy(monthDay = d, nth = null, weekday = null) } }
    if (rule.nth != null && rule.weekday != null) {
        val (n, w) = rule.nth to rule.weekday
        add("n$n$w", "On ${nthWords(n, w)}") { it.copy(nth = n, weekday = w, monthDay = null) }
    }
    monthChoices(anchor)?.let { m ->
        add("d${m.day}", "On day ${m.day}") { it.copy(monthDay = m.day, nth = null, weekday = null) }
        add("n${m.nth}${m.weekday}", "On ${nthWords(m.nth, m.weekday)}") { it.copy(nth = m.nth, weekday = m.weekday, monthDay = null) }
        if (m.last) add("n-1${m.weekday}", "On ${nthWords(-1, m.weekday)}") { it.copy(nth = -1, weekday = m.weekday, monthDay = null) }
    }
    return out
}

/** Which monthly choice `rule` is. */
fun monthKey(rule: CalendarRule): String = if (rule.nth != null && rule.weekday != null) "n${rule.nth}${rule.weekday}" else "d${rule.monthDay ?: ""}"
