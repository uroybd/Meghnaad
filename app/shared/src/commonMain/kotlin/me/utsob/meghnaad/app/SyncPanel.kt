package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** The bucket this phone syncs its tasks with: connect it later, keep settings and hooks in step, sync now, or stop (SyncPanel.svelte). */
@Composable
fun SyncPanel(store: Store) {
    var busy by remember { mutableStateOf(false) }
    val options = store.syncOptions

    fun syncNow() {
        busy = true
        store.go {
            val r = sync.request().await()
            if (r.isSuccess) notify("Synced.")
            // The banner under the bar says why it did not.
            busy = false
        }
    }

    /** Keep the settings and/or the hooks in step with the bucket, from the next sync (now) on. */
    fun choose(next: SyncOptions) {
        store.syncOptions = next
        if (next.settings || next.hooks) syncNow()
    }

    Column(Modifier.fillMaxWidth().padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        if (store.connected) {
            Text("Syncing is on: this phone's tasks go to your S3 bucket, and what other devices add comes here.", fontSize = 14.sp)
            store.sync.lastSync?.let { Dim("Last synced ${formatStamp(it.toLong(), store.offsetAt(it.toLong())).takeLast(8)}.") }
            Text("Keep in step with the bucket", fontSize = 14.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold, modifier = Modifier.padding(top = 6.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Checkbox(options.settings, { choose(options.copy(settings = it)) })
                Text("Settings: the taskrc subset, reports, commands and the rest", fontSize = 14.sp)
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Checkbox(options.hooks, { choose(options.copy(hooks = it)) })
                Text("Hooks: the Lua script", fontSize = 14.sp)
            }
            Dim(
                "These are the files the web app keeps in the bucket, so an edit on either side reaches the other at the next sync; when both changed, " +
                    "the newer one wins and the other is kept to restore. The secrets for hooks are never synced.",
                size = 12,
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = ::syncNow, enabled = !busy) { Text(if (busy) "Syncing…" else "Sync now") }
                OutlinedButton(onClick = {
                    store.go {
                        if (confirmAsk("Stop syncing? The tasks stay on this phone, and the bucket keeps what it has.", "Stop syncing", "Cancel")) {
                            disconnect()
                            notify("Not syncing any more. Your tasks are on this phone only.")
                        }
                    }
                }) { Text("Stop syncing") }
            }
        } else {
            Text(
                "Syncing is off: your tasks are on this phone only. Connect an S3 bucket (the one your task CLI syncs with) to have them everywhere. " +
                    "What is on the phone is added to what the bucket has.",
                fontSize = 14.sp,
            )
            ConnectForm(connect = {
                val problem = store.connect(it)
                if (problem == null) store.notify("Connected. Your tasks are syncing.")
                problem
            })
        }
        androidx.compose.foundation.layout.Spacer(Modifier.padding(bottom = 24.dp))
    }
}
