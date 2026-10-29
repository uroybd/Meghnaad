package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties

/**
 * One line for the common case, typed as in the console: the card under it is the task the line will make. "More fields" opens
 * the full form with that task already broken into its fields (QuickAddModal.svelte).
 */
@Composable
fun QuickAdd(store: Store, from: Entry?, line: String, onClose: () -> Unit) {
    var text by remember { mutableStateOf(line) }
    var busy by remember { mutableStateOf(false) }
    val focus = remember { FocusRequester() }
    val colors = MaterialTheme.colorScheme

    val typed = text.trim()
    val preview = if (typed.isEmpty()) null else previewAdd(typed, store.config?.config, store.now, store.utcOffset)
    // The engine refuses a line with any part wrong, so adding waits until the card has none to report.
    val ready = preview != null && preview.description.isNotEmpty() && preview.errors.isEmpty()

    fun add() {
        if (!ready || busy) return
        busy = true
        store.go {
            // A slow add (a hook) lets the form go after a second; if the engine then refuses it, the line comes back to be put right.
            val (res, released) = waitOrRelease("Creating…", onClose) { act(from, addArgs(typed)) }
            busy = false
            val ok = res != null && res.result !is CliResult.Error
            if (released) { if (!ok) quickAdd(typed) } else if (ok) onClose()
        }
    }

    LaunchedEffect(Unit) { focus.requestFocus() }
    Dialog(onDismissRequest = onClose, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        BoxWithConstraints(Modifier.fillMaxSize().safeDrawingPadding().imePadding()) {
            // A phone's box sits at the top, clear of the keyboard; a wide window has it in the middle.
            Box(Modifier.fillMaxSize().padding(8.dp), contentAlignment = if (maxWidth <= TABLE_FROM) Alignment.TopCenter else Alignment.Center) {
                Card(Modifier.widthIn(max = 560.dp).fillMaxWidth()) {
                    Column(Modifier.verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        Text("New task", fontSize = 18.sp, fontWeight = FontWeight.SemiBold)
                        OutlinedTextField(
                            value = text,
                            onValueChange = { text = it.replace("\n", " ") },
                            modifier = Modifier.fillMaxWidth().focusRequester(focus),
                            singleLine = true,
                            placeholder = { Text("Buy milk due:tomorrow +home", fontSize = 14.sp) },
                            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Send),
                            keyboardActions = KeyboardActions(onSend = { add() }),
                        )
                        Dim("Words are the description; project: due: priority: +tag set the rest.", size = 12)
                        AddPreviewCard(store, preview)
                        for (e in preview?.errors ?: emptyList()) Text(e, color = colors.error, fontSize = 12.sp)
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                            TextButton(onClick = { store.addWithForm(seedOf(preview, typed, store.config?.config?.udas ?: emptyMap(), store::offsetAt)) }) {
                                LucideIcon("sliders-horizontal", size = 14.dp)
                                Text("  More fields", fontSize = 13.sp)
                            }
                            Box(Modifier.weight(1f))
                            TextButton(onClick = onClose) { Text("Cancel") }
                            Button(enabled = !busy && ready, onClick = ::add) {
                                LucideIcon("plus", size = 14.dp, tint = colors.onPrimary)
                                Text("  Add task")
                            }
                        }
                    }
                }
            }
        }
    }
}

/** The task a line would add, drawn as a card; with no preview (nothing typed yet) it says what will show here (AddPreviewCard.svelte). */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AddPreviewCard(store: Store, preview: AddPreview?) {
    val colors = MaterialTheme.colorScheme
    val fmt = formatFor(FormatKind.Info, store.config?.config?.settings)
    val defs = store.config?.config?.udas ?: emptyMap()
    Card(Modifier.fillMaxWidth(), color = colors.background) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            if (preview == null) {
                Dim("The task will show here as you type.")
                return@Column
            }
            Row(verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (preview.description.isEmpty()) Dim("No description yet", Modifier.weight(1f), 15)
                else Text(preview.description, Modifier.weight(1f), fontWeight = FontWeight.SemiBold)
                preview.priority?.let { p ->
                    val c = when (p) { "H" -> colors.error; "M" -> colors.tertiary; else -> colors.primary }
                    Pill(p, background = c.copy(alpha = 0.14f), color = c, bold = true)
                }
            }
            FlowRow(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                @Composable
                fun Fact(icon: String?, label: String?, value: String) = Row(verticalAlignment = Alignment.CenterVertically) {
                    if (icon != null) { LucideIcon(icon, size = 13.dp); Text(" ") }
                    if (label != null) { Dim(label, size = 13); Text(" ") }
                    Text(value, fontSize = 13.sp)
                }
                preview.project?.let { Fact("folder", null, it) }
                for ((name, ts) in preview.dates) {
                    // A due or scheduled date on a holiday says so (a phone has no tooltip, so it is written out below).
                    val clash = if (name == "due" || name == "scheduled") {
                        runCatching { holidayClash(name, fromParts(toParts(ts, store.offsetAt(ts))), store.config?.config, store.now, store.utcOffset) }.getOrNull()
                    } else null
                    if (clash != null) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            LucideIcon("triangle-alert", size = 13.dp, tint = extras.warn); Text(" ")
                            Text(name, fontSize = 13.sp, color = extras.warn); Text(" ")
                            Text(formatMoment(ts, store.offsetAt(ts), fmt), fontSize = 13.sp, color = extras.warn)
                        }
                    } else Fact("calendar", name, formatMoment(ts, store.offsetAt(ts), fmt))
                }
                preview.recur?.let { Fact("repeat", null, describeRecur(it)) }
                for ((name, value) in preview.udas) {
                    val def = defs[name]
                    Fact(null, def?.let { udaLabel(it) } ?: name, if (def?.type == "date") formatMoment(value.toLongOrNull(), store.offsetAt(value.toLongOrNull() ?: 0L), fmt) else value)
                }
                if (preview.depends.isNotEmpty()) Fact("link", null, "depends on " + preview.depends.joinToString(", "))
                for (t in preview.tags) Pill(t, background = colors.primary.copy(alpha = 0.12f), color = colors.primary)
            }
            for ((name, ts) in preview.dates) {
                if (name != "due" && name != "scheduled") continue
                runCatching { holidayClash(name, fromParts(toParts(ts, store.offsetAt(ts))), store.config?.config, store.now, store.utcOffset) }.getOrNull()
                    ?.let { Text(it.message, color = extras.warn, fontSize = 12.sp) }
            }
        }
    }
}
