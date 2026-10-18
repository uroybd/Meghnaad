package me.utsob.meghnaad.app

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
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
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull

// Boards on the phone (BoardsPage.svelte, BoardView.svelte, BoardSection.svelte, SectionInfo.svelte): a page of cards, each a
// number, a table or a chart over the tasks its filter picks, on the phone's 4 or 6 columns (BoardGrid.kt). Boards are made and changed here too (BoardEditor.kt); they are in `settings.json`,
// which the phone keeps in step with the bucket, so what is changed on one device is on the others after a sync.

@Composable
fun BoardsPage(store: Store) {
    // Leaving the page drops a draft that was not saved.
    DisposableEffect(Unit) { onDispose { store.boardEditor = null } }
    val board = store.boards.firstOrNull { it.id == store.boardId }
    val editor = store.boardEditor
    when {
        editor != null -> BoardEditScreen(store, editor) { store.boardEditor = null }
        board == null -> BoardList(store)
        else -> BoardScreen(store, board)
    }
}

/** What the sections of [board] show, asked again when what a section asks for changes (not when it only moves) and when a task changes anywhere. */
@Composable
fun rememberBoardData(store: Store, board: Board, wait: Boolean): Map<String, SectionData> {
    var data by remember { mutableStateOf<Map<String, SectionData>>(emptyMap()) }
    val asks = listOf(board.filter, board.sections.map { listOf(it.id, it.type, it.report, it.filter, it.ignoreBoardFilter, it.measure, it.by, it.series) })
    LaunchedEffect(asks, store.rev) {
        // While a board is being edited, a keystroke in a filter is not a question.
        if (wait) delay(250)
        data = store.loadBoardData(board)
    }
    return data
}

@Composable
private fun BoardList(store: Store) {
    val colors = MaterialTheme.colorScheme
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Text("Boards", fontSize = 20.sp, fontWeight = FontWeight.SemiBold)
        Dim("A board is a page of numbers, tables and charts, each with its own filter. They are kept in the app's own settings, which sync with your other devices.")
        val scope = rememberCoroutineScope()
        var newName by remember { mutableStateOf("") }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(newName, { newName = it }, Modifier.weight(1f), singleLine = true, placeholder = { Text("Name of a new board", fontSize = 13.sp) })
            Button({
                val name = newName.trim()
                scope.launch {
                    createBoard(store, name)?.let { made ->
                        newName = ""
                        store.boardId = made.id
                        store.boardEditor = BoardEditor(store, made)
                    }
                }
            }, enabled = newName.isNotBlank()) { Text("New board") }
        }
        store.appSettings?.boardErrors?.takeIf { it.isNotEmpty() }?.let { problems ->
            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text("Some of what is saved for boards was not understood and is left out until it is fixed:", color = colors.error, fontSize = 13.sp)
                problems.forEach { Text("• $it", color = colors.error, fontSize = 12.sp) }
            }
        }
        if (store.boards.isEmpty()) Dim("No boards yet.")
        store.boards.forEach { b ->
            Card(Modifier.fillMaxWidth().clickable { store.boardId = b.id }) {
                Row(Modifier.padding(12.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    LucideIcon("layout-dashboard", size = 18.dp)
                    Column(Modifier.weight(1f)) {
                        Text(b.name, fontWeight = FontWeight.SemiBold, fontSize = 15.sp)
                        b.description?.let { Dim(it) }
                    }
                    Dim("${b.sections.size} ${if (b.sections.size == 1) "section" else "sections"}")
                }
            }
        }
    }
}

@Composable
private fun BoardScreen(store: Store, board: Board) {
    val data = rememberBoardData(store, board, wait = false)
    var info by remember(board) { mutableStateOf<String?>(null) }

    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 12.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            IconAction("chevron-left", "All boards", { store.boardId = null })
            Text(board.name, Modifier.weight(1f), fontSize = 20.sp, fontWeight = FontWeight.SemiBold)
            val clipboard = LocalClipboardManager.current
            IconAction("pencil", "Edit this board", { store.boardEditor = BoardEditor(store, board) }, size = 20.dp)
            IconAction("link", "Copy a link to this board", { clipboard.setText(AnnotatedString(placeLink(AppPlace.Boards(board.id)))); store.notify("Link copied.") }, size = 20.dp)
        }
        board.description?.let { Dim(it, Modifier.padding(horizontal = 12.dp)) }
        if (board.sections.isEmpty()) Dim("This board has no sections yet. Edit it to add some.", Modifier.padding(horizontal = 12.dp))
        BoardGrid(store, board.sections, data, onInfo = { info = it })
    }

    info?.let { id ->
        val section = board.sections.firstOrNull { it.id == id }
        val got = data[id]
        if (section != null && got != null) SectionInfoDialog(store, section, got) { info = null }
    }
}

// ---- The info of a card

/** The most tasks listed for one filter; the count says how many there are. */
private const val LISTED = 50

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SectionInfoDialog(store: Store, section: Section, data: SectionData, onDismiss: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val sources = when (data) {
        is SectionData.Number -> data.sources
        is SectionData.Chart -> data.sources
        else -> emptyList()
    }
    val chart = (data as? SectionData.Chart)?.let { orderChart(it.chart, section) }
    val settings = store.config?.config?.settings
    val dateFmt = formatFor(FormatKind.Report, settings)
    val tz = store.utcOffset
    val columns = remember(section) {
        (listOf("id", "description", "project", "status", "tags") + seriesOf(section).map { (it.measure ?: section.measure).field } + section.by?.field + section.by?.time?.field)
            .filterNotNull().distinct()
    }

    fun openInTasks(filter: String) {
        store.filter = filter
        store.view = View.Tasks
        onDismiss()
    }

    /** Look at one of these tasks: close this panel and open the task's detail. */
    fun openTask(uuid: String) {
        onDismiss()
        store.openDetail(uuid)
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        confirmButton = { TextButton(onDismiss) { Text("Close") } },
        title = { Text(section.title) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                InfoHeading("What it shows")
                if (section.type == "table") InfoLine("Report", section.report ?: "next") else InfoLine("Measure", describeMeasure(section.measure))
                describeBy(section.by)?.let { InfoLine("Grouped by", it) }
                if (chart != null) {
                    InfoLine("Order", (section.sort ?: if (section.by?.time != null) "in time order" else "largest first") + (section.limit?.let { ", the first $it" } ?: "") + if (section.stacked == true) ", stacked" else "")
                }

                if (data is SectionData.Failed) {
                    InfoHeading("Problem")
                    Text(data.message, color = colors.error, fontSize = 13.sp)
                } else {
                    InfoHeading("Filter")
                }
                if (data is SectionData.Table) {
                    FilterLine(null, data.filter, null) { openInTasks(data.filter) }
                    ((data.result as? CliResult.Report)?.value?.matched)?.let { Dim("The report matched $it ${if (it == 1) "task" else "tasks"}.") }
                }
                val series = seriesOf(section)
                sources.forEachIndexed { i, src ->
                    FilterLine(src.label.takeIf { sources.size > 1 }, src.filter, series.getOrNull(i)?.span) { openInTasks(src.filter) }
                }

                if (data is SectionData.Number) {
                    InfoHeading("Result")
                    Text("${formatBoardValue(data.answer.total, data.answer.unit)} from ${data.answer.tasks} ${if (data.answer.tasks == 1) "task" else "tasks"}.", fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
                }
                if (chart != null) {
                    InfoHeading("Axis values")
                    Row(Modifier.fillMaxWidth()) {
                        Dim(describeBy(section.by) ?: "Label", Modifier.weight(1.4f), size = 11)
                        chart.datasets.forEach { Dim(it.label, Modifier.weight(1f), size = 11) }
                    }
                    chart.labels.forEachIndexed { i, l ->
                        Row(Modifier.fillMaxWidth()) {
                            Text(l.label, Modifier.weight(1.4f), fontSize = 12.sp)
                            chart.datasets.forEach { d ->
                                Text("${formatBoardValue(d.data.getOrNull(i), chart.unit)} (${d.counts.getOrElse(i) { 0 }})", Modifier.weight(1f), fontSize = 12.sp)
                            }
                        }
                    }
                    Row(Modifier.fillMaxWidth()) {
                        Text("Over all", Modifier.weight(1.4f), fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
                        chart.datasets.forEach { d ->
                            Text("${formatBoardValue(d.total, chart.unit)} (${d.tasks})", Modifier.weight(1f), fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
                        }
                    }
                    Dim("Each value is the measure of that label's tasks; the number in brackets is how many tasks they are.", size = 11)
                }

                if (sources.isNotEmpty()) InfoHeading("Tasks the filter got")
                sources.forEach { src ->
                    val tasks = remember(src) { parseExportedTasks(src.text) }
                    if (sources.size > 1) Text(src.label, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
                    Dim("${tasks.size} ${if (tasks.size == 1) "task" else "tasks"}", size = 12)
                    tasks.take(LISTED).forEach { t ->
                        val uuid = (t["uuid"] as? JsonPrimitive)?.contentOrNull
                        Column(if (uuid != null) Modifier.fillMaxWidth().clickable { openTask(uuid) } else Modifier) {
                            Text("${exportedCell(t, "id") { "" }.let { if (it == "–") "" else "$it  " }}${exportedCell(t, "description") { it.toString() }}", fontSize = 13.sp)
                            val rest = columns.filter { it != "id" && it != "description" }.map { c -> c to exportedCell(t, c) { epoch -> formatMoment(epoch, tz, dateFmt) } }.filter { it.second != "–" }
                            if (rest.isNotEmpty()) Dim(rest.joinToString(" · ") { "${it.first}: ${it.second}" }, size = 11)
                        }
                    }
                    if (tasks.size > LISTED) Dim("and ${tasks.size - LISTED} more (open them in Tasks to see all).", size = 11)
                }
            }
        },
    )
}

@Composable
private fun InfoHeading(text: String) {
    Text(text.uppercase(), Modifier.padding(top = 8.dp), fontSize = 11.sp, fontWeight = FontWeight.SemiBold, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

@Composable
private fun InfoLine(label: String, value: String) {
    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Dim(label, Modifier.weight(0.7f), size = 13)
        Text(value, Modifier.weight(1.3f), fontSize = 13.sp)
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun FilterLine(label: String?, filter: String, span: StateSpan?, onOpen: () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        label?.let { Text(it, fontSize = 13.sp, fontWeight = FontWeight.SemiBold) }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(2.dp), itemVerticalAlignment = Alignment.CenterVertically) {
            Text(filter.ifEmpty { "(every task)" }, fontFamily = FontFamily.Monospace, fontSize = 12.sp)
            TextButton(onOpen) { Text("Open in Tasks", fontSize = 12.sp) }
        }
        span?.let { Dim(describeSpan(it), size = 11) }
    }
}
