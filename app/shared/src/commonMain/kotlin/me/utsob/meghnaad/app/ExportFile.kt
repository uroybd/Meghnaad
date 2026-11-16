package me.utsob.meghnaad.app

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonArray

// The Export button over a table (web/src/lib/exportScope.ts and exportFile.ts): which command line exports what the table
// shows, and the file it makes, shaped as the person chooses. All of it is the engine's (`tc_core::table_export`, through
// `callHelper`), shared with the web app and the terminal app, so the three make the same file for the same choice.

/** The file's format. */
enum class ExportFormat(val key: String, val label: String) {
    Json("json", "JSON"),
    Csv("csv", "CSV"),
    Tsv("tsv", "TSV"),
}

/** Every field the tasks have, the table's columns, or the ones picked. */
enum class ExportFields(val key: String) {
    All("all"),
    Table("table"),
    Custom("custom"),
}

/** How the person wants the file: [array] is JSON's list (`json.array=on`) or one task to a line, null as the taskrc says. */
data class ExportChoice(
    val format: ExportFormat = ExportFormat.Json,
    val array: Boolean? = null,
    val fields: ExportFields = ExportFields.All,
    val picked: List<String> = emptyList(),
)

private fun strings(v: JsonElement): List<String> = (v as? JsonArray)?.map { it.jsonPrimitive.content } ?: emptyList()
private fun words(list: List<String>) = JsonArray(list.map(::JsonPrimitive))
private fun settingsOf(raw: JsonElement?) = raw ?: JsonObject(emptyMap())

/**
 * The command line that exports what the table drawn by [input] shows, in the page's context ([pageChoice], a name or `none`;
 * null: the active one), from the settings [raw] as the engine sent them; null when it isn't a report's.
 */
fun exportArgs(input: CliInput, pageChoice: String?, raw: JsonElement?): List<String>? {
    if (raw == null) return null
    val w = input.args ?: rustHelpers().splitWords(input.line ?: "")
    val line = callHelper("export_line", words(w), raw, pageChoice?.let(::JsonPrimitive) ?: JsonNull)
    return if (line is JsonArray) strings(line) else null
}

/** Every field the tasks in what `export` wrote have, in the order they first appear. */
fun exportFields(text: String): List<String> = strings(callHelper("export_fields", JsonPrimitive(text)))

/** `json.array` as the taskrc has it, as Taskwarrior reads it (on unless it says otherwise). */
fun jsonArrayOf(raw: JsonElement?): Boolean = callHelper("json_array", settingsOf(raw)).jsonPrimitive.booleanOrNull ?: true

/** The fields a table shows, once each, in its order (`due.relative` is `due`). */
fun tableFields(columns: List<String>): List<String> = strings(callHelper("table_fields", words(columns)))

/** The file for [choice] from what `export` wrote ([name], [text]): the engine's name with the format's extension, the text, its type. */
fun renderExport(name: String, text: String, choice: ExportChoice, columns: List<String>, raw: JsonElement?): FileResult {
    val c = buildJsonObject {
        put("format", choice.format.key)
        choice.array?.let { put("array", it) } ?: put("array", JsonNull)
        put("fields", choice.fields.key)
        putJsonArray("picked") { choice.picked.forEach { add(JsonPrimitive(it)) } }
    }
    val f = callHelper("export_file", JsonPrimitive(name), JsonPrimitive(text), c, words(columns), settingsOf(raw)).jsonObject
    return FileResult(
        name = f.getValue("name").jsonPrimitive.content,
        mime = f.getValue("mime").jsonPrimitive.content,
        text = f.getValue("text").jsonPrimitive.content,
    )
}

const val EXPORT_CHOICE_KEY = "export-choice"

/** The last choice as kept ([keptExport]); the JSON shape always starts as the taskrc says. Anything damaged is the default. */
fun rememberedExport(text: String?): ExportChoice = try {
    val v = engineJson.parseToJsonElement(text ?: "{}").jsonObject
    ExportChoice(
        format = ExportFormat.entries.firstOrNull { it.key == v["format"]?.jsonPrimitive?.content } ?: ExportFormat.Json,
        fields = ExportFields.entries.firstOrNull { it.key == v["fields"]?.jsonPrimitive?.content } ?: ExportFields.All,
        picked = (v["picked"] as? JsonArray)?.mapNotNull { (it as? JsonPrimitive)?.takeIf { p -> p.isString }?.content } ?: emptyList(),
    )
} catch (e: Exception) {
    ExportChoice()
}

/** The choice to keep: format, fields and the fields picked (the web app keeps the same in the browser). */
fun keptExport(choice: ExportChoice): String = buildJsonObject {
    put("format", choice.format.key)
    put("fields", choice.fields.key)
    putJsonArray("picked") { choice.picked.forEach { add(JsonPrimitive(it)) } }
}.toString()

/** How many fields the file will have. */
fun exportFieldCount(choice: ExportChoice, available: List<String>, columns: List<String>): Int = when (choice.fields) {
    ExportFields.All -> available.size
    ExportFields.Table -> tableFields(columns).size
    ExportFields.Custom -> available.count { it in choice.picked }
}
