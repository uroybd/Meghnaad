package me.utsob.meghnaad.app

import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.int
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.Serializable
// Sorting by a column. A sort is a Taskwarrior sort spec (`due+,priority-,project+/`), applied through the command line as
// `rc.report.<name>.sort:<spec>`, so the console and the list always agree. What choosing a column or a grouping does to the keys
// is the engine's (`tc_core::sortspec`, through `callHelper`), which every front end shares; the cases are tested there.

@Serializable
data class SortKey(val column: String, val desc: Boolean = false, /** A trailing `/`: a break when the value changes. */ val brk: Boolean = false)

/** Columns a header (or a sort picker) can sort by; UDAs are added by the caller. */
val SORTABLE = setOf(
    "id", "uuid", "status", "description", "project", "priority", "tags", "depends", "entry", "start", "end", "due", "wait",
    "scheduled", "until", "modified", "urgency", "recur", "parent",
)

/** `due.relative` sorts by `due`. */
fun baseColumn(spec: String): String = spec.substringBefore('.')

private val KEYS = ListSerializer(SortKey.serializer())

private fun keysJson(keys: List<SortKey>): JsonElement = engineJson.encodeToJsonElement(KEYS, keys)

private fun keysOf(e: JsonElement): List<SortKey> = engineJson.decodeFromJsonElement(KEYS, e)

private fun text(s: String?): JsonElement = if (s == null) JsonNull else JsonPrimitive(s)

private fun JsonElement.textOrNull(): String? = if (this is JsonNull) null else jsonPrimitive.content

/** The keys of a spec: none for `none`, `random` or nothing, and a part that is no key is left out. */
fun parseSort(spec: String?): List<SortKey> = keysOf(callHelper("parse_sort", text(spec)))

fun serializeSort(keys: List<SortKey>): String = callHelper("serialize_sort", keysJson(keys)).jsonPrimitive.content

class ClickResult(val keys: List<SortKey>, /** The third click on a column: go back to the report's own sort. */ val reset: Boolean)

/**
 * What choosing a column does:
 *  - a new column becomes the primary key (ascending); earlier keys stay as tie-breakers;
 *  - the primary column again: ascending, then descending, then back to the report's default;
 *  - [shift]: add the column as a further tie-breaker, or flip its direction if present.
 */
fun clickSort(current: List<SortKey>, column: String, shift: Boolean = false): ClickResult {
    val o = callHelper("click_sort", keysJson(current), JsonPrimitive(column), JsonPrimitive(shift)).jsonObject
    return ClickResult(keysOf(o.getValue("keys")), o.getValue("reset").jsonPrimitive.boolean)
}

/** The sort override for [report] in a filter string, if any. */
fun sortOverride(filter: String, report: String): String? =
    callHelper("sort_override", JsonPrimitive(filter), JsonPrimitive(report)).textOrNull()

/** Set (or with null, remove) the sort override in a filter string; a spec equal to the report's default needs none. */
fun withSortOverride(filter: String, report: String, spec: String?, defaultSpec: String?): String =
    callHelper("with_sort_override", JsonPrimitive(filter), JsonPrimitive(report), text(spec), text(defaultSpec)).jsonPrimitive.content

class SortState(val desc: Boolean, val rank: Int?, val group: Boolean)

/** For a header: its place in the active sort (and whether it groups the list), or null. */
fun sortState(keys: List<SortKey>, column: String): SortState? {
    val e = callHelper("sort_state", keysJson(keys), JsonPrimitive(column))
    if (e is JsonNull) return null
    val o = e.jsonObject
    return SortState(o.getValue("desc").jsonPrimitive.boolean, o.getValue("rank").let { if (it is JsonNull) null else it.jsonPrimitive.int }, o.getValue("group").jsonPrimitive.boolean)
}

/**
 * Group by [column], or stop grouping by it: its sort key gains or loses the trailing `/`. A column the sort does not use
 * yet becomes the first key, ascending, since a group is only whole when the grouping column is what the list is sorted by
 * first.
 */
fun toggleGroup(current: List<SortKey>, column: String, defaults: List<SortKey> = emptyList()): List<SortKey> =
    keysOf(callHelper("toggle_group", keysJson(current), JsonPrimitive(column), keysJson(defaults)))

/** The same sort with no grouping at all. */
fun clearGroups(current: List<SortKey>): List<SortKey> = keysOf(callHelper("clear_groups", keysJson(current)))

/** The column the list is grouped by first, if any. */
fun groupColumn(keys: List<SortKey>): String? = callHelper("group_column", keysJson(keys)).textOrNull()
