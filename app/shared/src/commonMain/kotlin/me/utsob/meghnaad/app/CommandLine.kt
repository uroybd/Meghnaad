package me.utsob.meghnaad.app

// Turning between a typed command line and the argument list the engine takes: web/src/lib/cmdline.ts and tagfilter.ts.
// Splitting is the engine's own (`splitWords` asks it), so what is previewed is what runs.

/** The words of a command line, as the engine reads them: the engine's own `filter::split_words`. */
fun splitWords(s: String): List<String> = rustHelpers().splitWords(s)

/** A word as it is typed in a command line: as it is when that is safe, else in single quotes (`tc_core::helpers::shell_quote`). */
fun shellQuote(a: String): String = callHelper("shell_quote", kotlinx.serialization.json.JsonPrimitive(a)).let { (it as kotlinx.serialization.json.JsonPrimitive).content }

/** The command as it would be typed: `task project:Home +work list`. */
fun previewLine(args: List<String>): String = (listOf("task") + args.map(::shellQuote)).joinToString(" ")

/** Filter words and a report name, as sent to the engine. */
fun reportArgs(filter: String, report: String): List<String> = splitWords(filter) + report

fun joinWords(words: List<String>): String = words.joinToString(" ", transform = ::shellQuote)

/** The tags a filter requires, as in `+work +home` (a `-tag` or any other word is not one). */
fun activeTags(filter: String): List<String> =
    splitWords(filter).filter { Regex("^\\+[^\\s+-]").containsMatchIn(it) || Regex("^\\+\\S+$").matches(it) }.map { it.drop(1) }

/** [filter] with `+tag` removed if it is there, else added (replacing a `-tag` that would contradict it). */
fun toggleTag(filter: String, tag: String): String {
    val words = splitWords(filter)
    val plus = "+$tag"
    val next = if (plus in words) words.filter { it != plus } else words.filter { it != "-$tag" } + plus
    return joinWords(next)
}
