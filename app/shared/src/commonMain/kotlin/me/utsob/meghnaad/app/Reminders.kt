package me.utsob.meghnaad.app

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.builtins.MapSerializer
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.serializer

// Reminders (web/src/lib/notifier.svelte.ts). The web only works while the tab is open; on a phone an alarm keeps them: after every
// change the next moment something is worth saying is worked out (`nextTrigger`) and the system is asked to wake the app then,
// even if it is closed. The options are the web's, kept per device.

private const val SETTINGS_KEY = "tw-notify"
private const val SEEN_KEY = "tw-notified"

/** What the phone does for reminders that the app cannot do by itself: show a notification, and wake the app at a time. */
interface ReminderHost {
    /** Whether notifications may be shown now. */
    val allowed: Boolean

    /** Ask to be allowed (a phone asks once, then it is a setting); [done] is told what the answer was. */
    fun requestPermission(done: (Boolean) -> Unit)

    /** Show a notification, which opens the task when tapped. */
    fun show(notice: Notice)

    /** Wake the app at this time (seconds since 1970) to look at the tasks again; null cancels. */
    fun scheduleAt(epochSeconds: Long?)
}

fun loadSettings(prefs: Prefs): NotifySettings = try {
    prefs.get(SETTINGS_KEY)?.let { engineJson.decodeFromString(NotifySettings.serializer(), it) } ?: NotifySettings()
} catch (_: Exception) {
    NotifySettings()
}

fun saveSettings(prefs: Prefs, s: NotifySettings) = prefs.put(SETTINGS_KEY, engineJson.encodeToString(NotifySettings.serializer(), s))

private val seenSerializer = MapSerializer(String.serializer(), Long.serializer())

/** The keys of the notices shown, and when. */
fun loadSeen(prefs: Prefs): Map<String, Long> = try {
    prefs.get(SEEN_KEY)?.let { engineJson.decodeFromString(seenSerializer, it) } ?: emptyMap()
} catch (_: Exception) {
    emptyMap()
}

private fun saveSeen(prefs: Prefs, seen: Map<String, Long>) = prefs.put(SEEN_KEY, engineJson.encodeToString(seenSerializer, seen))

/** Tasks with a due or wait date, plus anything running: `_remind` sends only what is read here. */
suspend fun fetchReminders(tw: Tw): List<Reminder> =
    when (val r = tw.run(args = listOf("(", "due.any:", "or", "wait.any:", "or", "+ACTIVE", ")", "status:pending", "_remind")).result) {
        is CliResult.Json -> engineJson.decodeFromJsonElement(ListSerializer(Reminder.serializer()), r.value)
        is CliResult.Error -> throw EngineError(r.value.message)
        else -> emptyList()
    }

/**
 * What an alarm does, with or without the app on screen: show what is due now, and set the next alarm. Used by the receiver, so
 * the engine must be running (`Tw.start`).
 */
suspend fun runReminders(tw: Tw, prefs: Prefs, host: ReminderHost, clock: Clock) {
    val s = loadSettings(prefs)
    if (!s.enabled) return host.scheduleAt(null)
    val rows = fetchReminders(tw)
    val now = clock.nowSeconds().toLong()
    val tz = clock.utcOffsetSeconds()
    var seen = loadSeen(prefs)
    val notices = noticesToPost(rows, now, s, seen.keys, tz)
    if (notices.isNotEmpty()) {
        for (n in notices) {
            // A notification the phone would not show is not marked as shown, and does not stop the next alarm being set.
            val shown = !host.allowed || try {
                host.show(n)
                true
            } catch (_: Exception) {
                false
            }
            if (shown) seen = seen + (n.key to now)
        }
        seen = prune(seen, now)
        saveSeen(prefs, seen)
    }
    host.scheduleAt(nextTrigger(rows, now, s, seen.keys, tz))
}

/** The page's own side: the options, the tasks that are running (for the timer), and telling you while the app is open. */
class Notifier(
    private val store: Store,
    private val tw: Tw,
    private val prefs: Prefs,
    private val scope: CoroutineScope,
    private val clock: Clock,
    private val host: ReminderHost?,
) {
    var settings by mutableStateOf(loadSettings(prefs))
        private set

    /** Tasks that are running right now. */
    var active by mutableStateOf<List<Reminder>>(emptyList())
        private set

    /** When [active] was fetched (seconds), to extend tracked time between polls. */
    var polledAt by mutableStateOf(clock.nowSeconds().toLong())
        private set

    /** Set when the last poll failed (shown in the settings dialog, never as a popup). */
    var lastError by mutableStateOf<String?>(null)
        private set

    /** Whether this device can show system notifications at all, and whether it may now. */
    val hasSystem get() = host != null
    val systemAllowed get() = host?.allowed == true

    private var rows = emptyList<Reminder>()
    private var started = false
    private var pollJob: Job? = null

    private fun save() = saveSettings(prefs, settings)

    /** Turn reminders on, after asking the phone to allow notifications; refusing still leaves the alerts inside the app. */
    fun enable() {
        val h = host
        if (h != null && !h.allowed) h.requestPermission { turnOn() } else turnOn()
    }

    private fun turnOn() {
        settings = settings.copy(enabled = true)
        save()
        // Look at fresh data now: someone who just turned reminders on expects to hear about what is already due.
        pollSoon()
    }

    fun disable() {
        settings = settings.copy(enabled = false)
        save()
        host?.scheduleAt(null)
    }

    fun update(s: NotifySettings) {
        settings = s
        save()
        reschedule()
    }

    /** The tasks changed (written here, or by a sync): look again. */
    fun pollSoon() {
        pollJob?.cancel()
        pollJob = scope.launch { poll() }
    }

    suspend fun poll() {
        try {
            rows = fetchReminders(tw)
            active = rows.filter { it.start != null }
            polledAt = clock.nowSeconds().toLong()
            lastError = null
            check()
            reschedule()
        } catch (e: Exception) {
            lastError = e.message ?: e.toString()
        }
    }

    /** Tell what is due now, in the app (the alarm tells the phone when the app is not on screen). */
    fun check() {
        if (!settings.enabled) return
        val now = clock.nowSeconds().toLong()
        var seen = loadSeen(prefs)
        val notices = dueNotices(rows, now, settings, seen.keys, clock.utcOffsetSeconds())
        if (notices.isEmpty()) return
        for (n in notices) {
            seen = seen + (n.key to now)
            store.notify("${n.title}: ${n.body}")
        }
        saveSeen(prefs, prune(seen, now))
    }

    /** Set the alarm for the next notice (or cancel it, with reminders off). */
    private fun reschedule() {
        val h = host ?: return
        val now = clock.nowSeconds().toLong()
        h.scheduleAt(if (settings.enabled) nextTrigger(rows, now, settings, loadSeen(prefs).keys, clock.utcOffsetSeconds()) else null)
    }

    fun test() {
        val n = Notice("test:${clock.nowSeconds().toLong()}", NoticeKind.Due, "Test reminder", "Reminders work, with the app open or closed.", null)
        store.notify("${n.title}: ${n.body}")
        if (host?.allowed == true) host.show(n)
    }

    /** Begin polling and checking: once the app is running. */
    fun start() {
        if (started) return
        started = true
        scope.launch {
            while (true) {
                poll()
                // Dates pass between polls; look again at the picture already had much more often.
                repeat(4) {
                    delay(15_000)
                    check()
                }
            }
        }
    }
}
