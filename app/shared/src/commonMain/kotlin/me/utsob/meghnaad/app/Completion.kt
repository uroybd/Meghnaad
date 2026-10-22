package me.utsob.meghnaad.app

import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonPrimitive
// Tab completion for the console: commands and reports, attributes and their modifiers, tags, task ids, and values (projects,
// priorities, UDA values, date words). Everything is built from what the engine has told us, and each option can carry a
// short hint. The completing is the engine's (`tc_core::complete`, through `callHelper`), which every front end shares; the cases
// are tested there.

@Serializable
class TaskRef(val id: Int?, val uuid: String, val description: String)

@Serializable
class ReportName(val name: String, val description: String? = null)

@Serializable
class Vocab(
    /** Report names, with their descriptions as hints. */
    val reports: List<ReportName> = emptyList(),
    val projects: List<String> = emptyList(),
    val tags: List<String> = emptyList(),
    val udas: List<UdaDef> = emptyList(),
    val tasks: List<TaskRef> = emptyList(),
    val contexts: List<String> = emptyList(),
)

@Serializable
data class Option(val value: String, val hint: String? = null)

@Serializable
class Completion(
    /** Replace `line[start, end)` with [common] (or with one option's value). */
    val start: Int,
    val end: Int,
    val options: List<Option>,
    /** The longest common prefix of the option values. */
    val common: String,
)

/** The commands the console knows, as the engine lists them. */
val COMMAND_NAMES: List<String> by lazy { stringList(callHelper("command_names")) }

/** The virtual tags Taskwarrior computes (`+OVERDUE`). */
val VIRTUAL_TAGS: List<String> by lazy { stringList(callHelper("virtual_tags")) }

private fun stringList(e: JsonElement): List<String> = e.jsonArray.map { it.jsonPrimitive.content }

/** The vocabulary the engine last had; it is handed over again only when it is another one. */
private var sentVocab: Vocab? = null

/** What the word before the caret could become. Positions are UTF-16 units, as a text field counts them. */
fun complete(line: String, caret: Int, v: Vocab): Completion {
    if (v !== sentVocab) {
        callHelper("set_vocab", engineJson.encodeToJsonElement(Vocab.serializer(), v))
        sentVocab = v
    }
    return engineJson.decodeFromJsonElement(Completion.serializer(), callHelper("complete", JsonPrimitive(line), JsonPrimitive(caret)))
}

/** The date words (`sow`, `eod`) that begin with what has been typed of a date; none for a date written out or for nothing typed. */
fun dateWords(typed: String): List<Option> =
    engineJson.decodeFromJsonElement(ListSerializer(Option.serializer()), callHelper("date_words", JsonPrimitive(typed)))

@Serializable
class Inserted(val line: String, val caret: Int)

/** Insert [value] in place of the completed word; add a space unless it ends mid-token. */
fun insert(line: String, c: Completion, value: String, space: Boolean = true): Inserted =
    engineJson.decodeFromJsonElement(
        Inserted.serializer(),
        callHelper("insert_completion", JsonPrimitive(line), engineJson.encodeToJsonElement(Completion.serializer(), c), JsonPrimitive(value), JsonPrimitive(space)),
    )

@Serializable
class Applied(val line: String, val caret: Int, val progressed: Boolean)

/**
 * Tab behaviour: complete the shared prefix; when that makes no progress, the caller opens a menu of the options. A single
 * option is completed in full (plus a space).
 */
fun apply(line: String, c: Completion): Applied =
    engineJson.decodeFromJsonElement(
        Applied.serializer(),
        callHelper("apply_completion", JsonPrimitive(line), engineJson.encodeToJsonElement(Completion.serializer(), c)),
    )
