package me.utsob.meghnaad.app

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Job
import kotlinx.coroutines.async
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

enum class SyncStatus { Idle, Syncing, Failed }

/**
 * Keeps the device in step with the bucket, by the web client's rules (web/src/engine/client.svelte.ts): a sync is pull and
 * push in one; one asked for while another runs is run once more when that one is done; a write is pushed in the
 * background a moment after the last of a run of writes ([pushAfterMillis]), or at once when the app goes to the background
 * ([pushNow]), and a push that fails is only tried again later, since the write is already applied and kept here.
 * Use it from one thread (the UI's): the engine call itself runs elsewhere.
 */
class SyncController(
    private val port: EnginePort,
    private val scope: CoroutineScope,
    private val clock: Clock,
    private val pushAfterMillis: Long = PUSH_AFTER_MILLIS,
) {
    var status by mutableStateOf(SyncStatus.Idle)
        private set

    /** Why the last sync failed, until one succeeds. */
    var error by mutableStateOf<String?>(null)
        private set

    /** When the last sync succeeded (seconds since 1970). */
    var lastSync by mutableStateOf<Double?>(null)
        private set

    /** Called when a sync brought in changes, so what is on screen can be drawn again. */
    var onChanged: () -> Unit = {}

    /** Called when a sync replaced the settings or the hooks with the bucket's. */
    var onSettingsChanged: () -> Unit = {}

    private var running: Deferred<Result<Unit>>? = null

    /** A push waiting for the writes to stop. */
    private var pushJob: Job? = null

    /** What to do with the tasks on the device if the next sync finds they were synced somewhere else; used once. */
    private var pendingSwitch: String? = null
    private var again = false
    private var lastPull = Double.NEGATIVE_INFINITY

    /** Pull and push now, or after the one under way. The result says whether it worked, for a caller who wants to know. */
    fun request(switch: String? = null): Deferred<Result<Unit>> {
        if (switch != null) pendingSwitch = switch
        running?.let {
            again = true
            return it
        }
        val job = scope.async { run() }
        running = job
        job.invokeOnCompletion {
            running = null
            if (again) {
                again = false
                request()
            }
        }
        return job
    }

    /** After a write: push it a moment later (once for a run of writes), without making the caller wait. */
    fun pushSoon() {
        pushJob?.cancel()
        pushJob = scope.launch {
            delay(pushAfterMillis)
            pushJob = null
            request()
        }
    }

    /** Push what is waiting now, if anything is: the app is going to the background. */
    fun pushNow() {
        val waiting = pushJob ?: return
        waiting.cancel()
        pushJob = null
        request()
    }

    /** When the app is looked at again: pull unless that was done a moment ago. */
    fun pullIfStale() {
        if (clock.nowSeconds() - lastPull > NOT_SOONER_SECONDS) request()
    }

    private suspend fun run(): Result<Unit> {
        // This sync pushes whatever is waiting, so a push that was to follow it has nothing left to do.
        pushJob?.cancel()
        pushJob = null
        status = SyncStatus.Syncing
        return try {
            val switch = pendingSwitch
            pendingSwitch = null
            val done = port.sync(switch)
            error = null
            status = SyncStatus.Idle
            lastSync = clock.nowSeconds()
            lastPull = lastSync!!
            if (done.changed) onChanged()
            if (done.settings.isNotEmpty()) onSettingsChanged()
            Result.success(Unit)
        } catch (e: Exception) {
            // TaskChampion's own prefix adds nothing to "could not reach the server".
            error = (e.message ?: e.toString()).removePrefix("Server Error: ")
            status = SyncStatus.Failed
            Result.failure(e)
        }
    }

    companion object {
        /** How often a visible app pulls what other devices have done. */
        const val EVERY_MILLIS = 30_000L

        /** A pull that comes this soon after the last one is not worth a request. */
        const val NOT_SOONER_SECONDS = 4.0

        /** How long after the last of a run of writes they are pushed. */
        const val PUSH_AFTER_MILLIS = 2_000L
    }
}
