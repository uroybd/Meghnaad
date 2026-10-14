package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay

/** The whole app, the same on every platform: the first screens (connect, start), then the shell. */
@Composable
fun App(store: Store) {
    // Every icon the app can draw, loaded once off the main thread: they appear as soon as it is done.
    LaunchedEffect(Unit) { LucideLib.load() }
    AppTheme {
        Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
            when (store.setup) {
                Setup.Checking, Setup.Starting -> Box(Modifier.fillMaxSize(), Alignment.Center) { Dim("Starting…", size = 15) }
                Setup.NeedsConnection -> ConnectScreen(store)
                Setup.Failed -> Text(
                    store.error ?: "The engine could not start.",
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(24.dp),
                )
                Setup.Ready -> {
                    // Relative dates ("due in 2d") stay fresh without asking the engine again.
                    LaunchedEffect(Unit) {
                        while (true) {
                            delay(30_000)
                            store.tick()
                        }
                    }
                    Shell(store)
                }
            }
        }
    }
}
