package me.utsob.meghnaad.app

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * The keys of a hardware keyboard, as `tc_core::keymap` resolves them for the phone (keymap.ts): the defaults (the keymap asset,
 * a resource here), then `keys` and `mobile.keys` of `settings.json`. An action is a dotted name (`app.prompt`); a key is written as Zed and VS Code write it
 * (`ctrl+k`). The engine reads the keys and turns a key the keyboard gives into the same form, so the phone, the web app and
 * the terminal app read `settings.json` alike.
 */
class Keymap(
    /** Each action's keys, as the engine writes them (`ctrl+k`, `/`, `enter`). */
    val keys: Map<String, List<String>> = emptyMap(),
    /** Each context's keys and the action each runs. */
    val contexts: Map<String, Map<String, String>> = emptyMap(),
    /** What is wrong with `keys` or `mobile.keys`, one line each. */
    val problems: List<String> = emptyList(),
) {
    /** The action [chord] runs in [context] (the phone has one, `app`), if any. */
    fun action(chord: String?, context: String = "app"): String? = chord?.let { contexts[context]?.get(it) }
}

/** Every action the phone has, with what it does, for the list in Settings. */
val MOBILE_ACTIONS = listOf("app.prompt" to "Put the cursor in the prompt (from anywhere that isn’t a text field)")

/** The default keys of every front end: the asset the web serves (`web/public/keymaps`), carried here as a resource. */
val KEYMAP_DEFAULTS: JsonObject by lazy {
    runCatching { engineJson.parseToJsonElement(readResource("/keymaps/default.json") ?: "{}").jsonObject }.getOrElse { JsonObject(emptyMap()) }
}

/** The phone's keys from the defaults and the whole of `settings.json`; none when the engine cannot be asked. */
fun resolveKeymap(settings: JsonObject, defaults: JsonObject = KEYMAP_DEFAULTS): Keymap = runCatching {
    val o = callHelper("keymap", JsonPrimitive("mobile"), settings, defaults).jsonObject
    Keymap(
        keys = o["keys"]?.jsonObject?.mapValues { (_, v) -> v.jsonArray.map { it.jsonPrimitive.content } } ?: emptyMap(),
        contexts = o["contexts"]?.jsonObject?.mapValues { (_, v) -> v.jsonObject.mapValues { it.value.jsonPrimitive.content } } ?: emptyMap(),
        problems = o["problems"]?.jsonArray?.map { it.jsonPrimitive.content } ?: emptyList(),
    )
}.getOrElse { Keymap() }

/** A key the keyboard gave (a character, or a name: `enter`, `down`) with the modifiers held, as the keymap writes it. */
fun keyChord(key: String, mods: List<String>): String? = runCatching {
    (callHelper("key_chord", JsonPrimitive(key), JsonArray(mods.map(::JsonPrimitive))) as? JsonPrimitive)?.contentOrNull
}.getOrNull()

/** A key as a hint shows it: `↵`, `Ctrl+K`, `/`. */
fun keyLabel(chord: String): String =
    runCatching { (callHelper("key_label", JsonPrimitive(chord)) as? JsonPrimitive)?.contentOrNull }.getOrNull() ?: chord
