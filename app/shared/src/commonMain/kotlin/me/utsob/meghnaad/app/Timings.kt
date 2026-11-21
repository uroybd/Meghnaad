package me.utsob.meghnaad.app

import kotlin.time.TimeSource

/**
 * How long the engine's calls and a sync's stretches take, written to the log of a debug build (the app sets [log] only
 * when it is debuggable; a release leaves it null, and nothing is measured or written).
 */
object Timings {
    var log: ((String) -> Unit)? = null

    /** A point to measure from, or null when nothing is logged. */
    fun mark(): TimeSource.Monotonic.ValueTimeMark? = if (log == null) null else TimeSource.Monotonic.markNow()

    /** Log [what] with the milliseconds since [from] (nothing when [from] is null). */
    fun since(from: TimeSource.Monotonic.ValueTimeMark?, what: String) {
        val write = log ?: return
        if (from == null) return
        write("$what ${from.elapsedNow().inWholeMilliseconds} ms")
    }
}
