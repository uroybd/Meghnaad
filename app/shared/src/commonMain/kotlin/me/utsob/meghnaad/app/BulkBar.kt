package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** What can be done to the ticked tasks all at once, from two ticked on (BulkBar.svelte). */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun BulkBar(store: Store) {
    // The ticked tasks, as the list shows them (their numbers come from the rows), in the order they were ticked.
    val rows = (store.live?.result as? CliResult.Report)?.value?.rows ?: emptyList()
    val byUuid = rows.associateBy { it.uuid }
    val picks = store.selection.uuids.mapNotNull { u -> byUuid[u]?.let { Pick(u, it.id) } }
    var busy by remember { mutableStateOf(false) }
    var confirmDelete by remember { mutableStateOf(false) }
    var modifying by remember { mutableStateOf(false) }
    if (picks.size <= 1 && !modifying) return

    /**
     * Run [words] on the ticked tasks, a batch at a time. The tap is the answer to Taskwarrior's "are you sure?" questions, so
     * they are answered yes; questions about recurring tasks and dependencies are still asked. Whether everything went through.
     */
    suspend fun Store.run(words: List<String>): Boolean {
        busy = true
        try {
            for (batch in batches(picks)) {
                val res = act(live, filterWords(batch) + words, Answers(approved = batch.map { it.uuid }, confirmed = true))
                if (res?.wrote != true) return false
            }
            return true
        } finally {
            busy = false
        }
    }

    if (picks.size > 1) {
        Column(Modifier.fillMaxWidth().padding(vertical = 6.dp)) {
            Text("${picks.size} selected", fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(2.dp), itemVerticalAlignment = Alignment.CenterVertically) {
                OutlinedButton(enabled = !busy, onClick = { store.go { if (run(listOf("done"))) selection.clear() } }) {
                    LucideIcon("check", size = 15.dp); Text("  Complete")
                }
                OutlinedButton(enabled = !busy, onClick = {
                    // The second tap is the answer to "Delete N tasks?"; with `confirmation` off, the first tap deletes.
                    if (store.confirmation && !confirmDelete) {
                        confirmDelete = true
                        store.go { delay(3000); confirmDelete = false }
                    } else {
                        confirmDelete = false
                        store.go { if (run(listOf("delete"))) selection.clear() }
                    }
                }) {
                    LucideIcon("trash-2", size = 15.dp, tint = MaterialTheme.colorScheme.error)
                    Text(if (confirmDelete) "  Delete ${picks.size}? Tap again" else "  Delete", color = MaterialTheme.colorScheme.error)
                }
                OutlinedButton(enabled = !busy, onClick = { modifying = true }) { LucideIcon("pencil", size = 15.dp); Text("  Modify…") }
                // Name the tasks at the prompt and leave the rest to the user.
                OutlinedButton(enabled = !busy, onClick = { store.requestPrompt(promptFor(picks)) }) { LucideIcon("terminal", size = 15.dp); Text("  CLI") }
                MoreMenu(store.taskCommands, "More actions for the selected tasks", enabled = !busy) { c ->
                    store.go { if (run(commandWords(c))) selection.clear() }
                }
                TextButton(onClick = { store.selection.clear() }) { LucideIcon("x", size = 15.dp); Text("  Clear") }
            }
        }
    }
    if (modifying) {
        BulkModify(store, picks.size, onClose = { modifying = false }) { words ->
            var ok = false
            store.go {
                ok = run(listOf("modify") + words)
                if (ok) selection.clear()
            }.join()
            ok
        }
    }
}

/** Each line sets one field on every selected task; an empty value clears it; a line for tags adds or removes the tags named (BulkModify.svelte). */
@Composable
fun BulkModify(store: Store, count: Int, onClose: () -> Unit, onApply: suspend (List<String>) -> Boolean) {
    val fields = fieldsFor(store.config?.config?.udas ?: emptyMap())
    val entries = remember { mutableStateListOf(ModifyEntry("project", "")) }
    var busy by remember { mutableStateOf(false) }
    val scope = androidx.compose.runtime.rememberCoroutineScope()

    // What is wrong, if anything: an unusable entry, or the same field twice (tags excepted).
    val issue: String? = run {
        if (entries.isEmpty()) return@run "Add at least one change."
        for (e in entries) problem(e, fields)?.let { return@run it }
        val seen = HashSet<String>()
        for (e in entries) {
            if (e.key == "+" || e.key == "-") continue
            if (!seen.add(e.key)) return@run "${fields.firstOrNull { it.key == e.key }?.label ?: e.key} is set twice."
        }
        null
    }
    val summary = if (entries.isNotEmpty() && issue == null) describe(entries, fields) else ""

    AlertDialog(
        onDismissRequest = onClose,
        title = { Text("Modify $count tasks") },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Dim("Each line sets one field on every selected task. Leave the value empty to clear a field. A line for tags adds or removes the tags you name.", size = 13)
                entries.forEachIndexed { i, e ->
                    val f = fields.firstOrNull { it.key == e.key }
                    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Choice(f?.label ?: "Field", fields.map { it.label }) { j -> entries[i] = ModifyEntry(fields[j].key, "") }
                            androidx.compose.foundation.layout.Spacer(Modifier.weight(1f))
                            IconAction("x", "Remove this change", { entries.removeAt(i) }, size = 16.dp)
                        }
                        when {
                            f?.kind == FieldKind.Project -> SuggestField(e.value, { entries[i] = e.copy(value = it) }, store.projects, "project", placeholder = "none")
                            f?.kind == FieldKind.Tags -> SuggestField(e.value, { entries[i] = e.copy(value = it) }, store.tags, "tag", list = true, placeholder = "tags")
                            f?.values != null -> Choice(e.value.ifEmpty { "clear" }, listOf("clear") + f.values) { j -> entries[i] = e.copy(value = if (j == 0) "" else f.values[j - 1]) }
                            else -> OutlinedTextField(e.value, { entries[i] = e.copy(value = it) }, Modifier.fillMaxWidth(), singleLine = true, placeholder = { Text(f?.hint ?: "", fontSize = 13.sp, maxLines = 1) })
                        }
                    }
                }
                TextButton(onClick = {
                    // The next field nobody has used yet, so a second entry is not a duplicate by default.
                    val used = entries.map { it.key }.toSet()
                    val next = fields.firstOrNull { it.key !in used || it.key == "+" || it.key == "-" }
                    entries.add(ModifyEntry(next?.key ?: "project", ""))
                }) { LucideIcon("plus", size = 15.dp); Text("  Add a change") }
                if (issue != null) Text(issue, color = MaterialTheme.colorScheme.error, fontSize = 13.sp)
                else if (summary.isNotEmpty()) Text("On $count tasks: $summary.", fontSize = 13.sp)
            }
        },
        confirmButton = {
            Button(enabled = issue == null && !busy, onClick = {
                busy = true
                scope.launch {
                    val ok = try { onApply(modifyWords(entries)) } finally { busy = false }
                    if (ok) onClose()
                }
            }) { Text("Modify $count tasks") }
        },
        dismissButton = { TextButton(onClick = onClose) { Text("Cancel") } },
    )
}

