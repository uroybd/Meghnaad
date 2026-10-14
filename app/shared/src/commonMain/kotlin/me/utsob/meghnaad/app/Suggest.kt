package me.utsob.meghnaad.app

// Type-ahead for the form fields that take names (a project, a list of tags): which names to offer for what is being typed, and
// what the field says once one is chosen. Typing a name that is not on offer is always fine; the offers are only a help. (The
// console's Tab completion is `Completion.kt`.) web/src/lib/suggest.ts.

/** The part of a field being typed: [text] is what to offer names for, `start..end` is what a choice replaces. */
data class Word(val start: Int, val end: Int, val text: String)

private const val MAX = 8

private fun isSeparator(c: Char) = c.isWhitespace() || c == ','

/**
 * For one value (a project) the whole field; for a list (tags, separated by spaces or commas) the word around the caret, without
 * a `+` in front of it.
 */
fun wordAt(value: String, caret: Int, list: Boolean): Word {
    if (!list) return Word(0, value.length, value)
    var start = minOf(caret, value.length)
    var end = start
    while (start > 0 && !isSeparator(value[start - 1])) start--
    while (end < value.length && !isSeparator(value[end])) end++
    if (start < value.length && value[start] == '+') start++
    end = maxOf(start, end)
    return Word(start, end, value.substring(start, end))
}

/**
 * The names to offer for [typed]: those that begin with it first, then those that contain it (so `kit` finds `Home.Kitchen`),
 * ignoring case, never one in [taken]. Nothing when [typed] is already a name and nothing else could be meant.
 */
fun suggest(names: List<String>, typed: String, taken: Set<String> = emptySet()): List<Option> {
    val t = typed.lowercase()
    val pool = names.filter { it !in taken }
    val starts = pool.filter { it.lowercase().startsWith(t) }
    val inside = if (t.isNotEmpty()) pool.filter { !it.lowercase().startsWith(t) && it.lowercase().contains(t) } else emptyList()
    val all = starts + inside
    if (all.size == 1 && all[0] == typed) return emptyList()
    return all.take(MAX).map { Option(it) }
}

class Chosen(val value: String, val caret: Int)

/** [value] with [word] replaced by [choice]; in a list a space follows, ready for the next one. */
fun choose(value: String, word: Word, choice: String, list: Boolean): Chosen {
    val before = value.substring(0, word.start)
    val after = value.substring(word.end)
    // A list gets a space after the word unless one is there already; the caret goes past it either way.
    val gap = if (list && !Regex("^[\\s,]").containsMatchIn(after)) " " else ""
    return Chosen(before + choice + gap + after, before.length + choice.length + if (list) 1 else 0)
}

/** Is [typed] a name nobody has used yet? (Then the field says so.) */
fun isNew(names: List<String>, typed: String): Boolean = typed.trim().let { it.isNotEmpty() && it !in names }
