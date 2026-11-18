package me.utsob.meghnaad.app

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.longOrNull

// Spec 0016: a task shared out of the app, as text another app takes (a chat, mail, a note), in one of three forms. The
// short text is for a person and reads back into the quick add; the JSON is Taskwarrior's own, which `task import` and the
// app's own share target read back; the info is `task info`'s text, which the engine writes (`info_text`).

/** The forms a task is shared in, in the order the menu offers them. */
enum class ShareForm(val label: String) {
    Short("Short text"),
    Json("Task JSON"),
    Info("Task info"),
}

/**
 * The description, then the project, priority, due date and tags as the quick add reads them, so the text shared back
 * into the app fills the same fields: `project:home priority:H due:2026-11-20 +errand`. The due date is an ISO date in
 * the phone's zone (with its time when it has one), which the engine reads whatever the `dateformat`. A task with none
 * of them is its description alone.
 */
fun shortShare(row: Row, offsetAt: (Long) -> Int): String {
    val attributes = buildList {
        row.project?.takeIf { it.isNotEmpty() }?.let { add("project:" + if (it.any(Char::isWhitespace)) shellQuote(it) else it) }
        row.priority?.takeIf { it.isNotEmpty() }?.let { add("priority:$it") }
        row.due?.let { add("due:" + fromParts(toParts(it, offsetAt(it)))) }
        row.tags.forEach { add("+$it") }
    }
    return if (attributes.isEmpty()) row.description else row.description + "\n" + attributes.joinToString(" ")
}

private val pretty = Json { prettyPrint = true }

/** The object `task export` writes for the one task in [text] (an array of one, or a line of JSON), set out on lines. */
fun taskJsonOf(text: String): String? {
    val trimmed = text.trim()
    val element = runCatching { engineJson.parseToJsonElement(trimmed) }.getOrNull()
        ?: trimmed.lineSequence().firstOrNull { it.isNotBlank() }?.let { runCatching { engineJson.parseToJsonElement(it) }.getOrNull() }
    val task = when (element) {
        is JsonArray -> element.firstOrNull()
        is JsonObject -> element
        else -> null
    } as? JsonObject ?: return null
    return pretty.encodeToString(JsonElement.serializer(), task)
}

/** The config `info_text` reads: what the add preview takes, and the urgency coefficients for the breakdown. */
internal fun infoConfig(config: ConfigBody?): JsonElement =
    JsonObject(addConfig(config).jsonObject + ("urgency" to JsonObject((config?.urgency ?: emptyMap()).mapValues { JsonPrimitive(it.value) })))

/** The text of [form] for [row], as it is sent. */
suspend fun Store.shareText(form: ShareForm, row: Row): String = when (form) {
    ShareForm.Short -> shortShare(row, ::offsetAt)
    ShareForm.Json -> when (val r = query(listOf(row.uuid, "export"))) {
        is CliResult.File -> taskJsonOf(r.value.text)
        else -> null
    } ?: throw EngineError("The task could not be exported.")
    ShareForm.Info -> infoText(row.uuid)
}

/** The open tasks a filter picks, as `{id, description}`, by ID (what `info` lists them by). */
private suspend fun Store.linked(filter: List<String>): JsonArray {
    val rows = (query(filter + "_rows") as? CliResult.Json)?.value as? JsonArray ?: return JsonArray(emptyList())
    val tasks = rows.mapNotNull { it as? JsonObject }
        .sortedBy { it["id"]?.jsonPrimitive?.longOrNull ?: Long.MAX_VALUE }
        .map { JsonObject(mapOf("id" to (it["id"] ?: JsonNull), "description" to (it["description"] ?: JsonPrimitive("")))) }
    return JsonArray(tasks)
}

/** `task <uuid> info`, as the real `task` prints it: the engine's row, its history and the open tasks around it. */
private suspend fun Store.infoText(uuid: String): String {
    val row = ((query(listOf(uuid, "_rows")) as? CliResult.Json)?.value as? JsonArray)?.firstOrNull() as? JsonObject
        ?: throw EngineError("This task is no longer here.")
    val history = (query(listOf("_history", uuid)) as? CliResult.Json)?.value ?: JsonNull
    val open = listOf("(", "status:pending", "or", "status:waiting", ")")
    val depends = row["depends"]?.jsonArray?.map { it.jsonPrimitive.content }.orEmpty()
    val blockedBy = if (depends.isEmpty()) JsonArray(emptyList()) else linked(open + depends)
    val blocking = if (row["blocking"]?.jsonPrimitive?.booleanOrNull == true) linked(open + "depends.has:$uuid") else JsonArray(emptyList())
    val now = clockNow()
    val text = callHelper(
        "info_text", row, history, blockedBy, blocking, infoConfig(config?.config), JsonPrimitive(now), JsonPrimitive(offsetAt(now)),
    )
    return text.jsonPrimitive.content
}
