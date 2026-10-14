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
}

expect fun rustHelpers(): RustHelpers
