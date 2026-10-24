package me.utsob.meghnaad.app

import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

// Holidays for the taskrc folder, from Holidata, Nager.Date or a calendar link (web/src/lib/holidays.ts). What each source's answers
// mean, the file made from them and which countries there are, are the engine's (`tc_core::holidata`, `nager`, `ics`, through the
// helpers); the app only fetches the files and puts the result in the folder.

/** What a GET answered: the status, the redirect it names if any, and the body (cut short at the limit asked for). */
class HttpAnswer(val status: Int, val body: String, val location: String? = null)

/** A GET of a public page, with no cookies and no credentials, reading at most [limit] characters. Throws when it cannot be reached. */
expect suspend fun httpGet(url: String, limit: Int): HttpAnswer

enum class HolidaySource(val id: String, val label: String) {
    Holidata("holidata", "Holidata (42 countries)"),
    Nager("nager", "Nager.Date (200+ countries)"),
    Ics("ics", "Calendar link (.ics)"),
}

/** A country to choose: a Holidata locale (`en-US`) or a Nager.Date code (`BD`). */
class HolidayPlace(val id: String, val label: String)

@Serializable
private class HolidayLocale(val locale: String, val label: String)

@Serializable
private class NagerCountry(val code: String, val name: String)

@Serializable
class HolidayEntry(val date: String, val end: String? = null, val name: String)

/** A name in a calendar, what the calendar calls it, and whether it is left out unless asked for. */
@Serializable
class CalendarName(val name: String, val count: Int = 1, val kind: String = "", val skip: Boolean = false)

/** What the engine made from a source's answers. */
@Serializable
class HolidaysBuilt(
    val path: String,
    val key: String = "",
    val text: String,
    val count: Int,
    val regions: List<String> = emptyList(),
    val skipped: Int = 0,
    val entries: List<HolidayEntry> = emptyList(),
    val names: List<CalendarName> = emptyList(),
    val title: String = "",
)

/** What a file made by Add holidays says it was made for. */
@Serializable
class HolidayMade(
    val source: String,
    val key: String,
    val regions: List<String> = emptyList(),
    val skip: List<String> = emptyList(),
    val link: String? = null,
)

/** The files for the years asked for; [missing] are the years the source has none for. */
class HolidayYears(val bodies: List<String>, val missing: List<Int>)

/** A year's file is at most this long (a locale has a few dozen holidays a year); more is not what it should be. */
const val HOLIDAY_MAX_BODY = 200_000

/** A calendar is at most this long (the Worker's limit for the page). */
const val HOLIDAY_MAX_CALENDAR = 1024 * 1024

typealias HolidayGet = suspend (url: String, limit: Int) -> HttpAnswer

private val str = { s: String -> JsonPrimitive(s) }
private fun strings(list: List<String>) = JsonArray(list.map(str))

fun holidataPlaces(): List<HolidayPlace> =
    engineJson.decodeFromJsonElement(ListSerializer(HolidayLocale.serializer()), callHelper("holidata_locales"))
        .map { HolidayPlace(it.locale, it.label) }

/** The countries Nager.Date has holidays for. */
suspend fun nagerPlaces(get: HolidayGet = ::httpGet): List<HolidayPlace> {
    val url = (callHelper("nager_countries_url") as JsonPrimitive).content
    val answer = try {
        get(url, HOLIDAY_MAX_BODY)
    } catch (e: Exception) {
        error("Couldn't reach date.nager.at. Check the connection and try again.")
    }
    if (answer.status !in 200..299) error("date.nager.at answered ${answer.status}.")
    return engineJson.decodeFromJsonElement(ListSerializer(NagerCountry.serializer()), callHelper("nager_countries", str(answer.body)))
        .map { HolidayPlace(it.code, it.name) }
}

fun holidaySettings(text: String): HolidayMade? {
    val found = callHelper("holidays_settings", str(text))
    return if (found is JsonNull) null else engineJson.decodeFromJsonElement(HolidayMade.serializer(), found)
}

private suspend fun years(what: String, helper: String, first: Int, last: Int, get: HolidayGet): HolidayYears {
    val bodies = mutableListOf<String>()
    val missing = mutableListOf<Int>()
    for (year in first..last) {
        val url = (callHelper(helper, str(what), JsonPrimitive(year)) as? JsonPrimitive)?.takeIf { it.isString }?.content
            ?: error("$what $year is not a year that can be asked for.")
        val host = url.removePrefix("https://").substringBefore('/')
        val answer = try {
            get(url, HOLIDAY_MAX_BODY + 1)
        } catch (e: Exception) {
            error("Couldn't reach $host. Check the connection and try again.")
        }
        if (answer.status == 404) {
            missing += year
            continue
        }
        if (answer.status !in 200..299) error("$host answered ${answer.status} for $year.")
        if (answer.body.length > HOLIDAY_MAX_BODY) error("$host sent more than a holiday list should be.")
        bodies += answer.body
    }
    if (bodies.isEmpty()) error("There are no holidays for $what in those years.")
    return HolidayYears(bodies, missing)
}

suspend fun fetchHolidata(locale: String, first: Int, last: Int, get: HolidayGet = ::httpGet) = years(locale, "holidata_url", first, last, get)

suspend fun fetchNager(country: String, first: Int, last: Int, get: HolidayGet = ::httpGet) = years(country, "nager_url", first, last, get)

private fun built(element: kotlinx.serialization.json.JsonElement) = engineJson.decodeFromJsonElement(HolidaysBuilt.serializer(), element)

fun buildHolidata(locale: String, bodies: List<String>, regions: List<String>) =
    built(callHelper("holidata_rc", str(locale), strings(bodies), strings(regions)))

fun buildNager(country: String, bodies: List<String>, regions: List<String>) =
    built(callHelper("nager_rc", str(country), strings(bodies), strings(regions)))

/** The file for a calendar's events in [first] to [last]; [skip] is the names to leave out (null: its observances), [id] names the file (empty: its own name). */
fun buildCalendar(id: String, text: String, first: Int, last: Int, skip: List<String>?, link: String?) =
    built(
        callHelper(
            "ics_rc", str(id), str(text), JsonPrimitive(first), JsonPrimitive(last),
            skip?.let(::strings) ?: JsonNull, link?.let(str) ?: JsonNull,
        ),
    )

/** A link for a calendar as the Worker would take it (`webcal://` is `https://`); throws what is wrong with it. */
fun calendarLink(link: String): String =
    linkAnswer(callHelper("holiday_link", str(link.trim().replace(Regex("^webcal://", RegexOption.IGNORE_CASE), "https://"))))

private fun linkAnswer(answer: kotlinx.serialization.json.JsonElement): String {
    val o = answer as JsonObject
    (o["problem"] as? JsonPrimitive)?.let { error(it.content) }
    return (o["link"] as JsonPrimitive).content
}

/** The calendar at [link], with up to three redirects followed, each held to the same rules as the first link. */
suspend fun fetchCalendar(link: String, get: HolidayGet = ::httpGet): String {
    var url = calendarLink(link)
    repeat(4) {
        val answer = try {
            get(url, HOLIDAY_MAX_CALENDAR + 1)
        } catch (e: Exception) {
            error("Couldn't reach that address. Check the link and the connection.")
        }
        if (answer.status in 300..399 && answer.location != null) {
            url = linkAnswer(callHelper("holiday_follow", str(url), str(answer.location)))
            return@repeat
        }
        if (answer.status != 200) error("That address answered ${answer.status}, not a file.")
        if (answer.body.length > HOLIDAY_MAX_CALENDAR) error("That file is too large.")
        if (!Regex("^\\s*BEGIN:VCALENDAR", RegexOption.IGNORE_CASE).containsMatchIn(answer.body)) error("That link is not a calendar (.ics) file.")
        return answer.body
    }
    error("That address redirects too many times.")
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
