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

private fun isTag(w: String) = Regex("^\\+[^\\s+-]").containsMatchIn(w) || Regex("^\\+\\S+$").matches(w)

/** Where an `( +a or +b )` group of tags starts and ends in [words], or null (web/src/lib/tagfilter.ts). */
private fun tagGroup(words: List<String>): Pair<Int, Int>? {
    var start = words.indexOf("(")
    while (start >= 0) {
        var i = start + 1
        var tags = 0
        while (i < words.size && isTag(words[i])) {
            tags++
            i++
            if (words.getOrNull(i) != "or") break
            i++
        }
        if (tags >= 2 && words.getOrNull(i) == ")" && words[i - 1] != "or") return start to i
        start = words.drop(start + 1).indexOf("(").let { if (it < 0) -1 else start + 1 + it }
    }
    return null
}

/** The tags a filter requires, as in `+work +home` or `( +work or +home )` (a `-tag` or any other word is not one). */
fun activeTags(filter: String): List<String> = splitWords(filter).filter(::isTag).map { it.drop(1) }

/**
 * [filter] with `+tag` removed if it is there, else added (replacing a `-tag` that would contradict it). A second tag joins
 * the first with `or`, and a third goes into the same parentheses; tags the person wrote together, which all have to be
 * there, have the next one added with `and`.
 */
fun toggleTag(filter: String, tag: String): String {
    var words = splitWords(filter)
    val plus = "+$tag"
    val group = tagGroup(words)
    if (group != null) {
        val (start, end) = group
        val inside = words.subList(start + 1, end).filter { it != "or" }
        words = if (plus in inside) {
            val left = inside.filter { it != plus }
            val joined = if (left.size == 1) left else listOf("(") + left.flatMapIndexed { i, w -> if (i == 0) listOf(w) else listOf("or", w) } + ")"
            words.subList(0, start) + joined + words.subList(end + 1, words.size)
        } else {
            (words.subList(0, end) + listOf("or", plus) + words.subList(end, words.size)).filter { it != "-$tag" }
        }
    } else if (plus in words) {
        words = words.filter { it != plus }
    } else {
        val alone = words.filter(::isTag)
        words = words.filter { it != "-$tag" }
        words = if (alone.size == 1) words.flatMap { if (it == alone[0]) listOf("(", it, "or", plus, ")") else listOf(it) } else words + plus
    }
    return joinWords(words)
}
