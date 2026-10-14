package me.utsob.meghnaad.app

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/** A command the engine asked about, and what to repeat once it is answered. */
class Pending(val question: ConfirmResult, val line: String?, val args: List<String>?)

/**
 * What the screens show and what they can ask: the counterpart of `web/src/lib/store.svelte.ts`. All Taskwarrior behaviour
 * is the engine's; this only keeps the last answer and the state of the request.
 */
class AppModel(private val tw: Tw, private val scope: CoroutineScope, private val clock: Clock) {
    var reports by mutableStateOf(emptyList<ReportMeta>())
        private set
    var response by mutableStateOf<CliResponse?>(null)
        private set

    /** Why the engine could not start, or a command was refused. */
    var error by mutableStateOf<String?>(null)
        private set
    var busy by mutableStateOf(false)
        private set
    var pending by mutableStateOf<Pending?>(null)
        private set

    /** The offset of the user's own time zone, for dates. */
    val utcOffset get() = clock.utcOffsetSeconds()

    // One command at a time: the engine is a single replica.
    private val turn = Mutex()

    fun start(first: String = "next") {
        scope.launch {
            turn.withLock {
                busy = true
                try {
                    reports = tw.start().reports
                    execute(line = first, args = null, answers = Answers())
                } catch (e: Exception) {
                    error = e.message ?: e.toString()
                } finally {
                    busy = false
                }
            }
        }
    }

    fun run(line: String) = launchRun(line, null, Answers())

    fun runReport(name: String) = launchRun(null, listOf(name), Answers())

    /** Answer the engine's question: yes (to go ahead), or no (nothing is changed). */
    fun answer(yes: Boolean) {
        val asked = pending ?: return
        pending = null
        if (!yes) return
        val answers = when (asked.question.ask) {
            "permission" -> Answers(approved = asked.question.items.map { it.key })
            "extras" -> Answers(extras = asked.question.items.map { it.key })
            else -> Answers(confirmed = true)
        }
        launchRun(asked.line, asked.args, answers)
    }

    fun dismissError() {
        error = null
    }

    private fun launchRun(line: String?, args: List<String>?, answers: Answers) {
        scope.launch {
            turn.withLock {
                busy = true
                try {
                    execute(line, args, answers)
                } catch (e: Exception) {
                    error = e.message ?: e.toString()
                } finally {
                    busy = false
                }
            }
        }
    }

    private suspend fun execute(line: String?, args: List<String>?, answers: Answers) {
        error = null
        val got = tw.run(line, args, answers)
        val result = got.result
        if (result is CliResult.Confirm) {
            pending = Pending(result.value, line, args)
            return
        }
        // A command that is not a report leaves the last report where it is; only what is new is shown over it.
        response = got
    }
}
