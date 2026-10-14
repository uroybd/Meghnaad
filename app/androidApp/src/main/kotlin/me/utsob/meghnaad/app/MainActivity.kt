package me.utsob.meghnaad.app

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.ui.Modifier
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import kotlinx.coroutines.delay
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
        store = Store(Tw(engine, SystemClock), engine, vault, sync, lifecycleScope, SystemClock, AndroidPrefs(this), reminders)
        store.begin()
        openWhatWasTapped(intent)
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
        intent?.getStringExtra(AndroidReminders.EXTRA_TASK)?.let {
            intent.removeExtra(AndroidReminders.EXTRA_TASK)
            store.openFromOutside(it)
        }
    }

    override fun onStop() {
        super.onStop()
        engine.save()
    }
}
