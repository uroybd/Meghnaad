package me.utsob.meghnaad.app

/**
 * What the screens ask of the Rust engine's library that needs no engine and never waits for a sync (`crates/android`,
 * `helpers.rs`): where `tc-core` already does something, the app calls it instead of keeping a copy, so what it shows is
 * what the engine means.
 */
interface RustHelpers {
    /** The words of a command line as the engine reads them (quotes and escapes honoured). */
    fun splitWords(line: String): List<String>

    /** A date in a taskrc `dateformat` pattern, in the zone [tz] seconds east of UTC (Taskwarrior's `Y-M-D` for no pattern). */
    fun formatDate(pattern: String, epoch: Long, tz: Int, weekStartsMonday: Boolean): String

    /** A duration the way Taskwarrior writes it: `3d`, `2w`, `4mo`, `1.5y`; empty for under a second. */
    fun vague(seconds: Long): String

    /** Any other helper of `tc_core::helpers`, by name: its arguments as a JSON array, its answer as JSON (see [callHelper]). */
    fun call(name: String, args: String): String
}

/** A helper of `tc_core::helpers` by name, with its arguments; its answer, or an error when it is not a helper or not its arguments. */
fun callHelper(name: String, vararg args: kotlinx.serialization.json.JsonElement): kotlinx.serialization.json.JsonElement {
    val out = engineJson.parseToJsonElement(rustHelpers().call(name, kotlinx.serialization.json.JsonArray(args.toList()).toString()))
    (out as? kotlinx.serialization.json.JsonObject)?.get("error")?.let { error("helper $name: ${it.toString()}") }
    return out
}

expect fun rustHelpers(): RustHelpers
