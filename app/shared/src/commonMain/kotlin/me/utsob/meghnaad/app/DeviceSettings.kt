package me.utsob.meghnaad.app

import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.intOrNull
import kotlin.random.Random

// A device's own settings (spec 0027): a partial `settings.json` kept as `devices/<id>.json`, laid over everyone's. The
// home page, the keys (binding by binding) and the reminders may be a device's own; the rest is everyone's. Ported from
// web/src/lib/deviceSettings.ts with its tests.

/** Where the phone remembers which device's settings it uses. */
const val DEVICE_ID_KEY = "device-id"

private val KEY_HOLDERS = listOf("keys", "web", "mobile", "tui")

/** Why [id] can't name a device's settings, or null: letters, digits, `-` and `_`, 1 to 64. */
fun deviceIdProblem(id: String): String? = when {
    id.isEmpty() || id.length > 64 -> "A device ID is 1 to 64 characters."
    !id.all { it.isLetterOrDigit() && it.code < 128 || it == '-' || it == '_' } -> "A device ID is letters, digits, - and _."
    else -> null
}

/** A new device's ID, to keep or type over: a word for the kind of device and a few random letters. */
fun newDeviceId(kind: String = "phone", random: Random = Random.Default): String {
    val letters = "abcdefghijkmnpqrstuvwxyz23456789"
    return "$kind-" + (1..6).map { letters[random.nextInt(letters.length)] }.joinToString("")
}

private fun groupsOf(raw: JsonObject, section: String): Map<String, JsonObject> {
    val at = if (section == "keys") raw["keys"] else (raw[section] as? JsonObject)?.get("keys")
    val o = at as? JsonObject ?: return emptyMap()
    return o.mapNotNull { (g, v) -> (v as? JsonObject)?.let { g to it } }.toMap()
}

private fun withGroups(raw: Map<String, JsonElement>, section: String, groups: Map<String, JsonObject>): Map<String, JsonElement> {
    val out = raw.toMutableMap()
    if (section == "keys") {
        if (groups.isEmpty()) out.remove("keys") else out["keys"] = JsonObject(groups)
        return out
    }
    val holder = ((raw[section] as? JsonObject) ?: JsonObject(emptyMap())).toMutableMap()
    if (groups.isEmpty()) holder.remove("keys") else holder["keys"] = JsonObject(groups)
    if (holder.isEmpty()) out.remove(section) else out[section] = JsonObject(holder)
    return out
}

/**
 * What this device uses: [base] (`settings.json`) with [device] laid over it. The home page whole, the keys one by one (a
 * key the device writes, null included, is its own); the rest is [base]'s.
 */
fun mergeDevice(base: JsonObject, device: JsonObject?): JsonObject {
    if (device == null) return base
    var out: Map<String, JsonElement> = base.toMutableMap().also { m -> device["homepage"]?.let { m["homepage"] = it } }
    for (section in KEY_HOLDERS) {
        val mine = groupsOf(device, section)
        if (mine.isEmpty()) continue
        val groups = groupsOf(base, section).toMutableMap()
        for ((g, keys) in mine) groups[g] = JsonObject((groups[g] ?: JsonObject(emptyMap())) + keys)
        out = withGroups(out, section, groups)
    }
    return JsonObject(out)
}

/**
 * [device] with its keys made what tells [draft] (the keys as edited over the merged settings) from [base]: each key whose
 * command differs, and null for one [base] binds that the draft took out. A key the same as [base]'s is left out.
 */
fun withDeviceKeys(device: JsonObject, base: JsonObject, draft: JsonObject): JsonObject {
    var out: Map<String, JsonElement> = device
    for (section in KEY_HOLDERS) {
        val was = groupsOf(base, section)
        val now = groupsOf(draft, section)
        val diff = mutableMapOf<String, JsonObject>()
        for (g in was.keys + now.keys) {
            val a = was[g] ?: JsonObject(emptyMap())
            val b = now[g] ?: JsonObject(emptyMap())
            val keys = mutableMapOf<String, JsonElement>()
            for (k in a.keys + b.keys) {
                val v = b[k] ?: JsonNull
                if (k !in a || a[k] != v) keys[k] = v
            }
            if (keys.isNotEmpty()) diff[g] = JsonObject(keys)
        }
        out = withGroups(out, section, diff)
    }
    return JsonObject(out)
}

/** [device] with the home page its own ([own]: [homepage], null for `next`) or everyone's again. */
fun withDeviceHomepage(device: JsonObject, homepage: Homepage?, own: Boolean = true): JsonObject {
    val out = device.toMutableMap()
    if (!own) out.remove("homepage") else out["homepage"] = homepage?.let(::homepageJson) ?: JsonNull
    return JsonObject(out)
}

/** The reminders a device's file holds, over the defaults; null when it holds none. */
fun deviceReminders(device: JsonObject?): NotifySettings? {
    val r = device?.get("reminders") as? JsonObject ?: return null
    val d = NotifySettings()
    fun p(k: String) = r[k] as? JsonPrimitive
    return NotifySettings(
        enabled = p("enabled")?.takeIf { !it.isString }?.booleanOrNull ?: d.enabled,
        leadMinutes = p("leadMinutes")?.takeIf { !it.isString }?.intOrNull ?: d.leadMinutes,
        allDayHour = p("allDayHour")?.takeIf { !it.isString }?.intOrNull ?: d.allDayHour,
        waitOver = p("waitOver")?.takeIf { !it.isString }?.booleanOrNull ?: d.waitOver,
    )
}

/** Reminders as a device's file writes them. */
fun remindersJson(s: NotifySettings): JsonObject = JsonObject(
    mapOf(
        "enabled" to JsonPrimitive(s.enabled),
        "leadMinutes" to JsonPrimitive(s.leadMinutes),
        "allDayHour" to JsonPrimitive(s.allDayHour),
        "waitOver" to JsonPrimitive(s.waitOver),
    ),
)

/** A device's file with its ID and version set, and [reminders] (the phone's before) when it holds none of its own. */
fun deviceFile(device: JsonObject, id: String, version: JsonElement?, reminders: NotifySettings?): JsonObject {
    val out = device.toMutableMap()
    out["deviceId"] = JsonPrimitive(id)
    if (version != null) out["version"] = version
    if (out["reminders"] !is JsonObject && reminders != null) out["reminders"] = remindersJson(reminders)
    return JsonObject(out)
}

/** Whether a device's file holds an override of anything: a home page, a key or reminders. */
fun hasOverrides(device: JsonObject?): Boolean {
    if (device == null) return false
    if ("homepage" in device || device["reminders"] is JsonObject) return true
    return KEY_HOLDERS.any { groupsOf(device, it).isNotEmpty() }
}
