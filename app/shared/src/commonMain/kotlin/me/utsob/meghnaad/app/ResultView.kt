package me.utsob.meghnaad.app

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TriStateCheckbox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.toMutableStateList
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.state.ToggleableState
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
            is CliResult.Summary -> SummaryView(store, result.value)
            is CliResult.Calendar -> CalendarView(store, e, result.value)
            is CliResult.Burndown -> BurndownView(store, result.value)
            is CliResult.Styled -> StyledView(result.value)
            is CliResult.File -> FileView(store, result.value)
            is CliResult.Import -> ImportView(store)
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
    // A table where there is room for one (the web's width), cards where there is not.
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        if (maxWidth > TABLE_FROM) {
            ReportTableBlock(store, result, ctx, heads, e) { store.editing = EditRef(it, e) }
        } else Column(Modifier.fillMaxWidth()) {
            result.rows.forEachIndexed { i, row ->
                heads.getOrNull(i)?.let { Text(it, Modifier.padding(top = 10.dp, bottom = 4.dp), fontWeight = FontWeight.SemiBold, fontSize = 13.sp, color = MaterialTheme.colorScheme.onSurfaceVariant) }
                TaskCard(store, row, result, ctx, from = e, selectable = false, onEdit = { store.editing = EditRef(it, e) }, gapBefore = heads.isEmpty() && result.breaks.getOrNull(i) == true)
            }
        }
    }
    Dim(
        if (result.rows.size == result.matched) "${result.matched} ${if (result.matched == 1) "task" else "tasks"}" else "showing ${result.rows.size} of ${result.matched}",
    )
}

/**
 * Taskwarrior wants an answer: a plain yes or no, or a question per task the command would change (tick the ones to go ahead
 * with), or follow-ups that only arise once those are answered (ConfirmView.svelte). One question is a yes or no; several are
 * a list with a tick each, where ticking all is Taskwarrior's "all" and ticking none is its "quit".
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ConfirmView(store: Store, e: Entry, q: ConfirmResult) {
    // You typed the command, so yes is the usual answer: everything starts ticked, untick the exceptions.
    val ticked = remember(q) { q.items.map { true }.toMutableStateList() }
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(q.message, fontSize = 14.sp, fontWeight = FontWeight.Medium)
        when {
            q.ask == "plain" -> Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = { store.go { confirm(e, true) } }) { Text("Yes") }
                OutlinedButton(onClick = { store.go { confirm(e, false) } }) { Text("No") }
            }
            q.items.size == 1 -> Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = { store.go { answerItems(e, listOf(q.items[0].key)) } }) { Text("Yes") }
                OutlinedButton(onClick = { store.go { answerItems(e, emptyList()) } }) { Text("No") }
            }
            else -> {
                val count = ticked.count { it }
                fun setAll(on: Boolean) = ticked.indices.forEach { ticked[it] = on }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    TriStateCheckbox(
                        state = when (count) { q.items.size -> ToggleableState.On; 0 -> ToggleableState.Off; else -> ToggleableState.Indeterminate },
                        onClick = { setAll(count != q.items.size) },
                    )
                    Dim("Select all or none")
                }
                q.items.forEachIndexed { i, item ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Checkbox(ticked[i], { ticked[i] = it })
                        Column {
                            Text("${item.id ?: item.uuid.take(8)}  ${item.description}", fontSize = 14.sp)
                            // The question is only worth reading for the follow-ups: for the rest it is the same on every line.
                            if (q.ask == "extras") Dim(item.question)
                        }
                    }
                }
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(2.dp), itemVerticalAlignment = Alignment.CenterVertically) {
                    TextButton(onClick = { setAll(true) }) { Text("Select all") }
                    TextButton(onClick = { setAll(false) }) { Text("Select none") }
                    TextButton(onClick = { store.go { confirm(e, false) } }) { Text("Cancel") }
                    Button(onClick = { store.go { answerItems(e, q.items.filterIndexed { i, _ -> ticked[i] }.map { it.key }) } }) {
                        Text("Go ahead with $count of ${q.items.size}")
                    }
                }
            }
        }
    }
}
