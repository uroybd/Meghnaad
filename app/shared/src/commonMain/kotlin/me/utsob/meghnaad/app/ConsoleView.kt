package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** The console: every command's answer in a scrollback, and the prompt under it (ConsoleView.svelte). */
@Composable
fun ConsoleView(store: Store) {
    val state = rememberLazyListState()
    LaunchedEffect(store.entries.size) {
        if (store.entries.isNotEmpty()) state.animateScrollToItem(store.entries.lastIndex)
    }
    Column(Modifier.fillMaxSize()) {
        if (store.entries.isEmpty()) {
            Dim(
                "Type a command below: next, add Buy milk due:tomorrow, project:Home +work list, … Tab completes; help lists them all.",
                Modifier.weight(1f).padding(16.dp),
                size = 14,
            )
        } else {
            LazyColumn(Modifier.weight(1f).fillMaxWidth().padding(horizontal = 12.dp), state = state, verticalArrangement = Arrangement.spacedBy(12.dp), contentPadding = androidx.compose.foundation.layout.PaddingValues(vertical = 12.dp)) {
                items(store.entries.toList(), key = { it.id }) { e ->
                    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        if (e.title.isNotEmpty()) {
                            Text("$ ${e.title}", fontFamily = FontFamily.Monospace, fontSize = 12.5.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        ResultBlock(store, e)
                    }
                }
            }
        }
        Surface(Modifier.fillMaxWidth(), color = MaterialTheme.colorScheme.surfaceContainer, tonalElevation = 2.dp) { ConsoleInput(store) }
    }
}
