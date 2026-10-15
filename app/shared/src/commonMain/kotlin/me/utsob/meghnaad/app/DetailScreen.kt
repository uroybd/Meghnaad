package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * One task in full, over the page (a phone's drawer is the whole screen): what it is, the buttons for it, the time tracked so
 * far, everything about it, and a field for a note. Loads on open and again after any write (DetailDrawer.svelte).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun DetailScreen(store: Store, ref: DetailRef) {
    val uuid = ref.uuid
    var task by remember(uuid) { mutableStateOf<Row?>(null) }
    var message by remember(uuid) { mutableStateOf<String?>(null) }
    var note by remember(uuid) { mutableStateOf("") }
    var confirmDelete by remember(uuid) { mutableStateOf(false) }
    var fetchedAt by remember(uuid) { mutableStateOf(store.now) }
    var tick by remember(uuid) { mutableStateOf(store.now) }
    val colors = MaterialTheme.colorScheme

    // Load on open, and again after any write (this screen's own, the list's, or the console's).
    LaunchedEffect(uuid, store.rev) {
        try {
            when (val r = store.query(listOf(uuid, "info"))) {
                is CliResult.Info -> {
                    task = r.value.tasks.firstOrNull()
                    message = null
                    fetchedAt = store.clockNow()
                }
                else -> {
                    message = "This task is no longer here."
                    task = null
                }
            }
        } catch (e: Exception) {
            message = e.message ?: e.toString()
        }
    }
    // While a task is running, keep its tracked time ticking between fetches.
    val running = task?.start != null
    LaunchedEffect(running) {
        while (running) {
            delay(1000)
            tick = store.clockNow()
        }
    }
    val live = task?.let { t ->
        if (t.activeSeconds != null && t.start != null) {
            val grown = maxOf(0, tick - fetchedAt)
            // Only the running session (no end) grows between fetches.
            t.copy(activeSeconds = t.activeSeconds + grown, sessions = t.sessions.map { s -> if (s.end == null) s.copy(seconds = s.seconds + grown) else s })
        } else {
            t
        }
    }

    val clipboard = LocalClipboardManager.current

    fun act(vararg args: String) = store.go { act(ref.from, listOf(uuid) + args.toList()) }

    Sheet(drawer = true, width = 560.dp, color = colors.surfaceContainerLow) {
        Column(Modifier.fillMaxSize()) {
            Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp, top = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(live?.description ?: "Task", Modifier.weight(1f), fontWeight = FontWeight.SemiBold, fontSize = 18.sp, maxLines = 3)
                IconAction("link", "Copy a link to this task", { clipboard.setText(AnnotatedString(taskLink(uuid))); store.notify("Link copied.") }, size = 20.dp)
                IconAction("x", "Close details", { store.detail = null }, size = 20.dp)
            }
            Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 8.dp)) {
                message?.let { Text(it, color = colors.error, fontSize = 14.sp) }
                if (live != null) {
                    FlowRow(Modifier.fillMaxWidth().padding(vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        if (live.status == "pending") {
                            Button(onClick = { act("done") }) {
                                LucideIcon("check", size = 15.dp, tint = colors.onPrimary)
                                Text("  Done")
                            }
                            if (live.start != null) {
                                OutlinedButton(onClick = { act("stop") }) { LucideIcon("square", size = 13.dp); Text("  Stop") }
                            } else {
                                OutlinedButton(onClick = { act("start") }) { LucideIcon("play", size = 13.dp); Text("  Start") }
                            }
                        } else if (live.status != "deleted") {
                            OutlinedButton(onClick = { act("start") }) { LucideIcon("rotate-ccw", size = 14.dp); Text("  Reopen & start") }
                        }
                        OutlinedButton(onClick = { store.editing = EditRef(live, ref.from) }) { LucideIcon("pencil", size = 14.dp); Text("  Edit") }
                        if (live.status != "deleted") {
                            OutlinedButton(onClick = {
                                // The second tap answers Taskwarrior's "Delete task N?"; with `confirmation` off there is none.
                                if (store.confirmation && !confirmDelete) {
                                    confirmDelete = true
                                    store.go { delay(3000); confirmDelete = false }
                                } else {
                                    store.go {
                                        val res = act(ref.from, listOf(uuid, "delete"), Answers(approved = listOf(uuid)))
                                        if (res != null && res.result !is CliResult.Error) detail = null
                                    }
                                }
                            }) {
                                if (confirmDelete) Text("sure?", color = colors.error) else {
                                    LucideIcon("trash-2", size = 14.dp, tint = colors.error)
                                    Text("  Delete", color = colors.error)
                                }
                            }
                        }
                        MoreMenu(store.taskCommands) { c -> act(*commandWords(c).toTypedArray()) }
                    }
                    if (live.start != null && live.activeSeconds != null) {
                        Row(Modifier.padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            LucideIcon("timer", size = 15.dp, tint = extras.ok)
                            Text("running · ${formatSeconds(live.activeSeconds)} tracked", color = extras.ok, fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
                        }
                    }
                    TaskInfo(store, live) { u -> store.openDetail(u, ref.from) }
                    // A note: Enter adds a line (a phone's keyboard has no Shift+Enter), so it is added with the button.
                    Row(Modifier.fillMaxWidth().padding(top = 18.dp, bottom = 24.dp), verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        OutlinedTextField(
                            value = note,
                            onValueChange = { note = it },
                            modifier = Modifier.weight(1f),
                            placeholder = { Text("Add an annotation…", fontSize = 14.sp) },
                            minLines = 1,
                            maxLines = 5,
                            keyboardOptions = KeyboardOptions(capitalization = androidx.compose.ui.text.input.KeyboardCapitalization.Sentences),
                        )
                        Button(enabled = note.isNotBlank(), onClick = {
                            val text = note.trim()
                            store.go {
                                val res = act(ref.from, listOf(uuid, "annotate", text))
                                if (res != null && res.result !is CliResult.Error) note = ""
                            }
                        }) { Text("Add") }
                    }
                } else if (message == null) {
                    Dim("Loading…")
                }
            }
        }
    }
}
