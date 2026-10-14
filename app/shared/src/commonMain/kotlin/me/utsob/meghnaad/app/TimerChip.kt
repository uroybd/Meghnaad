package me.utsob.meghnaad.app

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay

/** The task that is running, its tracked time ticking, and a way to stop it (TimerChip.svelte). */
@Composable
fun TimerChip(store: Store, modifier: Modifier = Modifier) {
    val active = store.notifier.active
    val first = active.firstOrNull() ?: return
    var now by remember { mutableStateOf(store.clockNow()) }
    LaunchedEffect(first.uuid) {
        while (true) {
            delay(1000)
            now = store.clockNow()
        }
    }
    // With journal.time the engine has the tracked total; otherwise it's the current stretch.
    val elapsed = if (first.activeSeconds != null) first.activeSeconds + maxOf(0, now - store.notifier.polledAt) else maxOf(0, now - (first.start ?: now))
    Surface(
        modifier.semantics { contentDescription = "Task in progress" },
        shape = RoundedCornerShape(999.dp),
        color = androidx.compose.ui.graphics.Color.Transparent,
        border = BorderStroke(1.dp, extras.ok),
    ) {
        Row(Modifier.padding(start = 10.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Row(Modifier.weight(1f, fill = false).clickable { store.openDetail(first.uuid) }, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                LucideIcon("timer", size = 15.dp, tint = extras.ok)
                Text(first.description, Modifier.weight(1f, fill = false), maxLines = 1, overflow = TextOverflow.Ellipsis, fontSize = 13.sp)
                Text(formatSeconds(elapsed), fontFamily = FontFamily.Monospace, fontSize = 13.sp)
                if (active.size > 1) Text("+${active.size - 1}", fontSize = 12.sp)
            }
            IconAction("square", "Stop ${first.description}", { store.go { act(null, listOf(first.uuid, "stop")); notifier.poll() } }, size = 14.dp)
        }
    }
}
