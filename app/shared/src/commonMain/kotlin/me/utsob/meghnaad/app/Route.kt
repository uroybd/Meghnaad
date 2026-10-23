package me.utsob.meghnaad.app

// Every page has a link, `meghnaad://…`: web/src/lib/route.ts, with the app's own scheme where the web has its address, so a link can
// be pasted into a note or a message and opens the page:
//
//   meghnaad://report/<name>        Tasks, as that report (a report is its own slug)
//   meghnaad://projects  tags  summary  calendar  burndown  console
//   meghnaad://boards               the list of boards;  meghnaad://board/<slug>  one of them
//   meghnaad://settings             Settings;  meghnaad://settings/<tab>  one of its tabs;  meghnaad://settings/taskrc/<path>  one file
//                                   of the taskrc folder
//   meghnaad://task/<8 characters>  a task's detail (the first 8 characters of its uuid; the numbers `task 3` uses differ from one
//                                   replica to the next, so the uuid is how a task is named everywhere else)

const val TASK_LINK_PREFIX = "meghnaad://task/"

private val TASK_LINK = Regex("^meghnaad://task/([0-9a-f][0-9a-f-]{7,35})/?$", RegexOption.IGNORE_CASE)

/** The link that opens this task. */
fun taskLink(uuid: String): String = TASK_LINK_PREFIX + uuid.take(8)

/** The uuid characters in a link like `meghnaad://task/1a2b3c4d`, or `null` when the text is not a task's link. */
fun prefixFromLink(link: String): String? = TASK_LINK.find(link)?.groupValues?.get(1)?.lowercase()

/**
 * The filter that finds a task by the start of its uuid. A bare `12345678` would be read as task numbers, so the attribute form
 * is used; it works for any prefix.
 */
fun prefixFilter(prefix: String): String = "uuid.startswith:$prefix"

/** Where the app can be sent by a link. */
sealed interface AppPlace {
    data class Report(val name: String) : AppPlace

    /** One of the pages that is just a page. */
    data class Page(val view: View) : AppPlace

    /** The list of boards, or one of them by its slug. */
    data class Boards(val slug: String? = null) : AppPlace

    /** Settings, on a tab; on the taskrc tab, [file] is the file of the folder it shows (null: the main taskrc). */
    data class SettingsAt(val tab: SettingsTab? = null, val file: String? = null) : AppPlace

    data class TaskAt(val prefix: String) : AppPlace
}

private const val LINK_SCHEME = "meghnaad://"

private val PLAIN_PAGES = mapOf(
    "projects" to View.Projects,
    "tags" to View.Tags,
    "summary" to View.Summary,
    "calendar" to View.Calendar,
    "burndown" to View.Burndown,
    "console" to View.Console,
)

private val BOARD_SLUG = Regex("^[a-z0-9_-]{1,40}$")

/** The text of a link's part, with `%xx` escapes (UTF-8) read; null when an escape is broken. */
fun percentDecode(text: String): String? {
    val out = ArrayList<Byte>(text.length)
    var i = 0
    while (i < text.length) {
        val c = text[i]
        if (c == '%') {
            val byte = text.substring(i + 1, minOf(i + 3, text.length)).takeIf { it.length == 2 }?.toIntOrNull(16) ?: return null
            out.add(byte.toByte())
            i += 3
        } else {
            out.addAll(c.toString().encodeToByteArray().toList())
            i++
        }
    }
    return out.toByteArray().decodeToString()
}

/** A report's name or such as a part of a link: everything but letters, digits and `-_.~` is `%xx`, as the web's `encodeURIComponent` does. */
fun percentEncode(text: String): String = buildString {
    for (b in text.encodeToByteArray()) {
        val c = (b.toInt() and 0xFF).toChar()
        if (c.isLetterOrDigit() && c.code < 128 || c in "-_.~") append(c) else append('%').append((b.toInt() and 0xFF).toString(16).uppercase().padStart(2, '0'))
    }
}

/** The place a link names, or null when the text is not a link of this app's. */
fun parseLink(link: String): AppPlace? {
    if (!link.startsWith(LINK_SCHEME)) return null
    val parts = link.removePrefix(LINK_SCHEME).trimEnd('/').split('/')
    val head = parts[0]
    val rest = parts.drop(1)
    return when {
        head == "task" && rest.size == 1 -> prefixFromLink(link)?.let { AppPlace.TaskAt(it) }
        head == "report" && rest.size == 1 -> percentDecode(rest[0])?.takeIf { it.isNotEmpty() }?.let { AppPlace.Report(it) }
        head == "boards" && rest.isEmpty() -> AppPlace.Boards()
        head == "board" && rest.size == 1 -> rest[0].takeIf { BOARD_SLUG.matches(it) }?.let { AppPlace.Boards(it) }
        head == "settings" && rest.isEmpty() -> AppPlace.SettingsAt()
        head == "settings" && rest.size == 1 -> SettingsTab.entries.firstOrNull { it.name.lowercase() == rest[0] }?.let { AppPlace.SettingsAt(it) }
        head == "settings" && rest.size > 1 && rest[0] == "taskrc" -> rcFileFromLink(rest.drop(1))?.let { AppPlace.SettingsAt(SettingsTab.Taskrc, it) }
        rest.isEmpty() -> PLAIN_PAGES[head]?.let { AppPlace.Page(it) }
        else -> null
    }
}

/** The link of a place. */
fun placeLink(place: AppPlace): String = LINK_SCHEME + when (place) {
    is AppPlace.Report -> "report/${percentEncode(place.name)}"
    is AppPlace.Page -> PLAIN_PAGES.entries.first { it.value == place.view }.key
    is AppPlace.Boards -> if (place.slug == null) "boards" else "board/${place.slug}"
    is AppPlace.SettingsAt -> when {
        place.tab == null -> "settings"
        place.tab == SettingsTab.Taskrc && place.file != null -> "settings/taskrc/" + place.file.split('/').joinToString("/") { percentEncode(it) }
        else -> "settings/${place.tab.name.lowercase()}"
    }
    is AppPlace.TaskAt -> "task/${place.prefix}"
}

/** A file of the taskrc folder named by the parts of a link: none empty, `.` or `..` (what the engine takes as a path). */
private fun rcFileFromLink(parts: List<String>): String? {
    val decoded = parts.map { percentDecode(it) ?: return null }
    return decoded.takeIf { d -> d.none { it.isEmpty() || it == "." || it == ".." || '\\' in it } }?.joinToString("/")
}
