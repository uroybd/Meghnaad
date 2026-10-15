package me.utsob.meghnaad.app

import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.Serializable
// Recurrence periods as Taskwarrior writes them (`recur:weekly`, `recur:3d`, ISO `P2W`). The engine validates; this only
// offers sensible choices and describes what is stored (web/src/lib/recurrence.ts).

@Serializable
class Preset(val value: String, val label: String)

/** The periods a screen offers, with their labels: the engine's list (`tc_core::complete::RECUR_PRESETS`), which `recur:` completes too. */
val PRESETS: List<Preset> by lazy { engineJson.decodeFromJsonElement(ListSerializer(Preset.serializer()), callHelper("recur_presets")) }

/** "every 3 days" for `3d`; unknown spellings come back as typed. The words are the engine's (`tc_core::helpers::describe_recur`). */
fun describeRecur(period: String?): String =
    callHelper("describe_recur", kotlinx.serialization.json.JsonPrimitive(period ?: "")).let { (it as kotlinx.serialization.json.JsonPrimitive).content }

/** The preset a stored period corresponds to exactly, or empty when it needs the custom field. */
fun presetFor(period: String?): String = (period ?: "").trim().let { p -> if (PRESETS.any { it.value == p }) p else "" }
