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

/**
 * The line with a task command in it written out, or the line as it was when there is none. The command word is the first
 * word that is not part of the filter; the rest of the line goes after what the command stands for, so `1 fail +x` is
 * `1 done outcome:fail +x`.
 */
fun expandTaskCommands(line: String, commands: List<TaskCommand>): String {
    if (commands.isEmpty()) return line
    val byName = commands.associate { it.name to it.command }
    var words = splitWords(line)
    if (words.firstOrNull() == "task") words = words.drop(1)
    var changed = false
    // A name is written out once: `done` that stands for `done outcome:ok` then reaches Taskwarrior's `done`, and two that
    // stand for each other stop after one round each.
    val used = HashSet<String>()
    for (round in 0 until ROUNDS) {
        val at = words.indexOfFirst { it == "--" || !isFilterWord(it) }
        if (at < 0 || words[at] == "--" || words[at] in used) break
        val stands = byName[words[at]] ?: break
        used += words[at]
        words = words.take(at) + splitWords(stands) + words.drop(at + 1)
        changed = true
    }
    return if (changed) joinWords(words) else line
}

/** The words a command adds after the tasks it is run on (a tap on a task or on the selected tasks). */
fun commandWords(c: TaskCommand): List<String> = splitWords(c.command)
