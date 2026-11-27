package me.utsob.meghnaad.app

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch

// The Export button under a table and its dialog (ReportTable.svelte's button, ExportDialog.svelte): the same choices, in the
// same order and with the same defaults as the web app's and the terminal app's; the file is saved with the phone's own saver.

/** The Export button for the table [entry] drew, when it is a report's; it opens the dialog. */
@Composable
fun ExportButton(store: Store, entry: Entry?, columns: List<Column>) {
    val raw = store.config?.raw
    val line = remember(entry?.input, raw) {
        entry?.let { e -> runCatching { exportArgs(e.input, e.scope?.let(store::contextChoice), raw) }.getOrNull() }
    } ?: return
    var open by remember { mutableStateOf(false) }
    TextButton(onClick = { open = true }, modifier = Modifier.semantics { contentDescription = "Export these tasks" }) {
        LucideIcon("download", size = 14.dp, tint = MaterialTheme.colorScheme.primary)
        Text("  Export", fontSize = 13.sp)
    }
    if (open) ExportDialog(store, line, columns.map { it.name }) { open = false }
}

/** What the table shows as a file shaped as the person likes: the command it ran, then format, JSON's shape, and the fields. */
@Composable
fun ExportDialog(store: Store, args: List<String>, columns: List<String>, onClose: () -> Unit) {
    val raw = store.config?.raw
    val rcArray = remember(raw) { jsonArrayOf(raw) }
    val start = remember { rememberedExport(store.exportChoice()) }
    var choice by remember { mutableStateOf(start.copy(array = rcArray)) }
    var exported by remember { mutableStateOf<FileResult?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()
    LaunchedEffect(args) {
        when (val r = runCatching { store.exportTasks(args) }.getOrElse { CliResult.Error(ErrorResult(it.message ?: "The tasks could not be read.")) }) {
            is CliResult.File -> {
                exported = r.value
                // Nothing picked yet: start from the table's fields.
                if (choice.picked.isEmpty()) {
                    val have = exportFields(r.value.text)
                    choice = choice.copy(picked = tableFields(columns).filter { it in have })
                }
            }
            is CliResult.Error -> error = r.value.message
            else -> error = "The tasks could not be read."
        }
    }
    val available = remember(exported) { exported?.let { exportFields(it.text) } ?: emptyList() }
    val count = exportFieldCount(choice, available, columns)
    val colors = MaterialTheme.colorScheme

    @Composable
    fun Choice(label: String, selected: Boolean, radio: Boolean = true, mono: Boolean = false, onClick: () -> Unit) {
        Row(Modifier.fillMaxWidth().heightIn(min = 40.dp).clickable(onClick = onClick), verticalAlignment = Alignment.CenterVertically) {
            if (radio) RadioButton(selected, onClick = null, modifier = Modifier.padding(end = 8.dp))
            else Checkbox(selected, onCheckedChange = null, modifier = Modifier.padding(end = 8.dp))
            Text(label, fontSize = 15.sp, fontFamily = if (mono) FontFamily.Monospace else FontFamily.Default)
        }
    }

    AlertDialog(
        onDismissRequest = onClose,
        title = { Text("Export") },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                val e = exported
                when {
                    error != null -> Text(error ?: "", color = colors.error, fontSize = 13.sp)
                    e == null -> Dim("Reading the tasks…")
                    else -> Dim("${e.count} ${if (e.count == 1) "task" else "tasks"}, as the table shows them.")
                }
                Text(args.joinToString(" "), fontFamily = FontFamily.Monospace, fontSize = 12.sp, color = colors.onSurfaceVariant)
                Text("Format", fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(top = 10.dp))
                for (f in ExportFormat.entries) Choice(f.label, choice.format == f) { choice = choice.copy(format = f) }
                if (choice.format == ExportFormat.Json) {
                    Choice("As a list (json.array, ${if (rcArray) "on" else "off"} in your taskrc)", choice.array ?: rcArray, radio = false) {
                        choice = choice.copy(array = !(choice.array ?: rcArray))
                    }
                }
                Text("Fields", fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(top = 10.dp))
                Choice("All fields", choice.fields == ExportFields.All) { choice = choice.copy(fields = ExportFields.All) }
                Choice("Only the table's columns", choice.fields == ExportFields.Table) { choice = choice.copy(fields = ExportFields.Table) }
                Choice("Choose", choice.fields == ExportFields.Custom) { choice = choice.copy(fields = ExportFields.Custom) }
                if (choice.fields == ExportFields.Custom) {
                    Column(Modifier.padding(start = 24.dp)) {
                        for (f in available) Choice(f, f in choice.picked, radio = false, mono = true) {
                            choice = choice.copy(picked = if (f in choice.picked) choice.picked - f else choice.picked + f)
                        }
                    }
                }
                if (store.files == null) Dim("This build can't save files.")
            }
        },
        confirmButton = {
            TextButton(
                enabled = exported != null && count > 0 && store.files != null,
                onClick = {
                    val e = exported ?: return@TextButton
                    val file = renderExport(e.name, e.text, choice, columns, raw, store.clockNow())
                    store.keepExportChoice(keptExport(choice))
                    scope.launch {
                        store.files?.save(file.name, file.mime, file.text)
                        onClose()
                    }
                },
            ) { Text("Save") }
        },
        dismissButton = { TextButton(onClose) { Text("Cancel") } },
    )
}
