package me.utsob.meghnaad.app

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.serializer
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

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

enum class View { Tasks, Projects, Tags, Summary, Calendar, Burndown, Console, Settings }

enum class SettingsTab { Taskrc, Tasksh, Commands, Hooks, Urgency, Sync }

/** The pages that show one command's answer with the same component the console uses. */
enum class Panel { Summary, Calendar, Burndown }

/** What is run: a typed command line, or pre-split arguments (no quoting concerns): exactly one of them. */
data class CliInput(val line: String? = null, val args: List<String>? = null)

/** One command and what came of it: a line of the console's scrollback, or the live table of the Tasks view. */
class Entry(val id: Int, input: CliInput, title: String) {
    var input by mutableStateOf(input)
    /** What was run, as it would be typed. */
    var title by mutableStateOf(title)
    var result by mutableStateOf<CliResult?>(null)
    var loading by mutableStateOf(false)
    /** A transport-level failure (not a command error, which arrives as an `error` result). */
    var failure by mutableStateOf<String?>(null)
    var at by mutableStateOf(0.0)
    /** What the hooks printed while this command ran. */
    var feedback by mutableStateOf<List<HookLine>>(emptyList())
    /** What was already answered on this command, for the questions that follow the first. */
    var answers by mutableStateOf(Answers())
}

class Toast(val text: String, val bad: Boolean = false)

/** A question put to the person, answered by a dialog: `yes` is what the engine's `confirmation` wants. */
class Ask(val message: String, val yes: String = "Yes", val no: String = "No", val answer: CompletableDeferred<Boolean> = CompletableDeferred())

class DetailRef(val uuid: String, val from: Entry?)

class EditRef(val row: Row, val from: Entry?)

/** The add-task form; [n] makes each ask a new one, so "add another" starts it again. */
class AddRef(val description: String, val n: Int)

/** Which rows of the Tasks list are ticked, in the order they were ticked: held here so the list, the bulk bar and the console see the same ones. */
class Selection {
    val uuids = mutableStateListOf<String>()

    val count get() = uuids.size

    fun has(uuid: String) = uuid in uuids

    fun toggle(uuid: String) {
        if (!uuids.remove(uuid)) uuids.add(uuid)
    }

    /** Tick all of these (or none of them, when they are all ticked already). */
    fun toggleAll(all: List<String>) {
        if (all.isNotEmpty() && all.all { has(it) }) uuids.removeAll(all.toSet()) else all.filter { !has(it) }.forEach { uuids.add(it) }
    }

    fun clear() = uuids.clear()

    /** Keep only the ones still [present]. */
    fun keep(present: Set<String>) {
        if (uuids.any { it !in present }) uuids.retainAll(present)
    }
}

const val HISTORY_MAX = 20

private const val HISTORY_KEY = "history"
private const val SHOW_HIDDEN_KEY = "show-hidden-reports"

/** Long uuids make the "last command" line unreadable; the first 8 characters resolve the same way. */
fun shortenUuids(s: String): String =
    Regex("\\b([0-9a-f]{8})-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\\b").replace(s) { it.groupValues[1] }

private fun titleOf(input: CliInput) = input.args?.let(::previewLine) ?: "task ${input.line ?: ""}".trim()

private fun hookText(lines: List<HookLine>) = lines.joinToString(" · ") { it.text }

/**
 * What the screens show and what they can ask: the counterpart of `web/src/lib/store.svelte.ts`. All Taskwarrior behaviour is
 * the engine's; this only keeps the answers, the state of the requests and what the person is looking at.
 */
class Store(
    private val tw: Tw,
    private val port: EnginePort,
    private val vault: Vault,
    val sync: SyncController,
    private val scope: CoroutineScope,
    private val clock: Clock,
    private val prefs: Prefs = MemoryPrefs(),
    reminders: ReminderHost? = null,
) {
    /** Reminders: the options, the running task, and what is due while the app is open (the phone's alarm does the rest). */
    val notifier = Notifier(this, tw, prefs, scope, clock, reminders)

    // ---- Getting started
    var setup by mutableStateOf(Setup.Checking)
        private set

    /** Why the engine could not start. */
    var error by mutableStateOf<String?>(null)
        private set

    // ---- Settings
    var config by mutableStateOf<ConfigResponse?>(null)
        private set
    var configError by mutableStateOf<String?>(null)
        private set

    /** The app's own `settings.json`, read (null until it has been). */
    var appSettings by mutableStateOf<ParsedSettings?>(null)
        private set

    val taskCommands: List<TaskCommand> get() = appSettings?.commands ?: emptyList()
    val reports: List<ReportMeta> get() = config?.reports ?: emptyList()

    /** Reports named `_something` are hidden helpers: left out of the lists to pick from unless this is on. */
    var showHiddenReports by mutableStateOf(prefs.get(SHOW_HIDDEN_KEY) == "1")
        private set

    val offeredReports: List<ReportMeta> get() = offeredReports(reports, showHiddenReports, report)

    /** Whether there are any to show or hide, so the switch is only there when it does something. */
    val hasHiddenReports: Boolean get() = reports.any { isHiddenReport(it.name) }

    val udas: List<UdaDef> get() = config?.config?.udas?.values?.toList() ?: emptyList()

    /** The taskrc's `confirmation` (on unless turned off): the delete buttons ask before they delete. */
    val confirmation: Boolean
        get() = config?.config?.settings?.get("confirmation")?.trim()?.let { Regex("^(1|y|yes|on|true)$", RegexOption.IGNORE_CASE).matches(it) } ?: true

    val localOnly get() = vault.localOnly

    // ---- What is on screen
    var view by mutableStateOf(View.Tasks)
    var settingsTab by mutableStateOf(SettingsTab.Taskrc)

    /** The report in focus, and the extra filter on it. Running a report in the console sets these too. */
    var report by mutableStateOf("next")
    var filter by mutableStateOf("")

    /** The live table of the Tasks view: the report currently in focus. */
    var live by mutableStateOf<Entry?>(null)
        private set

    /** The console's scrollback (the commands that are not reports). */
    val entries = mutableStateListOf<Entry>()
    val panels = mutableStateMapOf<Panel, Entry>()

    /** The filter on the Projects and Tags pages. They start as the pending tasks; taking that away brings in finished ones. */
    var projectsFilter by mutableStateOf("status:pending")
    var tagsFilter by mutableStateOf("status:pending")
    var summaryFilter by mutableStateOf("")
    var calendarFilter by mutableStateOf("")
    var burndownFilter by mutableStateOf("")
    var burndownPeriod by mutableStateOf("daily")

    /** The tag the Tags page should show (`n` changes on every request, so asking for the same tag again still reacts). */
    var tagFocus by mutableStateOf<Pair<String, Int>?>(null)
        private set
    var projectFocus by mutableStateOf<Pair<String, Int>?>(null)
        private set

    val selection = Selection()

    var detail by mutableStateOf<DetailRef?>(null)
    var editing by mutableStateOf<EditRef?>(null)
    /** tasksh's review, drawn as a card over the page while it is going. */
    val review = ReviewSession(this)
    var adding by mutableStateOf<AddRef?>(null)
    var notifyOpen by mutableStateOf(false)

    /** Whether the slide-over menu is open (the system's back button closes it). */
    var menuOpen by mutableStateOf(false)

    /** Text typed at the console line and not yet run; and a request for it to take this text and the focus. */
    var promptLine by mutableStateOf("")
    var promptRequest by mutableStateOf<Pair<String, Int>?>(null)
        private set

    // ---- Messages
    var toast by mutableStateOf<Toast?>(null)
        private set

    /** The question being asked, if any. */
    var ask by mutableStateOf<Ask?>(null)
        private set

    /** The most recent command run, whether typed or driven by the screens. Shown above the prompt. */
    var lastCommand by mutableStateOf("")
        private set

    /** The last [HISTORY_MAX] commands, oldest first; what the arrows walk. */
    var history by mutableStateOf(loadHistory())
        private set

    // ---- What completion knows
    var projects by mutableStateOf(emptyList<String>())
        private set
    var tags by mutableStateOf(emptyList<String>())
        private set
    var tasks by mutableStateOf(emptyList<TaskRef>())
        private set

    /** Now, in seconds, kept fresh every half minute so relative dates ("due in 2d") stay right without asking again. */
    var now by mutableStateOf(clock.nowSeconds().toLong())
        private set

    /** Bumped after every successful write, for views that show a single task. */
    var rev by mutableStateOf(0)
        private set

    val utcOffset get() = clock.utcOffsetSeconds()

    fun offsetAt(epoch: Long) = clock.offsetAt(epoch)

    val vocab: Vocab
        get() = Vocab(
            reports = reports.map { ReportName(it.name, it.description) },
            projects = projects,
            tags = tags,
            udas = udas,
            tasks = tasks,
            contexts = config?.config?.contexts?.keys?.toList() ?: emptyList(),
        )

    private var nextId = 1
    private var toastJob: Job? = null
    private var lastWasLive = false

    init {
        // What another device did arrives with a sync: draw again what is on screen.
        sync.onChanged = {
            written()
            live?.let { e -> scope.launch { exec(e) } }
        }
        // The device took settings or hooks from the bucket (it keeps them in step when asked to): read them again.
        sync.onSettingsChanged = { scope.launch { settingsChanged(); loadAppSettings() } }
    }

    /**
     * Run something for the app, not for a screen: a command from the prompt changes the page (and so removes the prompt that
     * sent it), and a tick on a card removes the card, neither of which may cancel what they started.
     */
    fun go(block: suspend Store.() -> Unit): Job = scope.launch { block() }

    /** Now in seconds, fresh (not the half-minute value of [now]): for a timer. */
    fun clockNow(): Long = clock.nowSeconds().toLong()

    fun tick() {
        now = clock.nowSeconds().toLong()
    }

    // ---- Getting started

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
        connectionVersion++
        // Connecting from Settings (the engine is running): keep going. From the first screen: start.
        if (setup == Setup.Ready) written() else boot()
        return null
    }

    /** Go on without a bucket: the tasks stay on this device, and a connection can be made later in Settings. */
    fun useLocally() {
        vault.localOnly = true
        boot()
    }

    /** Ask first: throwing the copy away loses any change that has not reached the bucket yet. */
    fun requestStartOver() {
        scope.launch {
            val sure = confirmAsk(
                "This throws away this phone's copy of your tasks, including any change that has not reached the server yet, and loads them again from the server. Continue?",
                yes = "Start over",
                no = "Cancel",
            )
            if (sure) startOver()
        }
    }

    /** Throw the copy kept on this device away and load the tasks again from the bucket (for when the two disagree). */
    fun startOver() {
        scope.launch {
            try {
                port.forget()
                sync.request().await()
                written()
                live?.let { exec(it) }
            } catch (e: Exception) {
                notify(e.message ?: e.toString(), bad = true)
            }
        }
    }

    private fun boot() {
        setup = Setup.Starting
        scope.launch {
            try {
                loadConfig(first = true)
                // With nothing kept here the tasks come from the bucket before the first command is answered; an unreachable
                // bucket is no reason to stop: the app starts empty and says why it has not synced.
                if (port.restored) sync.request() else sync.request().await()
                setup = Setup.Ready
                loadAppSettings()
                notifier.start()
                launchTask?.let { launchTask = null; openDetail(it) }
            } catch (e: Exception) {
                error = e.message ?: e.toString()
                setup = Setup.Failed
            }
        }
    }

    // ---- Settings

    /** Read the settings (and give them to the engine): it cannot run a command before this. */
    suspend fun loadConfig(first: Boolean = false) {
        try {
            config = tw.start()
            configError = null
            if (!first) refreshNames() else scope.launch { refreshNames() }
        } catch (e: Exception) {
            configError = e.message ?: e.toString()
            if (first) throw e
        }
    }

    /** Read `settings.json`. A failure leaves what was there: the commands are a convenience, not needed to work. */
    suspend fun loadAppSettings() {
        try {
            val (status, body) = tw.request("GET", "/api/settings")
            appSettings = parseSettings(if (status in 200..299) body else "")
        } catch (e: Exception) {
            if (appSettings == null) appSettings = parseSettings("")
        }
    }

    /**
     * The saved settings changed (the taskrc page, `config`): read them again, and draw again what is on screen, since they
     * can change what it shows (colours, a UDA, a report, `dateformat`).
     */
    suspend fun settingsChanged() {
        loadConfig()
        live?.let { exec(it) }
        rev++
    }

    fun showHidden(on: Boolean) {
        showHiddenReports = on
        prefs.put(SHOW_HIDDEN_KEY, if (on) "1" else "0")
    }

    // ---- Names for completion

    /**
     * The project and tag names offered for completion come from the tasks on screen, which is only some of them. Ask for all of
     * them: `_names` is `_tags` and `_projects` in one request (the lists follow `complete.all.tags` and `list.all.projects`).
     */
    suspend fun refreshNames() {
        val value = try {
            (tw.run(args = listOf("_names")).result as? CliResult.Json)?.value as? JsonObject
        } catch (e: Exception) {
            null
        } ?: return
        fun names(x: kotlinx.serialization.json.JsonElement?) =
            (x as? JsonArray)?.mapNotNull { (it as? JsonPrimitive)?.takeIf { p -> p.isString }?.content } ?: emptyList()
        val newTags = names(value["tags"])
        val newProjects = names(value["projects"])
        if (newTags.isEmpty() && newProjects.isEmpty()) return
        val virtual = VIRTUAL_TAGS.toSet()
        tags = (tags + newTags.filter { it !in virtual }).toSet().sorted()
        val all = projects.toMutableSet()
        for (p in newProjects) {
            val parts = p.split('.')
            for (i in 1..parts.size) all += parts.take(i).joinToString(".")
        }
        projects = all.sorted()
    }

    /** Learn project, tag and task names from rows we have seen, for completion and helpers. */
    private fun learn(rows: List<Row>) {
        val ps = projects.toMutableSet()
        val ts = tags.toMutableSet()
        val known = tasks.associateBy { it.uuid }.toMutableMap()
        for (r in rows) {
            r.project?.let { p ->
                // Offer every ancestor too: Home.Kitchen implies Home.
                val parts = p.split('.')
                for (i in 1..parts.size) ps += parts.take(i).joinToString(".")
            }
            ts += r.tags
            known[r.uuid] = TaskRef(r.id, r.uuid, r.description)
        }
        projects = ps.sorted()
        tags = ts.sorted()
        tasks = known.values.toList()
    }

    // ---- History and messages

    /**
     * Record a command as "last run" and in the history. Live-table updates (typing a filter) are [replaceable]: each replaces the
     * previous one, so half-typed filters do not flood the 20 slots.
     */
    fun remember(cmd: String, replaceable: Boolean = false) {
        val text = cmd.replace(Regex("^task\\s+"), "").trim()
        if (text.isEmpty()) return
        lastCommand = shortenUuids("task $text")
        // Only a live update replaces the previous live update; a real command keeps it.
        val base = if (replaceable && lastWasLive) history.dropLast(1) else history
        history = (base.filter { it != text } + text).takeLast(HISTORY_MAX)
        lastWasLive = replaceable
        saveHistory()
    }

    /**
     * A refused `config` line may have carried a secret. Keep the command and the name, drop the value from everywhere the app
     * remembers it: the scrollback, the "last command" line and the saved history.
     */
    private fun redact(e: Entry) {
        val typed = e.input.line ?: previewLine(e.input.args ?: emptyList())
        val text = typed.replace(Regex("^task\\s+"), "").trim()
        val words = text.split(Regex("\\s+"))
        val at = words.indexOf("config")
        val safe = if (at < 0 || words.size <= at + 2) text else (words.take(at + 2) + "…").joinToString(" ")
        e.title = "task $safe"
        e.input = CliInput(line = safe)
        lastCommand = e.title
        history = history.map { if (it == text) safe else it }
        saveHistory()
    }

    private fun loadHistory(): List<String> = try {
        engineJson.decodeFromString(ListSerializer(String.serializer()), prefs.get(HISTORY_KEY) ?: "[]").takeLast(HISTORY_MAX)
    } catch (e: Exception) {
        emptyList()
    }

    private fun saveHistory() = prefs.put(HISTORY_KEY, engineJson.encodeToString(ListSerializer(String.serializer()), history))

    fun notify(text: String, bad: Boolean = false) {
        val t = Toast(text, bad)
        toast = t
        toastJob?.cancel()
        toastJob = scope.launch {
            delay(if (bad) 7000 else 3500)
            if (toast === t) toast = null
        }
    }

    fun dismissToast() {
        toast = null
    }

    /** Put a question to the person and wait for the answer. */
    suspend fun confirmAsk(message: String, yes: String = "Yes", no: String = "No"): Boolean {
        val a = Ask(message, yes, no)
        ask = a
        val answer = a.answer.await()
        if (ask === a) ask = null
        return answer
    }

    fun answerAsk(yes: Boolean) {
        ask?.answer?.complete(yes)
    }

    // ---- Moving about

    /** Open the Settings page, on one of its tabs. */
    fun openSettings(tab: SettingsTab = settingsTab) {
        settingsTab = tab
        view = View.Settings
    }

    /** Show a tag's entry on the Tags page: what a tag chip does wherever it is not toggling a report's filter. */
    fun showTag(tag: String) {
        tagFocus = tag to ((tagFocus?.second ?: 0) + 1)
        detail = null
        view = View.Tags
    }

    /** Show a project's entry on the Projects page (a sub-project opens its parents too). */
    fun showProject(project: String) {
        projectFocus = project to ((projectFocus?.second ?: 0) + 1)
        detail = null
        view = View.Projects
    }

    /** The page took what a chip asked for: coming back to it later should not repeat it. */
    fun focusHandled() {
        tagFocus = null
        projectFocus = null
    }

    /** A task to open once the app has started (a notification was tapped while it was not running). */
    private var launchTask: String? = null

    /** Open a task from outside (a notification): at once, or when the app is ready. */
    fun openFromOutside(uuid: String) {
        if (setup == Setup.Ready) openDetail(uuid) else launchTask = uuid
    }

    fun openDetail(uuid: String, from: Entry? = null) {
        detail = DetailRef(uuid, from)
    }

    /** Put [text] at the console line, with the cursor after it, ready to be carried on. */
    fun requestPrompt(text: String) {
        promptRequest = text to ((promptRequest?.second ?: 0) + 1)
    }

    fun clearEntries() = entries.clear()

    /**
     * The system's back button, in the order a person expects: close what is on top (a question, a form, the menu), then ends
     * a selection, then goes back to the Tasks page. False when there is nothing to go back from: the app may close.
     */
    fun handleBack(): Boolean {
        when {
            ask != null -> answerAsk(false)
            adding != null -> adding = null
            editing != null -> closeEditor()
            detail != null -> detail = null
            review.phase != ReviewPhase.Idle -> review.quit()
            notifyOpen -> notifyOpen = false
            menuOpen -> menuOpen = false
            selection.count > 0 -> selection.clear()
            view != View.Tasks -> view = View.Tasks
            else -> return false
        }
        return true
    }

    /** Pull down on a page: sync now, then draw again what is on screen (a failure is told, as the banner under the bar says why too). */
    suspend fun pullRefresh() {
        val result = sync.request().await()
        result.exceptionOrNull()?.let { notify("Couldn't sync: ${sync.error ?: it.message}", bad = true) }
        written()
        live?.let { exec(it) }
    }

    // ---- Running commands

    private suspend fun exec(e: Entry, answers: Answers = Answers()): CliResponse? {
        e.loading = true
        e.failure = null
        e.answers = answers
        return try {
            val res = tw.run(e.input.line, e.input.args, answers)
            e.result = res.result
            e.feedback = res.feedback
            when (val r = res.result) {
                is CliResult.Report -> learn(r.value.rows)
                is CliResult.Info -> learn(r.value.tasks)
                else -> Unit
            }
            if (res.wrote) {
                written()
                sync.pushSoon()
            }
            res
        } catch (err: Exception) {
            e.failure = err.message ?: err.toString()
            null
        } finally {
            e.loading = false
            e.at = clock.nowSeconds()
        }
    }

    /** Make [name] and [words] the report in focus and show it in the Tasks view. */
    fun focusReport(name: String, words: List<String>, from: Entry? = null) {
        report = name
        filter = joinWords(words)
        view = View.Tasks
        if (from != null) {
            // Hand over an already-fetched result instead of fetching it again. Only then is the list already showing these
            // arguments; otherwise the Tasks view must run them (marking them as loaded here would make it skip the run
            // and keep showing the previous, unfiltered, list).
            val args = reportArgs(filter, report)
            liveKey = engineJson.encodeToString(ListSerializer(String.serializer()), args)
            live = Entry(nextId++, CliInput(args = args), titleOf(CliInput(args = args))).also {
                it.result = from.result
                it.feedback = from.feedback
                it.at = from.at
            }
        }
    }

    /** The arguments the live table was loaded with, so the same thing is not fetched twice. */
    var liveKey = ""
        private set

    /** Print something into the console that is not a command's answer (a review's banner and messages). */
    fun addEntry(title: String, result: CliResult): Entry {
        val e = Entry(nextId++, CliInput(args = emptyList()), title)
        e.result = result
        e.at = clock.nowSeconds()
        entries.add(e)
        return e
    }

    /**
     * Run a command from the console. A report command focuses that report instead of scrolling. [typed] is what was typed when
     * [input] is that with a task command written out: the history keeps it.
     */
    suspend fun run(input: CliInput, typed: String? = null): Entry {
        // Typed in the Console, a report prints right there. From the bar under the other pages it opens in the Tasks view
        // instead, where it can be sorted and filtered.
        val inConsole = view == View.Console
        val e = Entry(nextId++, input, titleOf(input))
        e.loading = true
        entries.add(e)
        remember(typed ?: input.line ?: previewLine(input.args ?: emptyList()))
        val res = exec(e)
        // `config` and `context` rewrite the saved settings: pick them up so the pages follow.
        val name = res?.command?.name
        if (name == "config" || name == "context") {
            if (res.result !is CliResult.Error) settingsChanged() else if (name == "config") redact(e)
        }
        val command = res?.command
        if (command != null && command.report && res.result is CliResult.Report && !inConsole) {
            entries.removeAll { it.id == e.id }
            focusReport(command.name, command.filter, e)
            // The entry that would have shown them is gone.
            if (res.feedback.isNotEmpty()) notify(hookText(res.feedback), bad = res.feedback.any { it.kind == "warn" })
        } else if (res?.wrote == true) {
            // A write from the console changed what the focused report shows.
            live?.let { exec(it) }
        }
        return e
    }

    /** Run (or run again) the Tasks view's live table. */
    suspend fun runLive(input: CliInput) {
        liveKey = engineJson.encodeToString(ListSerializer(String.serializer()), input.args ?: listOf(input.line ?: ""))
        val e = live ?: Entry(nextId++, input, titleOf(input)).also {
            it.loading = true
            live = it
        }
        e.input = input
        e.title = titleOf(input)
        remember(input.args?.let(::previewLine) ?: input.line ?: "", replaceable = true)
        exec(e)
    }

    /** Run (or run again) what one of the Summary, Calendar or Burndown pages shows. */
    suspend fun runPanel(which: Panel, input: CliInput) {
        val e = panels[which] ?: Entry(nextId++, input, titleOf(input)).also {
            it.loading = true
            panels[which] = it
        }
        e.input = input
        e.title = titleOf(input)
        exec(e)
    }

    /** Run an entry again (after a change), keeping its place. */
    suspend fun refresh(e: Entry) {
        exec(e)
    }

    /** Answer a plain yes/no question on an entry. */
    suspend fun confirm(e: Entry, yes: Boolean) {
        if (!yes) {
            e.result = CliResult.Text(TextResult(listOf("Cancelled.")))
            return
        }
        val res = exec(e, e.answers.copy(confirmed = true))
        if (res?.wrote == true) live?.let { exec(it) }
    }

    /** Answer a per-task question on an entry with the keys that were ticked. */
    suspend fun answerItems(e: Entry, keys: List<String>) {
        val r = (e.result as? CliResult.Confirm)?.value ?: return
        val more = if (r.ask == "permission") e.answers.copy(approved = keys) else e.answers.copy(extras = keys)
        val res = exec(e, more)
        if (res?.wrote == true) live?.let { exec(it) }
    }

    /** Something was written: let the views reload, and refresh the completion names if they depend on it. */
    fun written() {
        rev++
        scope.launch { refreshNames() }
        if (setup == Setup.Ready) notifier.pollSoon()
    }

    /** A screen's action (done, start, add, …): run it, say how it went, and draw again what is on screen. */
    suspend fun act(from: Entry?, args: List<String>, answers: Answers = Answers(), again: Boolean = false): CliResponse? {
        if (!again) remember(previewLine(args))
        return try {
            val res = tw.run(args = args, answers = answers)
            val r = res.result
            // What a hook said goes with the outcome, in the same message.
            val said = if (res.feedback.isNotEmpty()) " ${hookText(res.feedback)}" else ""
            val bad = res.feedback.any { it.kind == "warn" }
            when (r) {
                is CliResult.Error -> notify(r.value.message + said, bad = true)
                is CliResult.Changed -> notify(r.value.message + said, bad = bad)
                is CliResult.Text -> notify(r.value.lines.joinToString(" ") + said)
                is CliResult.Confirm -> {
                    // Ask, then repeat the command with the answer. A screen's action touches one task, so this is one
                    // question; a longer list (rare) is answered as a whole.
                    val q = r.value
                    if (q.ask == "plain") {
                        if (!answers.confirmed && confirmAsk(q.message)) return act(from, args, answers.copy(confirmed = true), true)
                    } else {
                        val keys = q.items.map { it.key }
                        val question = if (q.items.size == 1) q.items[0].question else q.message
                        if (confirmAsk(question)) {
                            return act(from, args, if (q.ask == "permission") answers.copy(approved = keys) else answers.copy(extras = keys), true)
                        }
                        // A follow-up can be declined while the change itself still goes ahead.
                        if (q.ask == "extras" && confirmAsk("Go ahead with the change without that?")) {
                            return act(from, args, answers.copy(extras = emptyList()), true)
                        }
                    }
                    notify("Nothing was changed.")
                }
                else -> Unit
            }
            if (res.wrote) {
                written()
                sync.pushSoon()
                listOfNotNull(from, live).distinct().forEach { refresh(it) }
            }
            res
        } catch (err: Exception) {
            notify(err.message ?: err.toString(), bad = true)
            null
        }
    }

    /** An import of a file of tasks: without [apply] it only reports what would happen; with it the tasks are written. */
    suspend fun importTasks(text: String, apply: Boolean): JsonObject {
        val report = tw.import(text, apply)
        if (apply) {
            written()
            sync.pushSoon()
        }
        return report
    }

    /** The first screen of the Tasks view needs the live table: run it again after a settings or sync change. */
    fun redrawLive() {
        live?.let { e -> scope.launch { exec(e) } }
    }

    // What was fetched, kept until the tasks change, so moving along a row of ids asks once each (lookup.ts).
    private val taskCache = mutableMapOf<String, CompletableDeferred<Row?>>()
    private var cachedRev = -1

    /** One of the settings routes (`/api/settings`, `/api/config/taskrc`, …): status and body; a failure's own words are thrown. */
    suspend fun request(method: String, path: String, body: String = ""): String {
        val (status, text) = tw.request(method, path, body)
        if (status !in 200..299) {
            val why = try {
                engineJson.parseToJsonElement(text).jsonObject["error"]?.jsonPrimitive?.content
            } catch (e: Exception) {
                null
            }
            throw EngineError(why ?: "Server error ($status).", status)
        }
        return text
    }

    // ---- The connection, for Settings

    /** Whether this device is connected to a bucket. */
    val connected: Boolean get() = connectionVersion >= 0 && vault.connection() != null

    private var connectionVersion by mutableStateOf(0)

    /** Which of the settings and the hooks are kept in step with the bucket's. */
    var syncOptions: SyncOptions
        get() {
            connectionVersion
            return vault.syncOptions
        }
        set(value) {
            vault.syncOptions = value
            connectionVersion++
        }

    /** Stop syncing: the tasks stay on this device, and the bucket keeps what it has. */
    fun disconnect() {
        vault.clearConnection()
        vault.localOnly = true
        connectionVersion++
    }

    /** One task by uuid, or null when it is gone or the request failed. Asked for only when someone wants to see it. */
    suspend fun fetchTask(uuid: String): Row? {
        if (cachedRev != rev) {
            taskCache.clear()
            cachedRev = rev
        }
        val pending = taskCache.getOrPut(uuid) {
            CompletableDeferred<Row?>().also { d ->
                scope.launch {
                    d.complete(
                        try {
                            (tw.run(args = listOf(uuid, "info")).result as? CliResult.Info)?.value?.tasks?.firstOrNull()
                        } catch (e: Exception) {
                            null
                        },
                    )
                }
            }
        }
        return pending.await()
    }

    /** A command as the engine answers it, whatever it is (a question, an error): for a screen that handles those itself. */
    suspend fun call(args: List<String>, answers: Answers = Answers()): CliResponse = tw.run(args = args, answers = answers)

    /** Close the task editor; a review waiting on it (the Edit choice) carries on. */
    fun closeEditor() {
        editing = null
        if (review.phase == ReviewPhase.Editing) go { review.editClosed() }
    }

    /** What a command answers, for a screen that asks and reads (a task's history, its instances): errors are thrown. */
    suspend fun query(args: List<String>): CliResult {
        val result = tw.run(args = args).result
        if (result is CliResult.Error) throw EngineError(result.value.message)
        return result
    }

    /** The task, by its uuid prefix of at least 8 characters (what a widget tap or a link carries). */
    suspend fun openFromPrefix(prefix: String) {
        val found = try {
            ((tw.run(args = listOf("uuid:$prefix", "uuids")).result as? CliResult.Text)?.value?.lines ?: emptyList())
                .joinToString(" ").split(Regex("\\s+")).filter { it.isNotEmpty() }
        } catch (e: Exception) {
            notify(e.message ?: e.toString(), bad = true)
            emptyList()
        }
        if (found.size == 1) {
            detail = DetailRef(found[0], null)
        } else {
            detail = null
            notify(if (found.isNotEmpty()) "More than one task starts with $prefix." else "No task starts with $prefix.", bad = true)
        }
    }

}
