package me.utsob.meghnaad.app

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.toMutableStateList
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json

private val prettyJson = Json { prettyPrint = true; prettyPrintIndent = "  " }

/** One command's answer, drawn by its kind: the one component the console, the panels and the detail share (ResultView.svelte). */
@Composable
fun ResultBlock(store: Store, e: Entry, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    Column(modifier.fillMaxWidth()) {
        e.failure?.let { Text(it, color = colors.error, fontSize = 14.sp) }
        val result = e.result
        if (result == null) {
            if (e.loading) Dim("…")
            return@Column
        }
        for (line in e.feedback) {
            Text(line.text, color = if (line.kind == "warn") colors.error else colors.onSurfaceVariant, fontSize = 13.sp, modifier = Modifier.padding(bottom = 2.dp))
        }
        when (result) {
            is CliResult.Report -> ReportSection(store, e, result.value)
            is CliResult.Text -> Mono(result.value.lines)
            is CliResult.Table -> TableView(result.value)
            is CliResult.Changed -> {
                Text(result.value.message, fontSize = 14.sp)
                for (t in result.value.tasks) Dim("${t.id ?: "-"}  ${t.description}")
            }
            is CliResult.Error -> Text(result.value.message, color = colors.error, fontSize = 14.sp)
            is CliResult.Confirm -> ConfirmView(store, e, result.value)
            is CliResult.Json -> Mono(prettyJson.encodeToString(kotlinx.serialization.json.JsonElement.serializer(), result.value.let { it }).lines())
            is CliResult.Info -> Dim("${result.value.tasks.size} task(s): open one to see it in full.")
            is CliResult.Unsupported -> Dim("“${result.kind}” has no screen yet.")
        }
    }
}

@Composable
private fun Mono(lines: List<String>) {
    Column(Modifier.horizontalScroll(rememberScrollState())) {
        for (l in lines) Text(l.ifEmpty { " " }, fontFamily = FontFamily.Monospace, fontSize = 12.5.sp, softWrap = false)
    }
}

/** A table of text (`show`, `columns`, `udas`…): monospace, the columns aligned as the engine says. */
@Composable
private fun TableView(t: TableResult) {
    val cols = t.headers.size
    val widths = (0 until cols).map { c -> maxOf(t.headers[c].length, t.rows.maxOfOrNull { it.getOrNull(c)?.length ?: 0 } ?: 0) }
    fun line(cells: List<String>) = (0 until cols).joinToString("  ") { c ->
        val text = cells.getOrElse(c) { "" }
        if (c in t.right) text.padStart(widths[c]) else text.padEnd(widths[c])
    }
    Column {
        t.title?.let { Text(it, fontWeight = FontWeight.SemiBold, fontSize = 14.sp, modifier = Modifier.padding(bottom = 4.dp)) }
        Column(Modifier.horizontalScroll(rememberScrollState())) {
            Text(line(t.headers), fontFamily = FontFamily.Monospace, fontSize = 12.5.sp, fontWeight = FontWeight.Bold, softWrap = false)
            t.rows.forEachIndexed { i, r ->
                Text(line(r), fontFamily = FontFamily.Monospace, fontSize = 12.5.sp, fontWeight = if (i in t.highlight) FontWeight.Bold else FontWeight.Normal, softWrap = false)
            }
        }
        for (f in t.footer) Dim(f, size = 12)
    }
}

/** A report as cards (the console prints one right where it was asked for). */
@Composable
fun ReportSection(store: Store, e: Entry, result: ReportResult) {
    val ctx = store.cellCtx(result.report)
    result.description?.let { Dim(it, Modifier.padding(bottom = 4.dp)) }
    if (result.rows.isEmpty()) {
        Dim("No tasks match.")
        return
    }
    val heads = groupHeads(result, ctx)
    result.rows.forEachIndexed { i, row ->
        heads.getOrNull(i)?.let { Text(it, Modifier.padding(top = 10.dp, bottom = 4.dp), fontWeight = FontWeight.SemiBold, fontSize = 13.sp, color = MaterialTheme.colorScheme.onSurfaceVariant) }
        TaskCard(store, row, result, ctx, from = e, selectable = false, onEdit = { store.editing = EditRef(it, e) }, gapBefore = heads.isEmpty() && result.breaks.getOrNull(i) == true)
    }
    Dim(
        if (result.rows.size == result.matched) "${result.matched} ${if (result.matched == 1) "task" else "tasks"}" else "showing ${result.rows.size} of ${result.matched}",
    )
}

/**
 * Taskwarrior wants an answer: a plain yes or no, or a question per task the command would change (tick the ones to go ahead
 * with), or follow-ups that only arise once those are answered.
 */
@Composable
private fun ConfirmView(store: Store, e: Entry, q: ConfirmResult) {
    val scope = rememberCoroutineScope()
    val ticked = remember(q) { q.items.map { true }.toMutableStateList() }
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(q.message, fontSize = 14.sp, fontWeight = FontWeight.Medium)
        if (q.ask == "plain") {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = { scope.launch { store.confirm(e, true) } }) { Text("Yes") }
                OutlinedButton(onClick = { scope.launch { store.confirm(e, false) } }) { Text("No") }
            }
        } else {
            q.items.forEachIndexed { i, item ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(ticked[i], { ticked[i] = it })
                    Column {
                        Text("${item.id ?: "-"}  ${item.description}", fontSize = 14.sp)
                        Dim(item.question)
                    }
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = { scope.launch { store.answerItems(e, q.items.filterIndexed { i, _ -> ticked[i] }.map { it.key }) } }) { Text("Go ahead") }
                OutlinedButton(onClick = { scope.launch { store.confirm(e, false) } }) { Text("Cancel") }
            }
        }
    }
}
