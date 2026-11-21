package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.launch

// Naming this device on the first save of its own settings, and choosing which device's settings to use on the first
// switch to This device (spec 0027; DeviceDrawer.svelte).

/** What is being asked about this device's settings. */
sealed interface DeviceAsk {
    class Name(val id: String, val answer: CompletableDeferred<String?>) : DeviceAsk

    class Choose(val ids: List<String>) : DeviceAsk
}

/** This device's ID, asking for one (prefilled with a new one) when it has none yet; null when the person declined. */
suspend fun ensureDeviceId(store: Store): String? {
    store.deviceId?.let { return it }
    val answer = CompletableDeferred<String?>()
    store.deviceAsk = DeviceAsk.Name(newDeviceId("phone"), answer)
    return answer.await()
}

/** The first switch to This device: when other devices have settings of their own, offer to use one of them. */
suspend fun offerDevices(store: Store) {
    if (store.deviceId != null) return
    val ids = runCatching { store.listDevices() }.getOrDefault(emptyList())
    if (ids.isNotEmpty()) store.deviceAsk = DeviceAsk.Choose(ids)
}

@Composable
fun DevicePrompt(store: Store) {
    val scope = rememberCoroutineScope()
    when (val ask = store.deviceAsk) {
        null -> {}
        is DeviceAsk.Name -> {
            var id by remember(ask) { mutableStateOf(ask.id) }
            val problem = deviceIdProblem(id.trim())
            fun close(answer: String?) {
                ask.answer.complete(answer)
                store.deviceAsk = null
            }
            AlertDialog(
                onDismissRequest = { close(null) },
                title = { Text("Name this device") },
                text = {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Dim(
                            "This device's own settings are kept under its ID, beside everyone's settings.json. Keep this one " +
                                "or type your own; another device given the same ID shares them.",
                        )
                        OutlinedTextField(
                            value = id,
                            onValueChange = { id = it },
                            label = { Text("Device ID") },
                            singleLine = true,
                            textStyle = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
                            isError = problem != null,
                            modifier = Modifier.fillMaxWidth(),
                        )
                        if (problem != null && id.isNotBlank()) Text(problem, color = MaterialTheme.colorScheme.error, fontSize = 13.sp)
                    }
                },
                confirmButton = { Button({ close(id.trim()) }, enabled = problem == null) { Text("Save") } },
                dismissButton = { TextButton({ close(null) }) { Text("Cancel") } },
            )
        }
        is DeviceAsk.Choose -> {
            var error by remember(ask) { mutableStateOf<String?>(null) }
            AlertDialog(
                onDismissRequest = { store.deviceAsk = null },
                title = { Text("This device's settings") },
                text = {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Dim("Other devices have settings of their own. Use one of them here, or make new ones for this device.")
                        for (d in ask.ids) {
                            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                OutlinedButton({
                                    scope.launch {
                                        runCatching { store.useDevice(d) }.onSuccess { store.deviceAsk = null }.onFailure { error = it.message }
                                    }
                                }, Modifier.weight(1f)) { Text("Use $d", fontFamily = FontFamily.Monospace) }
                                TextButton({
                                    scope.launch {
                                        runCatching { store.deleteDevice(d) }
                                            .onSuccess {
                                                val left = ask.ids - d
                                                store.deviceAsk = if (left.isEmpty()) null else DeviceAsk.Choose(left)
                                            }
                                            .onFailure { error = it.message }
                                    }
                                }) { Text("Delete") }
                            }
                        }
                        error?.let { Text(it, color = MaterialTheme.colorScheme.error, fontSize = 13.sp) }
                        Dim("New ones are made, and named, when you first save something under This device.")
                    }
                },
                confirmButton = { Button({ store.deviceAsk = null }) { Text("New ones for this device") } },
            )
        }
    }
}

private val Int.dp get() = androidx.compose.ui.unit.Dp(this.toFloat())
