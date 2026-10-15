package me.utsob.meghnaad.app

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.double
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
// Urgency settings as the Urgency tab shows them. The key spelling and what each key applies to follow Taskwarrior's `urgency.*`
// settings, and the rules for a name are the ones the engine applies when it saves (`tc_core::urgency_settings`, through
// `callHelper`), so a screen cannot offer what would be refused.

enum class UrgencyKind { Term, Project, Tag, Keyword, Uda }

data class Described(val key: String, val kind: UrgencyKind, val name: String, val label: String, val hint: String)

class UrgencyTerm(val term: String, val label: String, val hint: String)

private fun kindOf(s: String) = UrgencyKind.valueOf(s.replaceFirstChar { it.uppercase() })

/** The built-in terms, in the order Taskwarrior lists them. */
val URGENCY_TERMS: List<UrgencyTerm> by lazy {
    callHelper("urgency_terms").jsonArray.map { t ->
        val o = t.jsonObject
        UrgencyTerm(o.getValue("term").jsonPrimitive.content, o.getValue("label").jsonPrimitive.content, o.getValue("hint").jsonPrimitive.content)
    }
}

const val AGE_MAX = "urgency.age.max"
const val INHERIT_KEY = "urgency.inherit"

fun describeUrgency(key: String): Described {
    val o = callHelper("describe_urgency_key", JsonPrimitive(key)).jsonObject
    return Described(
        o.getValue("key").jsonPrimitive.content,
        kindOf(o.getValue("kind").jsonPrimitive.content),
        o.getValue("name").jsonPrimitive.content,
        o.getValue("label").jsonPrimitive.content,
        o.getValue("hint").jsonPrimitive.content,
    )
}

class Addable(val kind: UrgencyKind, val label: String, val placeholder: String)

/** What can be added beyond the built-in terms. */
val ADDABLE: List<Addable> by lazy {
    callHelper("urgency_addable").jsonArray.map { t ->
        val o = t.jsonObject
        Addable(kindOf(o.getValue("kind").jsonPrimitive.content), o.getValue("label").jsonPrimitive.content, o.getValue("placeholder").jsonPrimitive.content)
    }
}

/** The rule the engine applies to names: no spaces, `=`, `#`, and not containing ".coefficient". */
fun validUrgencyName(name: String): Boolean = callHelper("valid_urgency_name", JsonPrimitive(name)).jsonPrimitive.boolean

fun buildUrgencyKey(kind: UrgencyKind, name: String): String =
    callHelper("build_urgency_key", JsonPrimitive(kind.name.lowercase()), JsonPrimitive(name)).jsonPrimitive.content

/** UDA names are plain identifiers; the value after the first dot is free text. */
fun validUda(name: String): Boolean = callHelper("valid_urgency_uda", JsonPrimitive(name)).jsonPrimitive.boolean

/** The order rows appear in: built-in terms first, then custom ones grouped by kind. */
fun sortUrgencyKeys(keys: Collection<String>): List<String> =
    callHelper("sort_urgency_keys", JsonArray(keys.map { JsonPrimitive(it) })).jsonArray.map { it.jsonPrimitive.content }

/** The value in force: the saved override, else Taskwarrior's built-in one. */
fun effectiveUrgency(key: String, overrides: Map<String, Double>, defaults: Map<String, Double>): Double? = overrides[key] ?: defaults[key]

private fun numbers(m: Map<String, Double>): JsonElement = JsonObject(m.mapValues { JsonPrimitive(it.value) })

/** Record `value` for `key`. A value equal to the built-in one is not an override, so it is dropped and the setting follows the default again. */
fun withUrgencyValue(overrides: Map<String, Double>, defaults: Map<String, Double>, key: String, value: Double): Map<String, Double> =
    callHelper("urgency_with_value", numbers(overrides), numbers(defaults), JsonPrimitive(key), JsonPrimitive(value))
        .jsonObject.mapValues { it.value.jsonPrimitive.double }
