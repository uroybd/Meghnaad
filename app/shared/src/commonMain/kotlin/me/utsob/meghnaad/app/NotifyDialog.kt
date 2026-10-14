package me.utsob.meghnaad.app

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
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
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
                    "Get a heads-up about due dates and tasks coming off wait. They keep working when the app is closed: the phone wakes the app at the " +
                        "moment something needs saying.",
                    size = 13,
                )
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.SpaceBetween) {
                    Text("Remind me about due dates", Modifier.weight(1f))
                    Switch(s.enabled, { if (it) n.enable() else n.disable() })
                }
                Dim(status, size = 12)
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text("Heads-up before a timed due date", fontSize = 14.sp)
                    Choice(LEADS.first { it.first == s.leadMinutes }.second, LEADS.map { it.second }) { n.update(s.copy(leadMinutes = LEADS[it].first)) }
                    Text("Due on a date with no time", fontSize = 14.sp, modifier = Modifier.padding(top = 4.dp))
                    Choice(hourLabel(s.allDayHour), (0..23).map(::hourLabel)) { n.update(s.copy(allDayHour = it)) }
                    Row(Modifier.fillMaxWidth().padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text("Tell me when a waiting task comes back", Modifier.weight(1f), fontSize = 14.sp)
                        Switch(s.waitOver, { n.update(s.copy(waitOver = it)) })
                    }
                }
                n.lastError?.let { Text("Couldn't check for reminders: $it", color = colors.error, fontSize = 12.sp) }
            }
        },
        confirmButton = { TextButton(onClick = { store.notifyOpen = false }) { Text("Done") } },
        dismissButton = { TextButton(onClick = { n.test() }) { Text("Send a test") } },
    )
}
