package me.utsob.meghnaad.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.statusBarsPadding
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
        val vault = AndroidVault(this)
        engine = LocalEngine(filesDir, vault)
        val sync = SyncController(engine, lifecycleScope, SystemClock)
        val store = Store(Tw(engine, SystemClock), engine, vault, sync, lifecycleScope, SystemClock, AndroidPrefs(this))
        store.begin()
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
        setContent { Box(Modifier.fillMaxSize().statusBarsPadding().imePadding()) { App(store) } }
    }

    override fun onStop() {
        super.onStop()
        engine.save()
    }
}
