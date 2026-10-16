package me.utsob.meghnaad.app

// Task commands: a word that stands for a whole command line. `fail` for `done outcome:fail`, `late` for `+OVERDUE list`. They
// work with or without `task` in front, and with a filter before them (`project:Home fail`, `3 fail`, `+work late`), which
// stays in front of what the command stands for.
//
// Unlike the taskrc's `alias.*` (the engine's, which stands for words anywhere on the line), this is the app's: it reads the
// line as Taskwarrior does, a filter and then the command, and only the command word can be a task command. So `add fail the
// test` is still a task named "fail the test". A task command wins over a Taskwarrior command or report with the same name
// (web/src/lib/taskCommands.ts).

/** Taskwarrior's limit for aliases that use aliases. */
private const val ROUNDS = 10

private val UUID = Regex("^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$", RegexOption.IGNORE_CASE)
private val LOGIC = setOf("(", ")", "and", "or", "xor", "not", "!")

/**
 * Is this word part of a filter, so the command (if there is one) comes later? The same shapes the engine takes for a filter:
 * `+tag` and `-tag`, anything with `:` or `=` (attributes, `rc.` settings), task numbers and ranges, `/pattern/`, a uuid, and
 * the words that join them.
 */
fun isFilterWord(w: String): Boolean =
    Regex("^[+-].").containsMatchIn(w) ||
        Regex("[:=]").containsMatchIn(w) ||
        Regex("^[\\d,.-]+$").matches(w) ||
        w.startsWith("/") ||
        UUID.matches(w) ||
        w.lowercase() in LOGIC

/** One statement of a line, as the words to run and how it follows the one before. */
private class Piece(val words: List<String>, val join: Joiner?)

/**
 * The statements a task command stands for in [words] (a statement as words), or null when the command word is not a task
 * command. The words before the command word (the filter) go in front of every statement it stands for, and the words after it
 * go after the last one, as an alias in a shell takes its arguments.
 */
private fun expandWords(words: List<String>, byName: Map<String, String>, used: Set<String>, depth: Int): List<Piece>? {
    if (depth >= ROUNDS) return null
    val at = words.indexOfFirst { it == "--" || !isFilterWord(it) }
    if (at < 0 || words[at] == "--" || words[at] in used) return null
    val stands = byName[words[at]] ?: return null
    // A name is written out once: `done` that stands for `done outcome:ok` then reaches Taskwarrior's `done`, and two that
    // stand for each other stop after one round each.
    val seen = used + words[at]
    val inner = splitChain(stands)
    val out = mutableListOf<Piece>()
    inner.forEachIndexed { i, s ->
        val own = words.take(at) + splitWords(s.line) + (if (i == inner.size - 1) words.drop(at + 1) else emptyList())
        val deeper = expandWords(own, byName, seen, depth + 1) ?: listOf(Piece(own, null))
        deeper.forEachIndexed { j, p -> out.add(Piece(p.words, if (j == 0) s.join else p.join)) }
    }
    return out
}

/**
 * The statements of a line (`a && b ; c`, see Compound.kt) with each task command in them written out. A statement that is not
 * a task command is kept as it was typed. Throws a [ChainError] for a line that is not a well-formed chain.
 */
fun planChain(line: String, commands: List<TaskCommand>): List<Statement> = plan(line, commands).first

private fun plan(line: String, commands: List<TaskCommand>): Pair<List<Statement>, Boolean> {
    val parts = splitChain(line)
    if (commands.isEmpty()) return parts to false
    val byName = commands.associate { it.name to it.command }
    val steps = mutableListOf<Statement>()
    var expanded = false
    for (s in parts) {
        var words = splitWords(s.line)
        if (words.firstOrNull() == "task") words = words.drop(1)
        val pieces = expandWords(words, byName, emptySet(), 0)
        if (pieces == null) {
            steps += s
            continue
        }
        expanded = true
        pieces.forEachIndexed { j, p -> steps += Statement(joinWords(p.words), if (j == 0) s.join else p.join) }
    }
    return steps to expanded
}

/**
 * The line with its task commands written out, or the line as it was when there is none. A task command's own statements are
 * joined with their operators again, so the line reads as what runs.
 */
fun expandTaskCommands(line: String, commands: List<TaskCommand>): String {
    val (steps, expanded) = plan(line, commands)
    if (!expanded) return line
    return steps.withIndex().joinToString("") { (i, s) -> (if (i > 0) " ${s.join?.text} " else "") + s.line }
}

/** One statement of a task command: the words it adds after the tasks it is run on, and how it follows the one before. */
data class CommandStep(val words: List<String>, val join: Joiner?)

/** The statements a command stands for, as a tap on one or more tasks runs them. */
fun commandSteps(c: TaskCommand): List<CommandStep> = splitChain(c.command).map { CommandStep(splitWords(it.line), it.join) }
