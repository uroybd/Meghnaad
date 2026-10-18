package me.utsob.meghnaad.app

import kotlinx.serialization.builtins.MapSerializer
import kotlinx.serialization.builtins.serializer

// A page's own context (web/src/lib/pageContext.ts). The context set with `context work` is the console's, as it is Taskwarrior's; a
// page may choose another, or none, for what it shows. It is sent the way Taskwarrior takes an override on a command line,
// `rc.context=<name>`, so the engine does nothing new.

/** What a page may say it wants: no context at all (as `context none`). */
const val NO_CONTEXT = "none"

/** The pages that choose a context: each report page, and each board. */
object Scope {
    const val Tasks = "tasks"
    const val Projects = "projects"
    const val Tags = "tags"
    fun panel(which: Panel) = which.name.lowercase()
    fun board(id: String) = "board:$id"
}

/** The word that makes a command run in [choice], a context's name or `none`. */
fun contextWord(choice: String) = "rc.context=$choice"

/** [input] run in the context [choice] names; unchanged when the page follows the active one (null). */
fun withContext(input: CliInput, choice: String?): CliInput {
    if (choice.isNullOrEmpty()) return input
    val word = contextWord(choice)
    return CliInput(
        line = input.line?.let { "$word $it" },
        args = input.args?.let { listOf(word) + it },
    )
}

private val choicesSerializer = MapSerializer(String.serializer(), String.serializer())

const val CONTEXT_CHOICES_KEY = "page-contexts"

/** The choices kept on this device, as saved; a damaged value is nothing. */
fun loadChoices(text: String?): Map<String, String> = try {
    if (text == null) emptyMap() else engineJson.decodeFromString(choicesSerializer, text).filterValues { it.isNotEmpty() }
} catch (e: Exception) {
    emptyMap()
}

fun saveChoices(choices: Map<String, String>): String = engineJson.encodeToString(choicesSerializer, choices)
