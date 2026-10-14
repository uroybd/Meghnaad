package me.utsob.meghnaad.app

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject

// The app's own settings (`settings.json`), kept beside the taskrc settings. The engine only checks that the text is a JSON
// object of a sensible size; the app reads it. So far it holds one thing: `taskCommands`, words you type (or tap) that stand
// for a whole command line (web/src/lib/appsettings.ts):
//
//   { "taskCommands": [ { "name": "fail", "command": "done outcome:fail", "icon": "circle-x" } ] }

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

class ParsedSettings(
    /** Every top-level key as it was, so what this app does not know about is kept when it saves. */
    val raw: JsonObject,
    /** The valid task commands, in order. */
    val commands: List<TaskCommand>,
    /** What is wrong, one line each; a command with a problem is left out. */
    val errors: List<String>,
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
    val list = value["taskCommands"] ?: return ParsedSettings(value, emptyList(), emptyList())
    if (list !is JsonArray) return ParsedSettings(value, emptyList(), listOf("taskCommands must be a list"))
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
        if (command == null || command.isBlank()) {
            errors += "$at ($name): needs a command"
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
    return ParsedSettings(value, commands, errors)
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

