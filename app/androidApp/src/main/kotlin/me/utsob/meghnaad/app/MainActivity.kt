package me.utsob.meghnaad.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.activity.compose.setContent
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

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Draw under the system bars (the app pads for them), as a phone's apps do.
        enableEdgeToEdge()
        APP_VERSION = packageManager.getPackageInfo(packageName, 0).versionName ?: ""
        val vault = AndroidVault(this)
        engine = LocalEngine(filesDir, vault)
        val sync = SyncController(engine, lifecycleScope, SystemClock)
        val store = Store(Tw(engine, SystemClock), engine, vault, sync, lifecycleScope, SystemClock, AndroidPrefs(this))
        store.begin()
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

    override fun onStop() {
        super.onStop()
        engine.save()
    }
}
