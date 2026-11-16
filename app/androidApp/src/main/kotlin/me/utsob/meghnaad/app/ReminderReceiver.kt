package me.utsob.meghnaad.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

/**
 * Wakes at the time the last alarm was set for, and after the phone starts or the app is updated (alarms do not survive either):
 * shows what is due, and sets the next alarm. Works with the app closed.
 */
class ReminderReceiver : BroadcastReceiver() {
    private companion object {
        /** How long an alarm waits for a sync before it reminds from what the phone has. */
        const val SYNC_WAIT_MS = 6_000L
    }

    override fun onReceive(context: Context, intent: Intent) {
        val pending = goAsync()
        CoroutineScope(Dispatchers.Default).launch {
            try {
                val engine = AppEngine.get(context)
                val tw = Tw(engine, SystemClock)
                tw.start()
                // A task done on another device is still open in this phone's copy until it syncs: try a sync first, so it is not
                // announced. Briefly, and offline or failing it is the copy the phone has.
                val sync = CoroutineScope(Dispatchers.Default).async { runCatching { engine.sync() } }
                withTimeoutOrNull(SYNC_WAIT_MS) { sync.await() }
                runReminders(tw, AndroidPrefs(context), AndroidReminders(context), SystemClock)
            } catch (e: Exception) {
                Log.w("Reminders", "could not look at the tasks: ${e.message}")
            } finally {
                pending.finish()
            }
        }
    }
}
