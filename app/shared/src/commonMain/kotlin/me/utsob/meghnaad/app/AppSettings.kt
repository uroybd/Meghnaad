package me.utsob.meghnaad.app

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

// The app's own settings (`settings.json`), kept beside the taskrc settings. The engine only checks that the text is a JSON
// object of a sensible size; the app reads it. It holds `taskCommands`, words you type (or tap) that stand for a whole
// command line, `boards` (see Boards.kt) and `homepage`, where the app opens (web/src/lib/appsettings.ts):
//
//   { "taskCommands": [ { "name": "fail", "command": "done outcome:fail", "icon": "circle-x" } ],
//     "boards": [ … ], "homepage": { "board": "week" },
//     "appearance": { "progress": { "progress": { "min": 0, "max": 100 } } } }

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

/** A UDA shown as a progress bar: the value at which the bar is empty and the value at which it is full. */
data class Progress(val min: Double, val max: Double) {
    /** How far along [value] is, from 0 (at or under [min]) to 1 (at or over [max]). */
    fun fraction(value: Double): Float = if (value.isNaN()) 0f else ((value - min) / (max - min)).coerceIn(0.0, 1.0).toFloat()

    /** Why this can't be the range of a bar, or null. */
    fun problem(): String? = when {
        !min.isFinite() || !max.isFinite() -> "min and max must be numbers"
        min >= max -> "min must be less than max"
        else -> null
    }
}

/**
 * A find-and-replace run on a text field before it is drawn as Markdown (`tc_core::transform`): [match] is a regex,
 * [replace] may use `$1` and `${name}`, and [fields] names where it runs (`description`, `annotations` or a string UDA).
 */
data class Transformation(val match: String, val replace: String, val fields: List<String>)

/** How the app draws things, kept in `settings.json` under `appearance`. */
data class Appearance(
    /** The numeric UDAs shown as progress bars in reports, by name. */
    val progress: Map<String, Progress> = emptyMap(),
    /** The field transformations, in the order they run. */
    val transformations: List<Transformation> = emptyList(),
    /** `idFallback`: a task with no ID (done, deleted) shows its short uuid, dimmed, rather than `task`'s `-`. */
    val idFallback: Boolean = false,
)

val DEFAULT_PROGRESS = Progress(0.0, 100.0)

/** The valid part of a saved `appearance`, and what is wrong with the rest, one line each. */
fun parseAppearance(v: JsonElement?): Pair<Appearance, List<String>> {
    if (v == null) return Appearance() to emptyList()
    val o = v as? JsonObject ?: return Appearance() to listOf(APPEARANCE_NOT_OBJECT)
    val (rules, ruleErrors) = parseTransformations(o["transformations"])
    val fallbackValue = o["idFallback"]
    val fallback = (fallbackValue as? JsonPrimitive)?.takeIf { !it.isString }?.content?.toBooleanStrictOrNull()
    val fallbackErrors = if (fallbackValue != null && fallback == null) listOf("appearance.idFallback must be true or false") else emptyList()
    val idFallback = fallback ?: false
    val list = o["progress"] ?: return Appearance(transformations = rules, idFallback = idFallback) to ruleErrors + fallbackErrors
    val bars = list as? JsonObject
        ?: return Appearance(transformations = rules, idFallback = idFallback) to
            ruleErrors + fallbackErrors + PROGRESS_NOT_OBJECT
    val errors = (ruleErrors + fallbackErrors).toMutableList()
    val out = LinkedHashMap<String, Progress>()
    for ((name, r) in bars) {
        val at = "appearance.progress.$name"
        val range = r as? JsonObject
        if (range == null) {
            errors += "$at: must be { \"min\": …, \"max\": … }"
            continue
        }
        val min = range["min"]?.let { (it as? JsonPrimitive)?.takeIf { p -> !p.isString }?.content?.toDoubleOrNull() ?: Double.NaN } ?: DEFAULT_PROGRESS.min
        val max = range["max"]?.let { (it as? JsonPrimitive)?.takeIf { p -> !p.isString }?.content?.toDoubleOrNull() ?: Double.NaN } ?: DEFAULT_PROGRESS.max
        val p = Progress(min, max)
        val bad = p.problem()
        if (bad != null) errors += "$at: $bad" else out[name] = p
    }
    return Appearance(out, rules, idFallback) to errors
}

/**
 * The well-formed transformations, and what is wrong with the rest. Only the shape is checked here; whether a pattern
 * compiles and the fields exist is the engine's to say ([transformProblems]), and such a rule is kept so it can be fixed.
 */
fun parseTransformations(v: JsonElement?): Pair<List<Transformation>, List<String>> {
    if (v == null) return emptyList<Transformation>() to emptyList()
    val list = v as? JsonArray ?: return emptyList<Transformation>() to
        listOf(TRANSFORMATIONS_NOT_LIST)
    val out = mutableListOf<Transformation>()
    val errors = mutableListOf<String>()
    list.forEachIndexed { i, r ->
        val at = "appearance.transformations[$i]"
        val o = r as? JsonObject
        val match = o?.get("match")?.let { (it as? JsonPrimitive)?.takeIf { p -> p.isString }?.content }
        val replace = o?.get("replace").let { if (it == null) "" else (it as? JsonPrimitive)?.takeIf { p -> p.isString }?.content }
        val fields = (o?.get("fields") as? JsonArray)?.map { (it as? JsonPrimitive)?.takeIf { p -> p.isString }?.content }
        when {
            match == null || replace == null -> errors += "$at: must be { \"match\": \"…\", \"replace\": \"…\", \"fields\": [ … ] }"
            fields == null || fields.any { it == null } -> errors += "$at: fields must be a list of field names"
            else -> out += Transformation(match, replace, fields.filterNotNull())
        }
    }
    return out to errors
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
    /** How the app draws things (the valid part of `appearance`). */
    val appearance: Appearance = Appearance(),
    /** What is wrong with `appearance`, one line each. */
    val appearanceErrors: List<String> = emptyList(),
    /** The kanbans (spec 0026): whether they are on, and the valid ones. */
    val kanban: KanbanSettings = KanbanSettings(),
    /** What is wrong with `kanban`, one line each; a kanban with a problem is left out. */
    val kanbanErrors: List<String> = emptyList(),
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
        return ParsedSettings(none.raw, emptyList(), listOf(NOT_SETTINGS))
    }
    val homepage = parseHomepage(value["homepage"])
    val (appearance, appearanceErrors) = parseAppearance(value["appearance"])
    val (kanban, kanbanErrors) = parseKanban(value["kanban"])
    val normalized = value["boards"]?.let { normalizeBoards(it) }
    val boards = normalized?.boards ?: emptyList()
    val boardErrors = normalized?.errors ?: emptyList()
    val list = value["taskCommands"]
        ?: return ParsedSettings(value, emptyList(), emptyList(), boards, boardErrors, homepage, appearance, appearanceErrors, kanban, kanbanErrors)
    if (list !is JsonArray) {
        return ParsedSettings(value, emptyList(), listOf("taskCommands must be a list"), boards, boardErrors, homepage, appearance, appearanceErrors, kanban, kanbanErrors)
    }
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
    return ParsedSettings(value, commands, errors, boards, boardErrors, homepage, appearance, appearanceErrors, kanban, kanbanErrors)
}

private val pretty = Json { prettyPrint = true; prettyPrintIndent = "  " }

/** `settings.json` as the app writes it. */
fun serializeSettings(raw: JsonObject): String = pretty.encodeToString(JsonObject.serializer(), raw) + "\n"

// Saving changes only what was edited (spec 0009; web/src/lib/appsettings.ts). A save is a three-way merge ([mergeSettings]):
// the file as it is, what this app understood of it (the same save made with what was read, unchanged), and the save with
// the edit. A key this app doesn't know, at any depth, and an entry it couldn't read are in the file but not in what it
// understood, so they stay; what it understood and the edit took out goes. Tested against the same cases as the web app and
// the terminal app (`fixtures/settings-kept.json`).

/** One text for a value, whatever order its keys are in. */
private fun canon(v: JsonElement?): String = when (v) {
    null -> "null"
    is JsonObject -> v.keys.sorted().joinToString(",", "{", "}") { "${JsonPrimitive(it)}:${canon(v[it])}" }
    is JsonArray -> v.joinToString(",", "[", "]") { canon(it) }
    else -> v.toString()
}

/** What an entry of a list is known by: its `id`, else its `name`, else nothing. */
private fun entryKey(v: JsonElement): String? {
    val o = v as? JsonObject ?: return null
    o["id"]?.text()?.let { return "id:$it" }
    o["name"]?.text()?.let { return "name:$it" }
    return null
}

/** Whether [old] is the entry [base] was read from: what both have is the same (reading may add a default). */
private fun readFrom(old: JsonElement, base: JsonElement): Boolean {
    if (old !is JsonObject || base !is JsonObject) return canon(old) == canon(base)
    return base.keys.all { k -> k !in old || canon(old[k]) == canon(base[k]) }
}

private fun mergeList(old: JsonArray, base: JsonArray, next: JsonArray): JsonArray {
    // Which entry of the file each entry understood was read from: the same id or name, else the next one like it.
    val used = HashSet<Int>()
    var after = 0
    val source = base.map { b ->
        val key = entryKey(b)
        val at = if (key != null) {
            old.indices.firstOrNull { it !in used && entryKey(old[it]) == key } ?: -1
        } else {
            old.indices.firstOrNull { it >= after && it !in used && entryKey(old[it]) == null && readFrom(old[it], b) } ?: -1
        }
        if (at >= 0) {
            used += at
            if (key == null) after = at + 1
        }
        at
    }
    // Which entry understood each entry saved is: the same id or name, else the same place among those with neither.
    val keyless = base.indices.filter { entryKey(base[it]) == null }
    var nth = 0
    val out = next.map { n ->
        val key = entryKey(n)
        val j = if (key != null) base.indexOfFirst { entryKey(it) == key } else keyless.getOrElse(nth++) { -1 }
        val from = if (j >= 0) source[j] else -1
        if (from >= 0) mergeSettings(old[from], base[j], n) else n
    }.toMutableList()
    // What this app couldn't read stays where it was.
    old.forEachIndexed { i, o -> if (i !in used) out.add(minOf(i, out.size), o) }
    return JsonArray(out)
}

/**
 * [next] (the file as this app would write it with the edit) laid over [old] (the file as it is), where [base] is the file
 * as this app would write it without the edit. What is in [old] and not in [base] is kept: a key this app doesn't know, an
 * entry it couldn't read. What is in [base] and not in [next] was taken out. Lists are matched entry by entry (by `id`,
 * else `name`, else place), so an entry keeps the keys this app doesn't know.
 */
fun mergeSettings(old: JsonElement, base: JsonElement?, next: JsonElement): JsonElement {
    if (old is JsonObject && next is JsonObject) {
        val understood = base as? JsonObject ?: JsonObject(emptyMap())
        val out = LinkedHashMap<String, JsonElement>()
        for ((k, v) in old) {
            val n = next[k]
            when {
                n != null -> out[k] = mergeSettings(v, understood[k], n)
                k !in understood -> out[k] = v
                // Taken out: only what was understood of it goes.
                v is JsonObject -> mergeSettings(v, understood[k], JsonObject(emptyMap())).let { if ((it as JsonObject).isNotEmpty()) out[k] = it }
                v is JsonArray -> mergeSettings(v, understood[k], JsonArray(emptyList())).let { if ((it as JsonArray).isNotEmpty()) out[k] = it }
            }
        }
        for ((k, v) in next) if (k !in old) out[k] = v
        return JsonObject(out)
    }
    if (old is JsonArray && next is JsonArray) return mergeList(old, base as? JsonArray ?: JsonArray(emptyList()), next)
    return next
}

private fun written(v: JsonElement): String = pretty.encodeToString(JsonElement.serializer(), v) + "\n"

/** [raw] read again, as this app reads it, for what it understood of the file. */
private fun reread(raw: JsonObject) = parseSettings(raw.toString())

// Each save is built twice: with what was read (`read`: true, which also writes the empty lists and sections that were in
// the file, since they were understood too) and with the edit.

private fun withCommands(raw: JsonObject, commands: List<TaskCommand>, read: Boolean = false) = buildJsonObject {
    for ((k, v) in raw) if (k != "taskCommands") put(k, v)
    if (commands.isNotEmpty() || (read && raw["taskCommands"] is JsonArray)) {
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

/** The text to save: `taskCommands` as given, and everything else in the file as it was. */
fun serializeSettings(raw: JsonObject, commands: List<TaskCommand>): String =
    written(mergeSettings(raw, withCommands(raw, reread(raw).commands, read = true), withCommands(raw, commands)))

private fun withBoards(raw: JsonObject, boards: List<Board>, homepage: Homepage?, setHomepage: Boolean, read: Boolean = false) = buildJsonObject {
    for ((k, v) in raw) if (k != "boards" && k != "boardsVersion" && !(setHomepage && k == "homepage")) put(k, v)
    if (setHomepage && homepage != null) put("homepage", homepageJson(homepage))
    if (boards.isNotEmpty() || (read && raw["boards"] is JsonArray)) {
        put("boardsVersion", JsonPrimitive(1))
        put("boards", boardsToJson(boards))
    }
}

/**
 * The text to save: `boards` as given (none leaves the key out), and everything else in the file as it was. With
 * [setHomepage], `homepage` is [homepage] (none takes it out); without, it is left as it was, so a board whose slug changed
 * can take the home page with it in the same write.
 */
fun serializeBoards(raw: JsonObject, boards: List<Board>, homepage: Homepage? = null, setHomepage: Boolean = false): String {
    val out = mergeSettings(raw, withBoards(raw, reread(raw).boards, null, false, read = true), withBoards(raw, boards, homepage, setHomepage))
    return written(withoutLegacyRanges(if (setHomepage) withHome(out, homepage) else out, boards.map { it.id }.toSet()))
}

/**
 * The boards in [file] saved here without their `ranges`: reading moves a board's date ranges into its `controls`, which it
 * writes, so the old key, kept by the merge, would name them twice.
 */
fun withoutLegacyRanges(file: JsonElement, ids: Set<String>): JsonElement {
    val o = file as? JsonObject ?: return file
    val list = o["boards"] as? JsonArray ?: return file
    val boards = list.map { b ->
        if (b is JsonObject && b["id"]?.text() in ids && "ranges" in b) JsonObject(b - "ranges") else b
    }
    return JsonObject(o + ("boards" to JsonArray(boards)))
}

private fun withHomepage(raw: JsonObject, homepage: Homepage?) = buildJsonObject {
    for ((k, v) in raw) if (k != "homepage") put(k, v)
    if (homepage != null) put("homepage", homepageJson(homepage))
}

/** [file] with `homepage` exactly as chosen: it is one of two shapes, so nothing of an unreadable one is kept in it. */
private fun withHome(file: JsonElement, homepage: Homepage?): JsonElement {
    val o = file as? JsonObject ?: return file
    val out = LinkedHashMap(o)
    if (homepage != null) out["homepage"] = homepageJson(homepage) else out.remove("homepage")
    return JsonObject(out)
}

/** The text to save: `homepage` as given (none takes the key out), and everything else in the file as it was. */
fun serializeHomepage(raw: JsonObject, homepage: Homepage?): String =
    written(withHome(mergeSettings(raw, withHomepage(raw, reread(raw).homepage), withHomepage(raw, homepage)), homepage))

private fun homepageJson(h: Homepage): JsonObject = buildJsonObject {
    h.report?.let { put("report", JsonPrimitive(it)) }
    h.board?.let { put("board", JsonPrimitive(it)) }
}

private fun withAppearance(raw: JsonObject, appearance: Appearance, read: Boolean = false) = buildJsonObject {
    for ((k, v) in raw) if (k != "appearance") put(k, v)
    val was = raw["appearance"] as? JsonObject
    val there = read && was != null
    val bars = appearance.progress.isNotEmpty() || (there && was?.get("progress") is JsonObject)
    val rules = appearance.transformations.isNotEmpty() || (there && was?.get("transformations") is JsonArray)
    val fallback = appearance.idFallback || (there && (was?.get("idFallback") as? JsonPrimitive)?.content?.toBooleanStrictOrNull() != null)
    if (bars || rules || fallback || there) {
        put(
            "appearance",
            buildJsonObject {
                if (bars) {
                    put(
                        "progress",
                        buildJsonObject {
                            for ((name, p) in appearance.progress) {
                                put(name, buildJsonObject { put("min", number(p.min)); put("max", number(p.max)) })
                            }
                        },
                    )
                }
                if (rules) put("transformations", transformationsJson(appearance.transformations))
                if (fallback) put("idFallback", JsonPrimitive(appearance.idFallback))
            },
        )
    }
}

/** The text to save: `appearance` as given (nothing chosen takes the key out), and everything else in the file as it was. */
fun serializeAppearance(raw: JsonObject, appearance: Appearance): String =
    written(mergeSettings(raw, withAppearance(raw, reread(raw).appearance, read = true), withAppearance(raw, appearance)))

private fun withKanban(raw: JsonObject, kanban: KanbanSettings, read: Boolean = false) = buildJsonObject {
    for ((k, v) in raw) if (k != "kanban") put(k, v)
    if (kanban.enabled || kanban.setup != null || kanban.kanbans.isNotEmpty() || (read && raw["kanban"] is JsonObject)) put("kanban", kanbanJson(kanban))
}

/** The text to save: `kanban` as given, and everything else in the file as it was. */
fun serializeKanban(raw: JsonObject, kanban: KanbanSettings): String =
    written(mergeSettings(raw, withKanban(raw, reread(raw).kanban, read = true), withKanban(raw, kanban)))

/** What is wrong with a `settings.json`, as a linter says it: an error stops it being saved, a warning doesn't. */
data class Lint(val errors: List<String>, val warnings: List<String>)

private val OBJECT_KEYS = listOf("homepage", "appearance", "kanban", "keys", "web", "mobile", "tui")
private val LIST_KEYS = listOf("taskCommands", "boards")
private const val NOT_SETTINGS = "The settings must be a JSON object: { \"taskCommands\": [ … ] }"
private const val APPEARANCE_NOT_OBJECT = "appearance must be an object: { \"progress\": { … } }"
private const val PROGRESS_NOT_OBJECT = "appearance.progress must be an object: { \"<uda>\": { \"min\": 0, \"max\": 100 } }"
private const val TRANSFORMATIONS_NOT_LIST = "appearance.transformations must be a list: [ { \"match\": …, \"replace\": …, \"fields\": [ … ] } ]"
private val SHAPES = Regex("^(boards must be a list|board \\d+: not an object|taskCommands must be a list|kanban must be an object.*|kanban\\.(kanbans|columns) must be a list)$")

/**
 * Lint [text] as `settings.json` (web/src/lib/appsettings.ts `lintSettings`). Errors: not JSON, not an object, over 64 KB,
 * or a known key in the wrong shape. Warnings: everything this app can't use, which is kept in the file, and what [more]
 * says (the keys' problems).
 */
fun lintSettings(text: String, more: (JsonObject) -> List<String> = { emptyList() }): Lint {
    val errors = mutableListOf<String>()
    val size = text.encodeToByteArray().size
    if (size > MAX_SETTINGS_BYTES) errors += "The file is ${(size + 1023) / 1024} KB; settings.json holds at most ${MAX_SETTINGS_BYTES / 1024} KB."
    if (text.isNotBlank()) {
        val value = try {
            Json.parseToJsonElement(text)
        } catch (e: Exception) {
            return Lint(errors + "Not valid JSON: ${e.message}", emptyList())
        }
        if (value !is JsonObject) return Lint(errors + NOT_SETTINGS, emptyList())
    }
    val read = parseSettings(text)
    val raw = read.raw
    for (k in OBJECT_KEYS) if (k in raw && raw[k] !is JsonObject) errors += if (k == "appearance") APPEARANCE_NOT_OBJECT else "$k must be an object"
    for (k in LIST_KEYS) if (k in raw && raw[k] !is JsonArray) errors += "$k must be a list"
    (raw["appearance"] as? JsonObject)?.let { a ->
        if ("progress" in a && a["progress"] !is JsonObject) errors += PROGRESS_NOT_OBJECT
        if ("transformations" in a && a["transformations"] !is JsonArray) errors += TRANSFORMATIONS_NOT_LIST
    }
    (raw["boards"] as? JsonArray)?.forEachIndexed { i, b ->
        if (b !is JsonObject) errors += "boards[$i] must be an object"
        else if (b["sections"] !is JsonArray) errors += "boards[$i]${b["id"]?.text()?.let { " ($it)" } ?: ""}: needs a sections list"
    }
    (raw["kanban"] as? JsonObject)?.let { k ->
        if ("kanbans" in k && k["kanbans"] !is JsonArray) errors += "kanban.kanbans must be a list"
        if ("columns" in k && k["columns"] !is JsonArray) errors += "kanban.columns must be a list"
    }
    val said = errors.toSet()
    val warnings = (read.errors + read.appearanceErrors + read.boardErrors + read.kanbanErrors + more(raw)).filter { it !in said && !SHAPES.matches(it) }
    return Lint(errors, warnings)
}

/** The rules as `settings.json` and the engine take them. */
fun transformationsJson(rules: List<Transformation>): JsonArray = buildJsonArray {
    for (r in rules) {
        add(
            buildJsonObject {
                put("match", r.match)
                put("replace", r.replace)
                put("fields", JsonArray(r.fields.map(::JsonPrimitive)))
            },
        )
    }
}

/** A number the way the web writes it: `100`, not `100.0`. */
private fun number(d: Double): JsonPrimitive = if (d == kotlin.math.floor(d) && kotlin.math.abs(d) < 1e15) JsonPrimitive(d.toLong()) else JsonPrimitive(d)
