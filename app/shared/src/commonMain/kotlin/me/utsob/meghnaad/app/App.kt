package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp

/** The whole app, the same on every platform. */
@Composable
fun App(model: AppModel) {
    MaterialTheme {
        Column(Modifier.fillMaxSize().padding(horizontal = 12.dp, vertical = 8.dp)) {
            TopBar(model)
            model.notice?.let { Text(it, modifier = Modifier.padding(vertical = 4.dp)) }
            model.error?.let { Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(vertical = 4.dp)) }
            model.response?.let { Body(model, it) }
        }
        model.pending?.let { Question(model, it) }
    }
}

@Composable
private fun TopBar(model: AppModel) {
    var line by remember { mutableStateOf("") }
    var open by remember { mutableStateOf(false) }
    // The on-screen keyboard sends an action, a hardware keyboard an Enter key: both run the line.
    fun submit() {
        if (line.isBlank()) return
        model.run(line.trim())
        line = ""
    }
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Box {
            TextButton(onClick = { open = true }) { Text("Reports") }
            DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
                for (report in model.reports) {
                    DropdownMenuItem(text = { Text(report.name) }, onClick = {
                        open = false
                        model.runReport(report.name)
                    })
                }
            }
        }
        OutlinedTextField(
            value = line,
            onValueChange = { line = it },
            singleLine = true,
            label = { Text("task …") },
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
            keyboardActions = KeyboardActions(onSend = { submit() }),
            modifier = Modifier.weight(1f).onKeyEvent {
                if (it.key == Key.Enter && it.type == KeyEventType.KeyDown) {
                    submit()
                    true
                } else {
                    false
                }
            },
        )
    }
}

@Composable
private fun Body(model: AppModel, response: CliResponse) {
    response.feedback.forEach { Text(it.text, modifier = Modifier.padding(vertical = 2.dp)) }
    when (val result = response.result) {
        is CliResult.Report -> ReportList(result.value, model.utcOffset)
        is CliResult.Text -> Lines(result.value.lines)
        is CliResult.Table -> Lines(listOfNotNull(result.value.title) + result.value.headers.joinToString("  ") + result.value.rows.map { it.joinToString("  ") })
        is CliResult.Changed -> Lines(listOf(result.value.message) + result.value.tasks.map { "${it.id ?: "-"}  ${it.description}" })
        is CliResult.Error -> Text(result.value.message, color = MaterialTheme.colorScheme.error)
        is CliResult.Confirm -> Unit
        is CliResult.Unsupported -> Text("“${result.kind}” has no screen yet.")
    }
}

@Composable
private fun ReportList(report: ReportResult, offset: Int) {
    Text("${report.report}: ${report.matched} tasks", style = MaterialTheme.typography.labelLarge, modifier = Modifier.padding(vertical = 4.dp))
    LazyColumn(Modifier.fillMaxSize()) {
        items(report.rows, key = { it.uuid }) { row ->
            ListItem(
                overlineContent = row.id?.let { { Text(it.toString()) } },
                headlineContent = { Text(row.description) },
                supportingContent = {
                    val detail = listOfNotNull(
                        row.project,
                        row.tags.takeIf { it.isNotEmpty() }?.joinToString(" ") { "+$it" },
                        row.due?.let { "due ${isoDate(it, offset)}" },
                    ).joinToString("  ")
                    if (detail.isNotEmpty()) Text(detail)
                },
                trailingContent = { Text(((row.urgency * 10).toInt() / 10.0).toString()) },
            )
        }
    }
}

@Composable
private fun Lines(lines: List<String>) {
    LazyColumn(Modifier.fillMaxSize()) { items(lines) { Text(it) } }
}

@Composable
private fun Question(model: AppModel, pending: Pending) {
    AlertDialog(
        onDismissRequest = { model.answer(false) },
        title = { Text(pending.question.message) },
        text = {
            Column {
                pending.question.items.forEach { Text("${it.id ?: "-"}  ${it.description}: ${it.question}") }
            }
        },
        confirmButton = { TextButton(onClick = { model.answer(true) }) { Text("Yes") } },
        dismissButton = { TextButton(onClick = { model.answer(false) }) { Text("No") } },
    )
}
