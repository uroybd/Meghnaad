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

/** Where the app is in getting started. */
enum class Setup {
    /** Looking at whether the device is connected to a bucket, or was told to go without. */
    Checking,

    /** Neither: the first screen asks. */
    NeedsConnection,

    /** The engine is starting (and fetching the tasks, the first time). */
    Starting,
    Ready,

    /** The engine could not start: [Store.error] says why. */
    Failed,
}

/**
 * What the screens show and what they can ask: the counterpart of `web/src/lib/store.svelte.ts`. All Taskwarrior behaviour
 * is the engine's; this only keeps the last answer and the state of the request.
 */
class Store(
    private val tw: Tw,
    private val port: EnginePort,
    private val vault: Vault,
    val sync: SyncController,
    private val scope: CoroutineScope,
    private val clock: Clock,
) {
    var setup by mutableStateOf(Setup.Checking)
        private set
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

    /** What the last command that changed tasks said ("Created task 2."), shown over the report that is kept up to date. */
    var notice by mutableStateOf<String?>(null)
        private set

    /** The report on show, which a command that changes tasks draws again. */
    private var reportName = "next"

    /** The offset of the user's own time zone, for dates. */
    val utcOffset get() = clock.utcOffsetSeconds()

    /** Whether the device keeps its tasks here only, so a failed sync is not something to say. */
    val localOnly get() = vault.localOnly

    // One command at a time: the engine is a single replica.
    private val turn = Mutex()

    init {
        // What another device did arrives with a sync: draw again what is on screen.
        sync.onChanged = { redraw() }
    }

    /** Decide what comes first: the connection screen, or the tasks. */
    fun begin() {
        if (vault.connection() == null && !vault.localOnly) setup = Setup.NeedsConnection else boot()
    }

    /**
     * Keep the connection and try it: a first sync fetches the tasks, and a wrong key or secret says so. Returns what went
     * wrong, or null when it worked (and the app goes on to start).
     */
    suspend fun connect(connection: Connection): String? {
        connection.problem()?.let { return it }
        vault.saveConnection(connection)
        vault.localOnly = false
        val result = sync.request().await()
        if (result.isFailure) {
            vault.clearConnection()
            return sync.error ?: "Could not connect."
        }
        boot()
        return null
    }

    /** Go on without a bucket: the tasks stay on this device, and a connection can be made later in Settings. */
    fun useLocally() {
        vault.localOnly = true
        boot()
    }

    /** Throw the copy kept on this device away and load the tasks again from the bucket (for when the two disagree). */
    fun startOver() {
        scope.launch {
            turn.withLock {
                busy = true
                try {
                    port.forget()
                    sync.request().await()
                    execute(line = null, args = listOf(reportName), answers = Answers())
                } catch (e: Exception) {
                    error = e.message ?: e.toString()
                } finally {
                    busy = false
                }
            }
        }
    }

    private fun boot(first: String = "next") {
        setup = Setup.Starting
        scope.launch {
            turn.withLock {
                busy = true
                try {
                    reports = tw.start().reports
                    // With nothing kept here the tasks come from the bucket before the first command is answered; an
                    // unreachable bucket is no reason to stop: the app starts empty and says why it has not synced.
                    if (port.restored) sync.request() else sync.request().await()
                    setup = Setup.Ready
                    execute(line = first, args = null, answers = Answers())
                } catch (e: Exception) {
                    error = e.message ?: e.toString()
                    setup = Setup.Failed
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

    /** Draw the report on show again (after a sync brought changes in). */
    fun redraw() {
        if (setup != Setup.Ready) return
        scope.launch {
            turn.withLock {
                try {
                    response = tw.run(args = listOf(reportName))
                } catch (e: Exception) {
                    error = e.message ?: e.toString()
                }
            }
        }
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
        notice = null
        val got = tw.run(line, args, answers)
        // A write is already applied and kept here: push it in the background.
        if (got.wrote) sync.pushSoon()
        when (val result = got.result) {
            is CliResult.Confirm -> pending = Pending(result.value, line, args)
            is CliResult.Report -> {
                reportName = result.value.report
                response = got
            }
            is CliResult.Changed -> {
                notice = (listOf(result.value.message) + got.feedback.map { it.text }).joinToString(" ")
                // The tasks changed: draw the report again, so the list is as the engine now has it.
                response = if (got.wrote) tw.run(args = listOf(reportName)) else got
            }
            else -> response = got
        }
    }
}
