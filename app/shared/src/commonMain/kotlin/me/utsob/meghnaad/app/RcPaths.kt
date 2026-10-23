package me.utsob.meghnaad.app

// The paths of the taskrc folder, as the screens handle them: which files an upload holds and where they go, and the `include` line that
// names one file from another (web/src/lib/rcpaths.ts). What the engine does with a path is `tc_core::rcfiles`.

/** The file everything starts from. */
const val RC_ROOT = "taskrc"

/** What the engine takes: at most this many files of this much text in all. */
const val RC_MAX_FILES = 64
const val RC_MAX_FILE_BYTES = 256 * 1024
const val RC_MAX_TOTAL_BYTES = 1024 * 1024

/** The `include` line that names [to] from [from]: the path is written from the folder [from] is in. */
fun includeLine(from: String, to: String): String {
    val here = from.split('/').dropLast(1)
    val there = to.split('/')
    var common = 0
    while (common < here.size && common < there.size - 1 && here[common] == there[common]) common++
    return "include " + "../".repeat(here.size - common) + there.drop(common).joinToString("/")
}

/** Add an `include` of [to] to the text of [from], on a line of its own, unless it is already there. */
fun withInclude(text: String, from: String, to: String): String {
    val line = includeLine(from, to)
    if (text.split("\r\n", "\n").any { it.trim() == line }) return text
    return text + (if (text.isEmpty() || text.endsWith("\n")) "" else "\n") + line + "\n"
}

/** A file picked for upload: its name, the path inside the folder that was picked (if one was), and its text (null: too large). */
class Picked(val name: String, val relative: String?, val text: String?)

/** What to send: each file's path and text, and what was left out and why. */
class Upload(val files: List<Pair<String, String>>, val skipped: List<String>)

/**
 * Which of the picked files to send, and where each goes. The folder that was picked is not part of a path (its files are `taskrc`,
 * `holidays/en-US.rc`), `.taskrc` at the top is `taskrc`, and what is not a taskrc file (hidden files and folders such as `.git`,
 * binary files, anything too large, anything past the most a folder holds) is left out and said.
 */
fun uploadFiles(picked: List<Picked>): Upload {
    val files = ArrayList<Pair<String, String>>()
    val skipped = ArrayList<String>()
    val seen = HashSet<String>()
    var total = 0
    for (p in picked) {
        val whole = (p.relative ?: p.name).split('/')
        val parts = (if (p.relative != null) whole.drop(1) else whole).toMutableList()
        val label = whole.joinToString("/")
        if (parts.isEmpty()) continue
        if (parts.size == 1 && parts[0] == ".taskrc") parts[0] = RC_ROOT
        else if (parts.any { it.startsWith(".") }) {
            skipped.add("$label (hidden)")
            continue
        }
        val path = parts.joinToString("/")
        val text = p.text
        when {
            text == null || text.length > RC_MAX_FILE_BYTES -> skipped.add("$label (too large)")
            '\u0000' in text -> skipped.add("$label (not text)")
            path in seen -> skipped.add("$label (the same name as another)")
            files.size >= RC_MAX_FILES || total + text.length > RC_MAX_TOTAL_BYTES -> skipped.add("$label (the folder is full)")
            else -> {
                seen.add(path)
                total += text.length
                files.add(path to text)
            }
        }
    }
    // `taskrc` first, so what a folder starts from is the first thing it lists.
    files.sortWith(compareBy<Pair<String, String>>({ it.first != RC_ROOT }, { it.first }))
    return Upload(files, skipped)
}

/** One row of the list of files. [path] is empty on the row that names a folder. */
data class RcRow(val path: String, val depth: Int, val name: String, val folder: String?)

/** The files as the folder is drawn: `taskrc` first, each folder's name once above its files, each row with its depth. */
fun listing(paths: List<String>): List<RcRow> {
    val rows = ArrayList<RcRow>()
    var last = ""
    for (path in paths.sortedWith(compareBy({ it != RC_ROOT }, { it }))) {
        val parts = path.split('/')
        val folder = if (parts.size > 1) parts.dropLast(1).joinToString("/") else null
        if (folder != null && folder != last) rows.add(RcRow("", parts.size - 2, parts[parts.size - 2], folder))
        last = folder ?: ""
        rows.add(RcRow(path, parts.size - 1, parts.last(), folder))
    }
    return rows
}
