package me.utsob.meghnaad.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

/**
 * Wakes at the time the last alarm was set for, and after the phone starts or the app is updated (alarms do not survive either):
 * shows what is due, and sets the next alarm. Works with the app closed.
 */
class ReminderReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val pending = goAsync()
        CoroutineScope(Dispatchers.Default).launch {
            try {
                val tw = Tw(AppEngine.get(context), SystemClock)
                tw.start()
                runReminders(tw, AndroidPrefs(context), AndroidReminders(context), SystemClock)
            } catch (e: Exception) {
                Log.w("Reminders", "could not look at the tasks: ${e.message}")
            } finally {
                pending.finish()
            }
        }
    }
}
