package me.utsob.meghnaad.app

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

// tasksh's `review` (web/src/lib/review.svelte.ts): walk the tasks the `_reviewed` report lists, show each, and mark it
// reviewed, skip it, edit it, change it, finish it or delete it. It is only a client of the engine: every step is an ordinary
// command (`modify reviewed:now`, `done`, `delete`), so undo, hooks and the settings work as they do for anything else. A phone
// has no keys, so a review is always the card with buttons (the web's console surface, with keys, is not here).
// (src/review.cpp in https://github.com/GothenburgBitFactory/taskshell)

enum class ReviewPhase { Idle, Setup, Running, Asking, Modifying, Editing, Done }

/** Taskwarrior is told these with every step, as tasksh does: no questions, and nothing but the answer. */
private val QUIET = listOf("rc.confirmation:no", "rc.verbose:nothing")

/** The choices that change the task: what is sent for each, and what is said when it went through. */
private class ReviewWrite(val words: List<String>, val said: String)

private val WRITES = mapOf(
    ReviewKey.Reviewed to ReviewWrite(listOf("modify", "reviewed:now"), "Marked as reviewed."),
    ReviewKey.Complete to ReviewWrite(listOf("done"), "Completed."),
    ReviewKey.Delete to ReviewWrite(listOf("delete"), "Deleted."),
)

/**
 * A question Taskwarrior asked in the middle of a step, which the person answers before it goes ahead: the usual one is "This is a
 * recurring task. Do you want to modify all pending recurrences of this same task?", which `rc.confirmation:no` does not silence
 * (`recurrence.confirmation` governs it), so tasksh shows it too.
 */
private class Asking(
    val words: List<String>,
    val answers: Answers,
    val r: ConfirmResult,
    /** Which of the question's items is being asked. */
    var index: Int,
    /** The keys answered yes so far. */
    val yes: MutableList<String>,
    /** What to do when the step has gone through. */
    val then: suspend () -> Unit,
)

class ReviewSession(private val store: Store) {
    var phase by mutableStateOf(ReviewPhase.Idle)
        private set
    /** The task being looked at, as `info` gives it. */
    var row by mutableStateOf<Row?>(null)
        private set
    var current by mutableStateOf(0)
        private set
    var reviewed by mutableStateOf(0)
        private set
    var total by mutableStateOf(0)
        private set
    /** The latest thing said. */
    var message by mutableStateOf("")
        private set
    var messageBad by mutableStateOf(false)
        private set
    /** The question being asked: the setup's, or one of Taskwarrior's. */
    var question by mutableStateOf<String?>(null)
        private set
    var busy by mutableStateOf(false)
        private set

    private var uuids = emptyList<String>()
    private var limit = 0
    /** What is still to be asked in the setup, and the `config` lines each answer writes. */
    private val parts = ArrayDeque<SetupPart>()
    private var lines = emptyList<Pair<String, String>>()
    private var asking: Asking? = null

    /** Begin a review ([limit] 0 is all of them). Does nothing while one is going. */
    suspend fun start(limit: Int = 0) {
        if (phase != ReviewPhase.Idle) {
            if (phase == ReviewPhase.Done) close() else return say("A review is already in progress.", bad = true)
        }
        this.limit = limit
        uuids = emptyList()
        current = 0
        reviewed = 0
        total = 0
        row = null
        message = ""
        busy = true
        try {
            if (store.config == null) store.loadConfig()
            val status = taskshStatus(store.config)
            if (status.enabled) return begin()
            val want = ReviewWanted(status.filter ?: REVIEW_FILTER, status.sort ?: REVIEW_SORT)
            lines = setupLines(store.config, want)
            parts.clear()
            if (status.uda != Part.Ok) parts.add(SetupPart.Uda)
            if (status.report != Part.Ok) parts.add(SetupPart.Report)
            phase = ReviewPhase.Setup
            ask()
        } finally {
            busy = false
        }
    }

    /** Yes or no to the setup's question (the UDA, then the report). */
    suspend fun answerSetup(yes: Boolean) {
        if (phase != ReviewPhase.Setup || busy) return
        if (!yes) {
            say("Review needs the reviewed UDA and the _reviewed report.", bad = true)
            return end(false)
        }
        val part = parts.removeFirst()
        busy = true
        try {
            for (line in partLines(lines, part)) {
                val r = store.call(configArgs(line)).result
                if (r is CliResult.Error) {
                    say(r.value.message, bad = true)
                    return end(false)
                }
            }
            if (parts.isNotEmpty()) return ask()
            store.settingsChanged()
            begin()
        } catch (e: Exception) {
            say(e.message ?: e.toString(), bad = true)
            end(false)
        } finally {
            busy = false
        }
    }

    /** A key pressed (Enter is the usual answer): the menu's choice, or y or n to a question (`press` in review.svelte.ts). */
    suspend fun press(key: String) {
        if (busy) return
        val k = key.lowercase()
        if (phase == ReviewPhase.Setup && (k == "y" || k == "n")) return answerSetup(k == "y")
        if (phase == ReviewPhase.Asking && (k == "y" || k == "n")) return answer(k == "y")
        if (phase == ReviewPhase.Running) menuKey(if (key == "Enter") "" else key)?.let { act(it) }
    }

    /** One of the menu's choices. */
    suspend fun act(key: ReviewKey) {
        val task = row
        if (phase != ReviewPhase.Running || busy || task == null) return
        busy = true
        try {
            when (key) {
                ReviewKey.Quit -> end()
                ReviewKey.Skip -> {
                    say("Skipped")
                    current++
                    show()
                }
                ReviewKey.Edit -> {
                    // The editor is the form the Edit button opens; closing it carries on (see `editClosed`).
                    phase = ReviewPhase.Editing
                    store.editing = EditRef(task, null)
                }
                ReviewKey.Modify -> {
                    phase = ReviewPhase.Modifying
                    say(REVIEW_MODIFY_PROMPT)
                }
                else -> {
                    val w = WRITES.getValue(key)
                    write(w.words, { say(w.said); reviewed++; current++; show() })
                }
            }
        } finally {
            busy = false
        }
    }

    /** The words after `modify` for the task being looked at; nothing cancels. The same task is shown again. */
    suspend fun modify(input: String) {
        if (phase != ReviewPhase.Modifying || busy) return
        val words = splitWords(input)
        phase = ReviewPhase.Running
        if (words.isEmpty()) return say("No changes.")
        busy = true
        try {
            write(listOf("modify") + words, { say("Modified."); show() })
        } finally {
            busy = false
        }
    }

    /** The editor was closed: as in tasksh, the task is marked reviewed and shown again. */
    suspend fun editClosed() {
        if (phase != ReviewPhase.Editing) return
        phase = ReviewPhase.Running
        busy = true
        try {
            write(listOf("modify", "reviewed:now"), { say("Modified."); show() })
        } finally {
            busy = false
        }
    }

    /** Yes or no to a question Taskwarrior asked in the middle of a step. */
    suspend fun answer(yes: Boolean) {
        val a = asking
        if (phase != ReviewPhase.Asking || a == null || busy) return
        if (a.r.ask == "extras") {
            // One question per item (the recurring series, a dependency to repair); no leaves that part undone and the step itself
            // still goes ahead.
            if (yes) a.yes.add(a.r.items[a.index].key)
            a.index++
            if (a.index < a.r.items.size) return askNext()
            return resend(Answers(extras = a.yes.toList()))
        }
        if (!yes) {
            asking = null
            question = null
            phase = ReviewPhase.Running
            return say("Cancelled.")
        }
        return resend(if (a.r.ask == "plain") Answers(confirmed = true) else Answers(approved = a.r.items.map { it.key }))
    }

    /** Stop, whatever is going on (the card closed). */
    fun quit() {
        when (phase) {
            ReviewPhase.Idle -> {}
            ReviewPhase.Done, ReviewPhase.Setup -> close()
            else -> end()
        }
    }

    /** Back to nothing happening. */
    fun close() {
        phase = ReviewPhase.Idle
        row = null
        question = null
        message = ""
        asking = null
    }

    private fun ask() {
        question = setupQuestion(parts.first(), taskshStatus(store.config))
        say(question!!)
    }

    private fun askNext() {
        val a = asking!!
        question = if (a.r.ask == "extras") a.r.items[a.index].question else a.r.message
        say(question!!)
    }

    private suspend fun resend(more: Answers) {
        val a = asking!!
        asking = null
        question = null
        phase = ReviewPhase.Running
        busy = true
        try {
            write(a.words, a.then, Answers(more.confirmed || a.answers.confirmed, more.approved ?: a.answers.approved, more.extras ?: a.answers.extras))
        } finally {
            busy = false
        }
    }

    private suspend fun begin() {
        val res = store.call(listOf("rc.color=off", "rc.detection=off", "rc._forcecolor=off", "rc.verbose=nothing", REVIEW_REPORT))
        val r = res.result
        if (r !is CliResult.Report) {
            say(if (r is CliResult.Error) r.value.message else "The _reviewed report did not answer with a list.", bad = true)
            return end(false)
        }
        uuids = r.value.rows.map { it.uuid }
        total = if (limit > 0) minOf(limit, uuids.size) else uuids.size
        if (total == 0) {
            say(REVIEW_NOTHING)
            return end(false)
        }
        say(REVIEW_INTRO)
        phase = ReviewPhase.Running
        current = 0
        reviewed = 0
        show()
    }

    /** Show the task at `current`, or finish when they are used up. */
    private suspend fun show() {
        while (current < total && (limit == 0 || reviewed < limit)) {
            val r = store.call(listOf(uuids[current], "info")).result
            val task = (r as? CliResult.Info)?.value?.tasks?.firstOrNull()
            if (task != null) {
                row = task
                return
            }
            // Not there any more (finished or deleted elsewhere since the list was made): on to the next.
            current++
        }
        end()
    }

    /**
     * A command for the task being looked at. When it has gone through, [then] carries on; when Taskwarrior asks something first, the
     * person is asked, and the same command is sent again with the answer.
     */
    private suspend fun write(words: List<String>, then: suspend () -> Unit, answers: Answers = Answers()) {
        val uuid = row!!.uuid
        try {
            val res = store.call(QUIET + uuid + words, answers)
            val r = res.result
            if (r is CliResult.Error) return say(r.value.message, bad = true)
            if (r is CliResult.Confirm) {
                asking = Asking(words, answers, r.value, 0, mutableListOf(), then)
                phase = ReviewPhase.Asking
                return askNext()
            }
            if (res.wrote) {
                store.written()
                store.live?.let { store.refresh(it) }
            }
            then()
        } catch (e: Exception) {
            say(e.message ?: e.toString(), bad = true)
        }
    }

    private fun end(summary: Boolean = true) {
        if (summary) say(reviewEnd(reviewed, total))
        asking = null
        // The card keeps the summary until it is closed.
        phase = ReviewPhase.Done
    }

    private fun say(text: String, bad: Boolean = false) {
        message = text
        messageBad = bad
    }
}
