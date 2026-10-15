package me.utsob.meghnaad.app

// Recurrence periods as Taskwarrior writes them (`recur:weekly`, `recur:3d`, ISO `P2W`). The engine validates; this only
// offers sensible choices and describes what is stored (web/src/lib/recurrence.ts).

class Preset(val value: String, val label: String)

val PRESETS = listOf(
    Preset("daily", "Every day"),
    Preset("weekdays", "Every weekday"),
    Preset("weekly", "Every week"),
    Preset("biweekly", "Every 2 weeks"),
    Preset("monthly", "Every month"),
    Preset("quarterly", "Every 3 months"),
    Preset("yearly", "Every year"),
)

/** "every 3 days" for `3d`; unknown spellings come back as typed. The words are the engine's (`tc_core::helpers::describe_recur`). */
fun describeRecur(period: String?): String =
    callHelper("describe_recur", kotlinx.serialization.json.JsonPrimitive(period ?: "")).let { (it as kotlinx.serialization.json.JsonPrimitive).content }

/** The preset a stored period corresponds to exactly, or empty when it needs the custom field. */
fun presetFor(period: String?): String = (period ?: "").trim().let { p -> if (PRESETS.any { it.value == p }) p else "" }

/** `recur:` completion candidates, with the plain-English meaning as a hint. */
class RecurWord(val value: String, val hint: String)

val RECUR_WORDS = PRESETS.map { RecurWord(it.value, it.label.lowercase()) } +
    listOf(RecurWord("3d", "every 3 days"), RecurWord("2w", "every 2 weeks"), RecurWord("2m", "every 2 months"))
