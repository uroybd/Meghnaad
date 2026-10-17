package me.utsob.meghnaad.app

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.encodeToJsonElement

// What a line typed into the quick add would make, and the same broken into the fields of the full form (addPreview.ts). The
// reading of the line is the engine's (`tc_core::preview`, through `callHelper`): the same `parse_mods` and `plan` that `add`
// runs, so the card shows what will be made and the form starts from it.

/** `tc_core::preview::Preview`: the task a line would add. Dates are seconds since 1970, by attribute name. */
@Serializable
data class AddPreview(
    val description: String = "",
    val project: String? = null,
    val priority: String? = null,
    val tags: List<String> = emptyList(),
    val dates: Map<String, Long> = emptyMap(),
    val recur: String? = null,
    /** Task ids and uuids as typed. */
    val depends: List<String> = emptyList(),
    /** By UDA name; one of type date is a timestamp. */
    val udas: Map<String, String> = emptyMap(),
    /** What is wrong with the line, one message for each part left out. */
    val errors: List<String> = emptyList(),
)

// The engine reads the whole of a UDA, so the defaults a screen leaves out of its copy are written.
private val configJson = Json { encodeDefaults = true }

/** The task [line] would add (what comes after `task add`), at [now] in the zone [tz] seconds east of UTC. */
fun previewAdd(line: String, config: ConfigBody?, now: Long, tz: Int): AddPreview {
    val body = config ?: ConfigBody()
    // Only what `add` reads: the reports are the engine's own and stay out.
    val cfg: JsonElement = JsonObject(
        mapOf(
            "udas" to configJson.encodeToJsonElement(body.udas),
            "contexts" to configJson.encodeToJsonElement(body.contexts),
            "active_context" to configJson.encodeToJsonElement(body.activeContext),
            "settings" to configJson.encodeToJsonElement(body.settings),
        ),
    )
    return engineJson.decodeFromJsonElement(AddPreview.serializer(), callHelper("add_preview", JsonPrimitive(line), cfg, JsonPrimitive(now), JsonPrimitive(tz)))
}

/** The arguments `task add` is run with for a line: its words, the way the engine reads them. */
fun addArgs(line: String): List<String> = listOf("add") + splitWords(line)

/** The fields of the full form, as it holds them (dates as `YYYY-MM-DD[THH:MM]`, tags as one text). */
data class FormSeed(
    val description: String = "",
    val project: String = "",
    val priority: String = "",
    val due: String = "",
    val wait: String = "",
    val scheduled: String = "",
    val until: String = "",
    val tags: String = "",
    val recur: String = "",
    /** Task ids, space separated. */
    val depends: String = "",
    val udas: Map<String, String> = emptyMap(),
    /** Attributes the form has no field for (`start:…`), sent as they are. */
    val carried: List<String> = emptyList(),
)

private val FORM_DATES = setOf("due", "wait", "scheduled", "until")

/** The form's fields for a preview; with none, for a bare description (or nothing). [offsetAt] is the zone at a moment. */
fun seedOf(p: AddPreview?, description: String = "", defs: Map<String, UdaDef> = emptyMap(), offsetAt: (Long) -> Int): FormSeed {
    fun wire(ts: Long?): String = if (ts == null) "" else fromParts(toParts(ts, offsetAt(ts)))
    return FormSeed(
        description = p?.description ?: description,
        project = p?.project ?: "",
        priority = p?.priority ?: "",
        due = wire(p?.dates?.get("due")),
        wait = wire(p?.dates?.get("wait")),
        scheduled = wire(p?.dates?.get("scheduled")),
        until = wire(p?.dates?.get("until")),
        tags = (p?.tags ?: emptyList()).joinToString(" "),
        recur = p?.recur ?: "",
        depends = (p?.depends ?: emptyList()).joinToString(" "),
        udas = (p?.udas ?: emptyMap()).mapValues { (name, value) -> if (defs[name]?.type == "date") wire(value.toLongOrNull()) else value },
        carried = (p?.dates ?: emptyMap()).filterKeys { it !in FORM_DATES }.map { (name, ts) -> "$name:${wire(ts)}" },
    )
}
