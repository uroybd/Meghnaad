package me.utsob.meghnaad.app

import androidx.compose.ui.draw.alpha
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import kotlinx.coroutines.launch
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

private val LEADS = listOf(0 to "none", 5 to "5 minutes", 15 to "15 minutes", 30 to "30 minutes", 60 to "1 hour", 120 to "2 hours")
private fun hourLabel(h: Int) = "remind at ${h.toString().padStart(2, '0')}:00"

/** The reminder options (NotifyDialog.svelte). */
@Composable
fun NotifyDialog(store: Store) {
    val n = store.notifier
    val s = n.settings
    val colors = MaterialTheme.colorScheme
    val scope = rememberCoroutineScope()
    val uda = remindUdaStatus(store.config)
    var adding by remember { mutableStateOf(false) }
    var udaError by remember { mutableStateOf<String?>(null) }
    // One `config` line at a time, as tasksh's setup does.
    fun addUda() = scope.launch {
        adding = true
        udaError = null
        try {
            for (line in remindUdaLines(store.config)) store.query(configArgs(line))
        } catch (e: Exception) {
            udaError = e.message ?: e.toString()
        } finally {
            store.settingsChanged()
            adding = false
        }
    }
    val status = when {
        !n.hasSystem -> "This build can't show system notifications. You'll still get alerts inside the app."
        n.systemAllowed -> "System notifications are allowed."
        else -> "The phone will ask for permission when you turn reminders on. If you said no before, allow notifications for this app in the phone's settings; you'll still get alerts inside the app."
    }
    AlertDialog(
        onDismissRequest = { store.notifyOpen = false },
        title = { Text("Reminders") },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Dim(
                    "Get a heads-up about due and scheduled dates and tasks coming off wait. They keep working when the app is closed: the phone wakes the app at the " +
                        "moment something needs saying.",
                    size = 13,
                )
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.SpaceBetween) {
                    Text("Remind me about due dates", Modifier.weight(1f))
                    Switch(s.enabled, { if (it) n.enable() else n.disable() })
                }
                Dim(status, size = 12)
                // Dimmed while reminders are off, and still usable, as in the web.
                Column(Modifier.alpha(if (s.enabled) 1f else 0.5f), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text("Heads-up before a timed due or scheduled date", fontSize = 14.sp)
                    Choice(LEADS.first { it.first == s.leadMinutes }.second, LEADS.map { it.second }) { n.update(s.copy(leadMinutes = LEADS[it].first)) }
                    Text("Due on a date with no time", fontSize = 14.sp, modifier = Modifier.padding(top = 4.dp))
                    Choice(hourLabel(s.allDayHour), (0..23).map(::hourLabel)) { n.update(s.copy(allDayHour = it)) }
                    Row(Modifier.fillMaxWidth().padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text("Tell me when a waiting task comes back", Modifier.weight(1f), fontSize = 14.sp)
                        Switch(s.waitOver, { n.update(s.copy(waitOver = it)) })
                    }
                }
                Dim(
                    "A task can have its own times with remind_in:10,1h: minutes, or durations as Taskwarrior writes them (10min, 1h, 2d; " +
                        "10m is ten months). For a date with no time they count back from the hour above. none turns them off for that task.",
                    size = 12,
                )
                if (uda == Part.Ok) {
                    Dim("The remind_in attribute is in your taskrc.", size = 12)
                } else {
                    Dim(
                        if (uda == Part.Missing) "Your taskrc has no remind_in attribute (UDA) yet, so the value is not kept on a task."
                        else "Your taskrc defines remind_in with another type; adding it makes it a string.",
                        size = 12,
                    )
                    OutlinedButton(onClick = { addUda() }, enabled = !adding) { Text("Add the remind_in attribute") }
                    udaError?.let { Text(it, color = colors.error, fontSize = 12.sp) }
                }
                n.lastError?.let { Text("Couldn't check for reminders: $it", color = colors.error, fontSize = 12.sp) }
            }
        },
        confirmButton = { TextButton(onClick = { store.notifyOpen = false }) { Text("Done") } },
        dismissButton = { TextButton(onClick = { n.test() }) { Text("Send a test") } },
    )
}
