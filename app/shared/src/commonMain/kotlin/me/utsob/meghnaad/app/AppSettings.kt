package me.utsob.meghnaad.app

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject

// The app's own settings (`settings.json`), kept beside the taskrc settings. The engine only checks that the text is a JSON
// object of a sensible size; the app reads it. It holds `taskCommands`, words you type (or tap) that stand for a whole
// command line, `boards` (see Boards.kt) and `homepage`, where the app opens (web/src/lib/appsettings.ts):
//
//   { "taskCommands": [ { "name": "fail", "command": "done outcome:fail", "icon": "circle-x" } ],
//     "boards": [ … ], "homepage": { "board": "week" } }

/** The most the engine saves. */
const val MAX_SETTINGS_BYTES = 64 * 1024

data class TaskCommand(
    /** The word that stands for [command]. */
    val name: String,
    /** What it stands for: a command line, with or without a filter of its own. */
    val command: String,
    /** A Lucide icon, by its name (`circle-x`), for the buttons that run it. */
    val icon: String? = null,
)

/** The page the app opens on: a report or a board. Nothing set is `next`. */
data class Homepage(val report: String? = null, val board: String? = null)

/** A saved `homepage` that is a report or a board, one of the two; anything else is no choice. */
fun parseHomepage(v: JsonElement?): Homepage? {
    val o = v as? JsonObject ?: return null
    if (o.keys.any { it != "report" && it != "board" }) return null
    val report = o["report"]?.text()?.trim()?.takeIf { it.isNotEmpty() }
    val board = o["board"]?.text()?.trim()?.takeIf { it.isNotEmpty() }
    if ((report == null) == (board == null)) return null
    return Homepage(report, board)
}

class ParsedSettings(
    /** Every top-level key as it was, so what this app does not know about is kept when it saves. */
    val raw: JsonObject,
    /** The valid task commands, in order. */
    val commands: List<TaskCommand>,
    /** What is wrong, one line each; a command with a problem is left out. */
    val errors: List<String>,
    /** The valid boards, in order, as the engine tidied them. */
    val boards: List<Board> = emptyList(),
    /** What is wrong with the boards; a board or section with a problem is left out. */
    val boardErrors: List<String> = emptyList(),
    /** Where the app opens, if the settings say. */
    val homepage: Homepage? = null,
)

/** A Lucide icon's name: lower case words joined by dashes (`circle-x`, `arrow-up-0-1`). */
val ICON_NAME = Regex("^[a-z0-9]+(-[a-z0-9]+)*$")

/**
 * Why [name] cannot be a task command, or null. It has to be one word that cannot be taken for part of a filter (a number,
 * `+tag`, `attribute:value`), because the line is read word by word.
 */
fun nameProblem(name: String): String? {
    if (name.isEmpty()) return "needs a name"
    if (name.any { it.isWhitespace() }) return "a name is one word (no spaces)"
    if (Regex("^[+\\-/(]").containsMatchIn(name) || Regex("[:='\"]").containsMatchIn(name)) {
        return "a name can’t start with + - / ( or contain : = or quotes"
    }
    if (Regex("^[\\d,.]+$").matches(name)) return "a name can’t be a number"
    if (name == "task" || name == "--") return "‘$name’ can’t be a name"
    return null
}

/** Why what a command stands for is not a command line (`done &&` has nothing after the `&&`), or null. */
fun commandProblem(command: String): String? {
    if (command.isBlank()) return "needs a command"
    return try {
        splitChain(command)
        null
    } catch (e: ChainError) {
        e.message
    }
}

private fun JsonElement.text(): String? = (this as? JsonPrimitive)?.takeIf { it.isString }?.content

fun parseSettings(text: String): ParsedSettings {
    val none = ParsedSettings(JsonObject(emptyMap()), emptyList(), emptyList())
    if (text.isBlank()) return none
    val value = try {
        Json.parseToJsonElement(text)
    } catch (e: Exception) {
        return ParsedSettings(none.raw, emptyList(), listOf("Not valid JSON: ${e.message}"))
    }
    if (value !is JsonObject) {
        return ParsedSettings(none.raw, emptyList(), listOf("The settings must be a JSON object: { \"taskCommands\": [ … ] }"))
    }
    val homepage = parseHomepage(value["homepage"])
    val normalized = value["boards"]?.let { normalizeBoards(it) }
    val boards = normalized?.boards ?: emptyList()
    val boardErrors = normalized?.errors ?: emptyList()
    val list = value["taskCommands"] ?: return ParsedSettings(value, emptyList(), emptyList(), boards, boardErrors, homepage)
    if (list !is JsonArray) return ParsedSettings(value, emptyList(), listOf("taskCommands must be a list"), boards, boardErrors, homepage)
    val errors = mutableListOf<String>()
    val commands = mutableListOf<TaskCommand>()
    val seen = HashSet<String>()
    list.forEachIndexed { i, c ->
        val at = "taskCommands[$i]"
        if (c !is JsonObject) {
            errors += "$at: must be { \"name\": …, \"command\": … }"
            return@forEachIndexed
        }
        val name = c["name"]?.text()
        if (name == null) {
            errors += "$at: name must be text"
            return@forEachIndexed
        }
        nameProblem(name)?.let {
            errors += "$at (${name.ifEmpty { "no name" }}): $it"
            return@forEachIndexed
        }
        val command = c["command"]?.text()
        val wrong = if (command == null) "needs a command" else commandProblem(command)
        if (command == null || wrong != null) {
            errors += "$at ($name): $wrong"
            return@forEachIndexed
        }
        val iconElement = c["icon"]
        val icon = iconElement?.text()
        if (iconElement != null && (icon == null || !ICON_NAME.matches(icon))) {
            errors += "$at ($name): icon must be a Lucide icon name such as \"circle-x\""
            return@forEachIndexed
        }
        if (!seen.add(name)) {
            errors += "$at: there is already a command named $name"
            return@forEachIndexed
        }
        commands += TaskCommand(name, command.trim(), icon)
    }
    return ParsedSettings(value, commands, errors, boards, boardErrors, homepage)
}

private val pretty = Json { prettyPrint = true; prettyPrintIndent = "  " }

/** The text to save: the other top-level keys as they were, and `taskCommands` as given. */
fun serializeSettings(raw: JsonObject, commands: List<TaskCommand>): String {
    val next = buildJsonObject {
        for ((k, v) in raw) if (k != "taskCommands") put(k, v)
        if (commands.isNotEmpty()) {
            put(
                "taskCommands",
                JsonArray(
                    commands.map { c ->
                        buildJsonObject {
                            put("name", JsonPrimitive(c.name))
                            put("command", JsonPrimitive(c.command))
                            c.icon?.let { put("icon", JsonPrimitive(it)) }
                        }
                    },
                ),
            )
        }
    }
    return pretty.encodeToString(JsonObject.serializer(), next) + "\n"
}


/**
 * The text to save: the other top-level keys as they were, and `boards` as given (none leaves the key out). With [setHomepage],
 * `homepage` is [homepage] (none takes it out); without, it is left as it was, so a board whose slug changed can take the home
 * page with it in the same write.
 */
fun serializeBoards(raw: JsonObject, boards: List<Board>, homepage: Homepage? = null, setHomepage: Boolean = false): String {
    val next = buildJsonObject {
        for ((k, v) in raw) if (k != "boards" && k != "boardsVersion" && !(setHomepage && k == "homepage")) put(k, v)
        if (setHomepage && homepage != null) put("homepage", homepageJson(homepage))
        if (boards.isNotEmpty()) {
            put("boardsVersion", JsonPrimitive(1))
            put("boards", boardsToJson(boards))
        }
    }
    return pretty.encodeToString(JsonObject.serializer(), next) + "\n"
}

/** The text to save: the other top-level keys as they were, and `homepage` as given (none takes the key out). */
fun serializeHomepage(raw: JsonObject, homepage: Homepage?): String {
    val next = buildJsonObject {
        for ((k, v) in raw) if (k != "homepage") put(k, v)
        if (homepage != null) put("homepage", homepageJson(homepage))
    }
    return pretty.encodeToString(JsonObject.serializer(), next) + "\n"
}

private fun homepageJson(h: Homepage): JsonObject = buildJsonObject {
    h.report?.let { put("report", JsonPrimitive(it)) }
    h.board?.let { put("board", JsonPrimitive(it)) }
}
