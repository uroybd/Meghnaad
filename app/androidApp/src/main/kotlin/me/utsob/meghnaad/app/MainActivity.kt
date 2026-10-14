package me.utsob.meghnaad.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.ui.Modifier
import androidx.lifecycle.lifecycleScope

class MainActivity : ComponentActivity() {
    private lateinit var engine: LocalEngine

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        engine = LocalEngine(filesDir, KeystoreSecrets(this))
        val model = AppModel(Tw(engine, SystemClock), lifecycleScope, SystemClock)
        model.start()
        setContent { Box(Modifier.fillMaxSize().statusBarsPadding().imePadding()) { App(model) } }
    }

    override fun onStop() {
        super.onStop()
        engine.save()
    }
}
