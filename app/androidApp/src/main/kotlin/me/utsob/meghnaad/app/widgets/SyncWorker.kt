package me.utsob.meghnaad.app.widgets

import me.utsob.meghnaad.app.SyncFailed
import android.content.Context
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import kotlinx.coroutines.runBlocking
import me.utsob.meghnaad.app.AndroidPrefs
import me.utsob.meghnaad.app.AndroidReminders
import me.utsob.meghnaad.app.AppEngine
import me.utsob.meghnaad.app.SystemClock
import me.utsob.meghnaad.app.Tw
import me.utsob.meghnaad.app.runReminders
import java.util.concurrent.TimeUnit

/** Pulls and pushes in the background, and redraws the widget. Does nothing until the phone is connected. */
class SyncWorker(context: Context, params: WorkerParameters) : Worker(context, params) {
    override fun doWork(): Result {
        val engine = Engines.get(applicationContext)
        if (engine.vault.connection() == null) return Result.success()
        return try {
            engine.sync()
            Widgets.refreshAll(applicationContext)
            // What came from the bucket may change what is due: look again and set the next alarm.
            runBlocking { runReminders(Tw(AppEngine.get(applicationContext), SystemClock), AndroidPrefs(applicationContext), AndroidReminders(applicationContext), SystemClock) }
            Result.success()
        } catch (e: SyncFailed) {
            // Offline, or the bucket is unreachable: try again later, with the system's backoff.
            Result.retry()
        }
    }

    companion object {
        private val online = Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build()

        /** Every half hour while a widget is on the home screen. */
        fun schedule(context: Context) {
            val work = PeriodicWorkRequestBuilder<SyncWorker>(30, TimeUnit.MINUTES).setConstraints(online).build()
            WorkManager.getInstance(context)
                .enqueueUniquePeriodicWork("periodic-sync", ExistingPeriodicWorkPolicy.KEEP, work)
        }

        fun cancel(context: Context) {
            WorkManager.getInstance(context).cancelUniqueWork("periodic-sync")
        }

        /** As soon as there is a network: after a change made from the widget, or a tap on its sync button. */
        fun runNow(context: Context) {
            val work = OneTimeWorkRequestBuilder<SyncWorker>().setConstraints(online).build()
            WorkManager.getInstance(context).enqueueUniqueWork("sync-now", ExistingWorkPolicy.REPLACE, work)
        }
    }
}
