package me.utsob.meghnaad.app

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

private const val PREVIEW = 12

private fun sizeText(bytes: Int) = when {
    bytes < 1024 -> "$bytes B"
    bytes < 1024 * 1024 -> "${fixed(bytes / 1024.0, 1)} KB"
    else -> "${fixed(bytes / 1024.0 / 1024.0, 1)} MB"
}

private val words = { n: Int -> "$n ${if (n == 1) "task" else "tasks"}" }

/** Coloured text: the `colors` palette, a sample or the legend, and the history graph (StyledView.svelte). */
@Composable
fun StyledView(r: StyledResult) {
    val dark = extras.dark
    val page = MaterialTheme.colorScheme
    Column(Modifier.horizontalScroll(rememberScrollState()).padding(vertical = 4.dp)) {
        for (line in r.lines) {
            val text = buildAnnotatedString {
                if (line.isEmpty()) append(" ")
                for (span in line) {
                    val st = span.style
                    var fg = st?.fg?.let { paletteColor(it, dark) }
                    var bg = st?.bg?.let { paletteColor(it, dark) }
                    if (st?.inverse == true) { val f = fg; fg = bg ?: page.surface; bg = f ?: page.onSurface }
                    withStyle(
                        SpanStyle(
                            color = fg ?: Color.Unspecified,
                            // A palette is drawn as it is; other backgrounds are only a wash, as everywhere else.
                            background = bg?.let { if (r.swatch) it else it.copy(alpha = 0.3f) } ?: Color.Unspecified,
                            fontWeight = if (st?.bold == true) FontWeight.Bold else null,
                            textDecoration = if (st?.underline == true) TextDecoration.Underline else null,
                        ),
                    ) { append(span.text) }
                }
            }
            Text(text, fontFamily = FontFamily.Monospace, fontSize = 13.sp, softWrap = false)
        }
    }
}

/** A file the command made (`export`): offered to keep rather than printed, since it can be thousands of lines (FileView.svelte). */
@Composable
fun FileView(store: Store, r: FileResult) {
    var saved by remember(r) { mutableStateOf<Boolean?>(null) }
    val lines = r.text.split('\n')
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text("Exported ${words(r.count)} (${sizeText(r.text.encodeToByteArray().size)}).")
        if (store.files != null) {
            Button(onClick = { store.go { saved = files?.save(r.name, r.mime, r.text) } }) {
                LucideIcon("download", size = 15.dp, tint = MaterialTheme.colorScheme.onPrimary)
                Text("  Save ${r.name}")
            }
            when (saved) {
                true -> Text("Saved.", color = extras.ok, fontSize = 13.sp)
                false -> Dim("Not saved.")
                null -> {}
            }
        } else Dim("This build can't save files.")
        if (r.count > 0) {
            var open by remember(r) { mutableStateOf(false) }
            TextButton(onClick = { open = !open }) { Text(if (open) "Hide the preview" else "Preview", fontSize = 13.sp) }
            if (open) {
                Column(Modifier.horizontalScroll(rememberScrollState())) {
                    for (l in lines.take(PREVIEW)) Text(l.ifEmpty { " " }, fontFamily = FontFamily.Monospace, fontSize = 12.sp, softWrap = false)
                    if (lines.size > PREVIEW) Text("…", fontFamily = FontFamily.Monospace, fontSize = 12.sp)
                }
            }
        }
    }
}

/**
 * `import`: pick a file of tasks (what `export` writes). The file is read here and sent as it is; the engine reads it, checks all
 * of it, and says what importing would do. Nothing is written until you confirm, and then all of it is or none (ImportView.svelte).
 */
@Composable
fun ImportView(store: Store) {
    var text by remember { mutableStateOf("") }
    var name by remember { mutableStateOf("") }
    var report by remember { mutableStateOf<ImportReport?>(null) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val colors = MaterialTheme.colorScheme
    val r = report
    val changes = if (r != null) r.added + r.modified else 0

    suspend fun choose() {
        val host = store.files ?: return
        val file = host.pick() ?: return
        report = null
        error = null
        if (file.text == null) {
            error = "That file is ${fixed(file.size / 1024.0 / 1024.0, 1)} MB; the most one import takes is ${MAX_IMPORT_BYTES / 1024 / 1024} MB. Split it and import the parts."
            return
        }
        busy = true
        try {
            name = file.name
            text = file.text
            report = engineJson.decodeFromJsonElement(ImportReport.serializer(), store.importTasks(text, false))
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        } finally {
            busy = false
        }
    }

    suspend fun apply() {
        busy = true
        error = null
        try {
            val done = engineJson.decodeFromJsonElement(ImportReport.serializer(), store.importTasks(text, true))
            report = done
            text = ""
            store.notify("Imported ${words(done.added + done.modified)}.")
            store.live?.let { store.refresh(it) }
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        } finally {
            busy = false
        }
    }

    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        if (store.files == null) Dim("This build can't open files.") else {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(onClick = { store.go { choose() } }, enabled = !busy) {
                    LucideIcon("upload", size = 15.dp)
                    Text("  Choose a file…")
                }
                Dim("Tasks as export writes them.${if (name.isNotEmpty() && r == null) " $name" else ""}")
            }
        }
        if (busy) Dim("Working…", size = 14)
        error?.let { Text(it, color = colors.error, fontSize = 14.sp) }
        if (r != null) {
            Text(
                "${if (r.applied) "Imported" else "$name:"} ${r.added} new, ${r.modified} changed, ${r.skipped} unchanged.",
                color = if (r.applied) extras.ok else colors.onSurface,
            )
            if (!r.applied) {
                if (changes > 0) Button(onClick = { store.go { apply() } }, enabled = !busy) { Text("Import ${words(changes)}") } else Dim("Nothing to import.", size = 14)
            }
            r.warnings.forEach { Text(it, color = extras.warn, fontSize = 13.sp) }
            r.feedback.forEach { Text(it.text, color = if (it.kind == "warn") extras.warn else colors.onSurface, fontSize = 13.sp) }
            if (r.lines.isNotEmpty()) {
                Column(Modifier.horizontalScroll(rememberScrollState())) {
                    for (l in r.lines) {
                        val label = when (l.action) { "add" -> "new"; "mod" -> "changed"; else -> "same" }
                        Text("${label.padEnd(8)}${l.uuid.take(8)}  ${l.description}", fontFamily = FontFamily.Monospace, fontSize = 12.sp, softWrap = false)
                    }
                    if (r.more > 0) Dim("…and ${r.more} more.")
                }
                if (!r.applied && changes > 0) Dim("Nothing is written until you import, and then all of it is, or none.")
            }
        }
    }
}
