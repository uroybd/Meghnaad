package me.utsob.meghnaad.app

import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.decodeFromJsonElement

// A due or scheduled date that falls on a holiday: what it is is the engine's (`tc_core::clash`, through `callHelper`), the same on
// every front end. A date is resolved first (`eod`, `som`, `eow-2d`), in the user's zone, then looked up in the `holiday.*` settings
// (holidayClash.ts).

/** A date on a holiday: the moment it resolves to, the holidays of that day, and what to say about it. */
@Serializable
data class HolidayClash(val date: Long, val names: List<String>, val message: String)

/** A date word of a command line, `[start, end)` in UTF-16 units (as the highlighter counts), that is on a holiday. */
@Serializable
data class ClashMark(val start: Int, val end: Int, val attr: String, val date: Long, val names: List<String>, val message: String)

/** Holidays are defined: nothing is asked of the engine (the settings go with each question) when there are none. */
private fun defined(config: ConfigBody?) = config?.settings?.keys?.any { it.startsWith("holiday.") } == true

/** Whether [attr] (`due` or `scheduled`) set to [value] (a date, or an expression) is on a holiday. */
fun holidayClash(attr: String, value: String, config: ConfigBody?, now: Long, tz: Int): HolidayClash? {
    if (value.isBlank() || !defined(config)) return null
    val r = callHelper("holiday_clash", JsonPrimitive(attr), JsonPrimitive(value), addConfig(config), JsonPrimitive(now), JsonPrimitive(tz))
    return if (r is JsonNull) null else engineJson.decodeFromJsonElement(HolidayClash.serializer(), r)
}

/** The date words of a command line that are on a holiday: what `add`, `log`, `modify` and `duplicate` would set. */
fun holidayMarks(line: String, commands: Set<String>, assumeAdd: Boolean, config: ConfigBody?, now: Long, tz: Int): List<ClashMark> {
    if (':' !in line || !defined(config)) return emptyList()
    val r = callHelper(
        "holiday_marks", JsonPrimitive(line), JsonArray(commands.map { JsonPrimitive(it) }), JsonPrimitive(assumeAdd),
        addConfig(config), JsonPrimitive(now), JsonPrimitive(tz),
    )
    return engineJson.decodeFromJsonElement(ListSerializer(ClashMark.serializer()), r)
}
