package me.utsob.meghnaad.app

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.snapshotFlow
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.ui.Modifier
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import kotlinx.coroutines.delay
import me.utsob.meghnaad.SyncWorker
import me.utsob.meghnaad.Widgets
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private lateinit var engine: LocalEngine
    private lateinit var store: Store

    /** Who is waiting to hear whether the phone allowed notifications. */
    private var permissionAnswer: ((Boolean) -> Unit)? = null
    private val askNotifications = registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        permissionAnswer?.invoke(granted)
        permissionAnswer = null
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Draw under the system bars (the app pads for them), as a phone's apps do.
        enableEdgeToEdge()
        APP_VERSION = packageManager.getPackageInfo(packageName, 0).versionName ?: ""
        val vault = AppEngine.vault(this)
        engine = AppEngine.get(this)
        val sync = SyncController(engine, lifecycleScope, SystemClock)
        val reminders = AndroidReminders(this) { done ->
            permissionAnswer = done
            if (android.os.Build.VERSION.SDK_INT >= 33) askNotifications.launch(android.Manifest.permission.POST_NOTIFICATIONS) else done(true)
        }
        store = Store(Tw(engine, SystemClock), engine, vault, sync, lifecycleScope, SystemClock, AndroidPrefs(this), reminders, ActivityFiles(this))
        // What is written here is news to the widgets on the home screen.
        store.onWritten = { Widgets.refreshAll(applicationContext) }
        store.begin()
        openWhatWasTapped(intent)
        // The background sync runs while reminders are on or a widget is placed (the widgets start it themselves).
        lifecycleScope.launch {
            snapshotFlow { store.notifier.settings.enabled }.collect { on ->
                if (on) SyncWorker.schedule(applicationContext) else if (!Widgets.needsSync(applicationContext)) SyncWorker.cancel(applicationContext)
            }
        }
        // The back button closes what is on top (a question, a form, the menu), then goes back to the Tasks page, and only
        // then leaves the app.
        onBackPressedDispatcher.addCallback(
            this,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    if (!store.handleBack()) {
                        isEnabled = false
                        onBackPressedDispatcher.onBackPressed()
                    }
                }
            },
        )
        // While the app is in front: pull what other devices did when it comes back, and every half minute after.
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.STARTED) {
                // A widget may have changed tasks while the app was away, and the phone may have changed theme: draw both again.
                Widgets.redrawAll(applicationContext)
                store.reloadFromOutside()
                if (store.setup == Setup.Ready) sync.pullIfStale()
                while (true) {
                    delay(SyncController.EVERY_MILLIS)
                    if (store.setup == Setup.Ready) sync.request()
                }
            }
        }
        // Clear of the status bar, the gesture bar and the keyboard, so the prompt is never under one of them.
        setContent { Box(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) { App(store) } }
    }

    /** A notification was tapped while the app was open. */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        openWhatWasTapped(intent)
    }

    private fun openWhatWasTapped(intent: Intent?) {
        if (intent == null) return
        // A task's link (`meghnaad://task/1a2b3c4d`), from another app or a note.
        intent.data?.takeIf { intent.action == Intent.ACTION_VIEW }?.let {
            intent.data = null
            store.openFromLink(it.toString())
        }
        intent.getStringExtra(EXTRA_TASK)?.let {
            intent.removeExtra(EXTRA_TASK)
            store.openFromOutside(it)
        }
        // A day of the calendar widget: the tasks due or scheduled on it.
        intent.getStringExtra(EXTRA_REPORT)?.let {
            val filter = intent.getStringExtra(EXTRA_FILTER).orEmpty()
            intent.removeExtra(EXTRA_REPORT)
            intent.removeExtra(EXTRA_FILTER)
            store.openReport(it, filter)
        }
    }

    companion object {
        /** What a tap on a widget or a notification asks the app to open: a task (its uuid), or a report with a filter. */
        const val EXTRA_TASK = AndroidReminders.EXTRA_TASK
        const val EXTRA_REPORT = "report"
        const val EXTRA_FILTER = "filter"
    }

    override fun onStop() {
        super.onStop()
        engine.save()
    }
}
