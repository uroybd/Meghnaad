package me.utsob.meghnaad.app

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
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

/** The sections of `settings.json` that hold keys: `general` is `keys`, for every front end; the rest their own. */
val KEY_SECTIONS = listOf("general", "web", "mobile", "tui")

/** One action of a section, for the Keybindings tab (`tc_core::keymap::Binding`). */
data class Binding(
    val action: String,
    /** What it does, from the asset's `about`; empty for a name that is no action. */
    val about: String,
    /** Its keys by default. */
    val defaults: List<String>,
    /** The keys set in this section (`[]` for none); null where nothing is set. */
    val keys: List<String>?,
)

class KeyBindings(val sections: Map<String, List<Binding>> = emptyMap(), val problems: List<String> = emptyList())

private fun strings(v: JsonElement?): List<String> = (v as? JsonArray)?.map { it.jsonPrimitive.content } ?: emptyList()

/** Every section's actions, the user's first (keymap.ts `keyBindings`); none when the engine cannot be asked. */
fun keymapBindings(settings: JsonObject, defaults: JsonObject = KEYMAP_DEFAULTS): KeyBindings = runCatching {
    val o = callHelper("keymap_bindings", settings, defaults).jsonObject
    KeyBindings(
        sections = o["sections"]?.jsonArray?.associate { s ->
            val so = s.jsonObject
            so.getValue("id").jsonPrimitive.content to so["actions"]?.jsonArray.orEmpty().map { a ->
                val ao = a.jsonObject
                Binding(
                    action = ao.getValue("action").jsonPrimitive.content,
                    about = ao["about"]?.jsonPrimitive?.contentOrNull ?: "",
                    defaults = strings(ao["defaults"]),
                    keys = ao["keys"]?.takeIf { it !is JsonNull }?.let(::strings),
                )
            }
        } ?: emptyMap(),
        problems = strings(o["problems"]),
    )
}.getOrElse { KeyBindings() }

/**
 * [settings] with an action's keys in a section set (`[]` for none), or taken out (null), bound in its group with the
 * default keys it no longer has unbound; the rest in its order.
 */
fun withBinding(
    settings: JsonObject, section: String, action: String, keys: List<String>?, defaults: JsonObject = KEYMAP_DEFAULTS,
): JsonObject {
    val next = callHelper(
        "keymap_set", settings, JsonPrimitive(section), JsonPrimitive(action),
        keys?.let { k -> JsonArray(k.map(::JsonPrimitive)) } ?: JsonNull, defaults,
    ).jsonObject
    return JsonObject(buildMap { for (k in settings.keys) next[k]?.let { put(k, it) }; putAll(next.filterKeys { it !in settings }) })
}

/**
 * `settings.json` brought to the latest version (`tc_core::settings_version`: the keys bound per group since 2), or null
 * when it already is. Every front end does this when it reads the file, and saves what comes back.
 */
fun migrateSettings(settings: JsonObject, defaults: JsonObject = KEYMAP_DEFAULTS): JsonObject? = runCatching {
    callHelper("settings_migrate", settings, defaults) as? JsonObject
}.getOrNull()

/** A command of the `:` line (spec 0020), or a line that moves to a row: as written (`:tabn[ext]`), what it does and takes, and its part of the documentation (empty for a line). */
data class KeyCommand(val written: String, val what: String, val link: String)

/** The commands as people read them: `web/public/keymaps/commands.json`, copied into the resources by scripts/gen-lucide.mjs. */
val COMMANDS_DOC: JsonObject by lazy {
    runCatching { engineJson.parseToJsonElement(readResource("/keymaps/commands.json") ?: "{}").jsonObject }.getOrElse { JsonObject(emptyMap()) }
}

/** Whether what came in [since] is in a build of [version]: `unreleased` is (it comes out with it), and up to it; with no version known, all. */
fun haveSince(since: String, version: String = APP_VERSION): Boolean =
    since == "unreleased" || version.isEmpty() || !isNewer(since, version)

private fun text(o: JsonObject, key: String): String = (o[key] as? JsonPrimitive)?.contentOrNull ?: ""

/** Every command of this build, then the lines that move to a row, from [doc]. */
fun keyCommands(doc: JsonObject = COMMANDS_DOC, version: String = APP_VERSION): List<KeyCommand> {
    val docs = text(doc, "docs")
    fun what(o: JsonObject) = text(o, "summary") + text(o, "takes").let { if (it.isEmpty()) "" else "; takes $it" }
    fun list(key: String) = (doc[key] as? JsonArray).orEmpty().mapNotNull { it as? JsonObject }.filter { haveSince(text(it, "since"), version) }
    return list("commands").map { c ->
        val name = text(c, "name")
        val shortest = (c["shortest"] as? JsonPrimitive)?.contentOrNull?.toIntOrNull() ?: name.length
        val written = if (shortest < name.length) ":" + name.take(shortest) + "[" + name.drop(shortest) + "]" else ":$name"
        KeyCommand(written, what(c), "$docs#$name")
    } + list("lines").map { l ->
        KeyCommand((l["written"] as? JsonArray).orEmpty().joinToString(" ") { (it as? JsonPrimitive)?.contentOrNull ?: "" }, what(l), "")
    }
}

/** The commands that match [query] (KeysPanel.svelte): the name whole (`denot`), or what it does and takes. */
fun commandRows(commands: List<KeyCommand>, query: String): List<KeyCommand> {
    val t = query.trim().lowercase()
    return commands.filter { c -> t.isEmpty() || listOf(c.written.replace("[", "").replace("]", ""), c.what).any { t in it.lowercase() } }
}

/** A key as typed (`Ctrl-K`), as `settings.json` writes it, or what is wrong with it: exactly one of the two. */
fun parseKey(text: String): Pair<String?, String?> = runCatching {
    val o = callHelper("key_parse", JsonPrimitive(text)).jsonObject
    o["key"]?.jsonPrimitive?.contentOrNull to o["problem"]?.jsonPrimitive?.contentOrNull
}.getOrElse { null to (it.message ?: it.toString()) }

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
