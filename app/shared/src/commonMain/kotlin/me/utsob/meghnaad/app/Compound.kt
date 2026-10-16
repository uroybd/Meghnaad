package me.utsob.meghnaad.app

// Compound command lines: `a && b`, `a || b`, `a ; b`. They are the shell's, not Taskwarrior's, so the app reads them and runs
// one statement at a time; the engine only ever sees one plain command per request (web/src/lib/compound.ts).
//
// As in bash, an operator that is not quoted or escaped always separates statements, so a filter written with `&&` or `||` has
// to be written `and` or `or` (or quoted, `'||'`). Operators bind left to right with no precedence: `a && b || c` is
// `(a && b) || c`, and a statement that is skipped leaves the status as it was.

/** How a statement follows the one before it. */
enum class Joiner(val text: String) { And("&&"), Or("||"), Then(";") }

/** One statement of a line, and how it follows the previous one (null for the first). [line] keeps its own quoting. */
data class Statement(val line: String, val join: Joiner?)

/** A line that is not a well-formed chain (an operator with nothing on one side). */
class ChainError(message: String) : Exception(message)

/**
 * The statements of a line. A line without operators is one statement, as typed (trimmed). Quotes (`'` and `"`) and a backslash
 * protect an operator the way the engine's own word splitting reads them; a trailing `;` is allowed.
 */
fun splitChain(line: String): List<Statement> {
    val texts = mutableListOf(StringBuilder())
    val joins = mutableListOf<Joiner?>(null)
    var quote: Char? = null
    var i = 0
    while (i < line.length) {
        val c = line[i]
        val cur = texts.last()
        if (quote != null) {
            cur.append(c)
            if (c == quote) quote = null
        } else if (c == '\'' || c == '"') {
            quote = c
            cur.append(c)
        } else if (c == '\\' && i + 1 < line.length) {
            val next = line[i + 1]
            // The engine reads `\'`, `\"`, `\\` and `\ ` itself; an escaped operator character becomes a quoted one.
            when {
                next in "'\"\\ " -> cur.append(c).append(next)
                next in "&|;" -> cur.append('\'').append(next).append('\'')
                else -> cur.append(c)
            }
            if (next in "'\"\\ &|;") i++
        } else if (c == ';') {
            texts.add(StringBuilder())
            joins.add(Joiner.Then)
        } else if ((c == '&' || c == '|') && line.getOrNull(i + 1) == c) {
            texts.add(StringBuilder())
            joins.add(if (c == '&') Joiner.And else Joiner.Or)
            i++
        } else {
            cur.append(c)
        }
        i++
    }
    val out = texts.indices.map { Statement(texts[it].toString().trim(), joins[it]) }.toMutableList()
    // A trailing `;` ends the last statement, as in a shell.
    if (out.size > 1 && out.last().line.isEmpty() && out.last().join == Joiner.Then) out.removeAt(out.size - 1)
    if (out.size == 1) return if (out[0].line.isEmpty()) emptyList() else out
    out.forEachIndexed { i, s ->
        if (s.line.isNotEmpty()) return@forEachIndexed
        throw ChainError(
            when (i) {
                0 -> "Nothing before ‘${out[1].join?.text}’."
                out.size - 1 -> "Nothing after ‘${s.join?.text}’."
                else -> "Nothing between ‘${s.join?.text}’ and ‘${out[i + 1].join?.text}’."
            },
        )
    }
    return out
}

/**
 * Run the statements in order, as a shell does: `&&` runs the next one only after a success, `||` only after a failure, `;`
 * always. [step] runs statement `i` and says whether it succeeded. A statement that is skipped leaves the status as it was.
 * Returns the status of the last statement that ran.
 */
suspend fun runSequence(joins: List<Joiner?>, step: suspend (Int) -> Boolean): Boolean {
    var ok = true
    for (i in joins.indices) {
        val join = joins[i]
        if ((join == Joiner.And && !ok) || (join == Joiner.Or && ok)) continue
        ok = step(i)
    }
    return ok
}
