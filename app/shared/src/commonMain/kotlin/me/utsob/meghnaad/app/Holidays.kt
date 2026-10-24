package me.utsob.meghnaad.app

import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonPrimitive

// Holidays from Holidata (holidata.net), for the taskrc folder (web/src/lib/holidays.ts). Which locales there are, the years' files and the
// rc text made from them are the engine's (`tc_core::holidata`, through the helpers); the app only fetches the files and puts the result
// in the folder.

/** What a GET answered: the status, and the body (cut short when it is longer than a holiday list can be). */
class HttpAnswer(val status: Int, val body: String)

/** A GET of a public page, with no cookies and no credentials. Throws when the host cannot be reached. */
expect suspend fun httpGet(url: String): HttpAnswer

@Serializable
class HolidayLocale(val locale: String, val label: String)

/** What the engine made from the years' files. */
@Serializable
class HolidaysBuilt(val text: String, val count: Int, val regions: List<String> = emptyList(), val skipped: Int = 0)

/** The files for the years asked for; [missing] are the years Holidata has none for. */
class HolidayYears(val bodies: List<String>, val missing: List<Int>)

/** A year's file is at most this long (a locale has a few dozen holidays a year); more is not Holidata. */
const val HOLIDAY_MAX_BODY = 200_000

fun holidayLocales(): List<HolidayLocale> =
    engineJson.decodeFromJsonElement(ListSerializer(HolidayLocale.serializer()), callHelper("holidata_locales"))

/** Where holidays for a locale are kept in the folder. */
fun holidayPath(locale: String) = "holidays/$locale.rc"

fun buildHolidays(locale: String, bodies: List<String>, regions: List<String>): HolidaysBuilt =
    engineJson.decodeFromJsonElement(
        HolidaysBuilt.serializer(),
        callHelper("holidata_rc", JsonPrimitive(locale), JsonArray(bodies.map(::JsonPrimitive)), JsonArray(regions.map(::JsonPrimitive))),
    )

/** The locale and regions a file made by Add holidays was made for, or null when it is not one. */
fun holidaySettings(text: String): Pair<String, List<String>>? {
    val found = callHelper("holidata_settings", JsonPrimitive(text)) as? JsonArray ?: return null
    val locale = (found[0] as? JsonPrimitive)?.content ?: return null
    val regions = (found[1] as? JsonArray)?.map { (it as JsonPrimitive).content } ?: emptyList()
    return locale to regions
}

/** The files for [first] to [last] of a locale. A year with no file is reported; no file at all, or no answer, throws. */
suspend fun fetchHolidays(locale: String, first: Int, last: Int, get: suspend (String) -> HttpAnswer = ::httpGet): HolidayYears {
    val bodies = mutableListOf<String>()
    val missing = mutableListOf<Int>()
    for (year in first..last) {
        val url = (callHelper("holidata_url", JsonPrimitive(locale), JsonPrimitive(year)) as? JsonPrimitive)?.takeIf { it.isString }?.content
            ?: error("$locale $year is not a year Holidata can have.")
        val answer = try {
            get(url)
        } catch (e: Exception) {
            error("Couldn't reach holidata.net. Check the connection and try again.")
        }
        if (answer.status == 404) {
            missing += year
            continue
        }
        if (answer.status !in 200..299) error("holidata.net answered ${answer.status} for $year.")
        if (answer.body.length > HOLIDAY_MAX_BODY) error("The file for $year is larger than a holiday list should be.")
        bodies += answer.body
    }
    if (bodies.isEmpty()) error("Holidata has no holidays for $locale in those years.")
    return HolidayYears(bodies, missing)
}

/** `name=value` in a taskrc's text: that line's value replaced if it is there, the line added if not. */
fun withSetting(text: String, name: String, value: String): String {
    val lines = text.split("\n").toMutableList()
    val at = lines.indexOfFirst { it.startsWith("$name=") }
    if (at >= 0) {
        lines[at] = "$name=$value"
        return lines.joinToString("\n")
    }
    return text + (if (text.isEmpty() || text.endsWith("\n")) "" else "\n") + "$name=$value\n"
}
