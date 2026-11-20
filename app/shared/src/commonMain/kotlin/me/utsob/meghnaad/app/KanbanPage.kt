package me.utsob.meghnaad.app

import androidx.compose.foundation.border
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

// The Kanban page on the phone (KanbanPage.svelte, spec 0026): the list of kanbans, or one of them. Each column is the kanban's
// report run over its filters, so which task is where, in what order and with which fields is the engine's answer; a task is in
// the first column that has it, and Other holds what the report picks and no column does. A card's Move to menu runs that
// column's On move on it.

@Composable
fun KanbanPage(store: Store) {
    val kanban = store.kanbans.firstOrNull { it.id == store.kanbanId }
    when {
        !store.kanbanSettings.enabled -> Column(Modifier.fillMaxSize().padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text("Kanban", fontSize = 20.sp, fontWeight = FontWeight.SemiBold)
            Dim("Kanban is off.")
            Button(onClick = { store.openSettings(SettingsTab.Kanban) }) { Text("Set it up in Settings → Kanban") }
        }
        kanban == null -> KanbanList(store)
        else -> KanbanScreen(store, kanban)
    }
}

@Composable
private fun KanbanList(store: Store) {
    val colors = MaterialTheme.colorScheme
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Text("Kanbans", fontSize = 20.sp, fontWeight = FontWeight.SemiBold)
        if (store.kanbans.isEmpty()) Dim("No kanbans yet.")
        for (k in store.kanbans) {
            Row(
                Modifier.fillMaxWidth().border(1.dp, colors.outlineVariant, ControlShape).clickable {
                    store.kanbanReport = null
                    store.kanbanId = k.id
                }.padding(12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                LucideIcon("square-kanban", size = 18.dp)
                Text(k.name, Modifier.weight(1f), fontWeight = FontWeight.SemiBold)
                Dim("on ${k.report}, ${k.columns.size} ${if (k.columns.size == 1) "column" else "columns"}", size = 12)
            }
        }
        TextButton(onClick = { store.openSettings(SettingsTab.Kanban) }) { Text("New kanban, or edit them in Settings") }
    }
}

private class KanbanData(val placed: Placed, val meta: List<Column>)

@Composable
private fun KanbanScreen(store: Store, kanban: Kanban) {
    val colors = MaterialTheme.colorScheme
    val scope = rememberCoroutineScope()
    val report = store.kanbanReport ?: kanban.report
    val filter = store.kanbanFilters[kanban.id] ?: kanban.filter
    val since = store.kanbanSince[kanban.id] ?: kanban.completedSince
    var data by remember(kanban.id) { mutableStateOf<KanbanData?>(null) }
    var error by remember(kanban.id) { mutableStateOf<String?>(null) }
    var otherOpen by remember(kanban.id) { mutableStateOf(false) }

    suspend fun load() {
        try {
            val (placed, meta) = store.loadKanban(kanban, KanbanView(report, filter, since))
            data = KanbanData(placed, meta)
            error = null
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        }
    }

    LaunchedEffect(kanban, report, filter, since, store.rev) {
        // Typing waits until it stops.
        delay(300)
        load()
    }

    fun move(uuid: String, from: Int, to: Int) {
        val col = kanban.columns.getOrNull(to) ?: return
        if (col.move.isBlank() || from == to) return
        scope.launch {
            val ok = store.actSteps(null, moveSteps(col.move, uuid, store.taskCommands))
            load()
            val d = data ?: return@launch
            if (!ok) return@launch
            val at = columnOf(d.placed.columns, uuid)
            if (at != to) {
                val where = if (at >= 0) kanban.columns[at].name else if (d.placed.other.any { it.uuid == uuid }) "Other" else null
                store.notify(
                    if (where != null) "${col.move} ran, but the task is in $where, not ${col.name}: ${col.name}’s filter doesn’t match it."
                    else "${col.move} ran, and the task is no longer on this kanban.",
                    bad = true,
                )
            }
        }
    }

    val ctx = store.cellCtx(report)
    Column(Modifier.fillMaxSize().padding(horizontal = 12.dp).padding(top = 10.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(kanban.name, Modifier.weight(1f), fontSize = 20.sp, fontWeight = FontWeight.SemiBold)
            TextButton(onClick = { store.openSettings(SettingsTab.Kanban) }) { Text("Edit") }
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text("Report", fontSize = 13.sp)
            val names = store.offeredReports.map { it.name }.let { if (report in it) it else listOf(report) + it }
            Choice(report, names, hints = names.map { if (it == kanban.report) "default" else null }) { n ->
                store.kanbanReport = names[n].takeIf { it != kanban.report }
            }
        }
        val plain = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None)
        val mono = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 13.sp)
        OutlinedTextField(filter, { store.kanbanFilters[kanban.id] = it }, Modifier.fillMaxWidth(), singleLine = true, textStyle = mono, keyboardOptions = plain, label = { Text("Filter") })
        OutlinedTextField(
            since, { store.kanbanSince[kanban.id] = it }, Modifier.fillMaxWidth(), singleLine = true, textStyle = mono, keyboardOptions = plain,
            label = { Text("Completed since") }, placeholder = { Text("today-14d", fontSize = 13.sp) },
        )
        error?.let { Text(it, color = colors.error, fontSize = 13.sp) }
        val d = data
        when {
            kanban.columns.isEmpty() -> Dim("This kanban has no columns yet: add them under Edit.")
            d == null -> Dim("Loading…")
            else -> BoxWithConstraints(Modifier.weight(1f)) {
              // The columns share the width, each between a card's least and most comfortable width; past that the row scrolls.
              val n = kanban.columns.size + if (otherOpen) 1 else 0
              val rest = if (otherOpen) 0.dp else 170.dp
              val columnWidth = ((maxWidth - rest - 10.dp * n) / n.coerceAtLeast(1)).coerceIn(260.dp, 440.dp)
              Row(
                Modifier.fillMaxSize().horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
                verticalAlignment = Alignment.Top,
            ) {
                kanban.columns.forEachIndexed { i, col ->
                    val rows = d.placed.columns.getOrElse(i) { emptyList() }
                    KanbanColumnView(col.name, rows.size, if (col.move.isBlank()) "Takes no cards: it has no On move." else null, columnWidth) {
                        val cells = if (rows.isEmpty()) emptyList() else formatTable(d.meta, rows, ctx).cells
                        rows.forEachIndexed { j, row -> KanbanCard(store, kanban, d.meta, row, cells.getOrElse(j) { emptyList() }, i, ::move) }
                    }
                }
                Column(Modifier.width(if (otherOpen) columnWidth else 160.dp).let { if (otherOpen) it.fillMaxHeight() else it }.background(colors.surfaceVariant.copy(alpha = 0.4f), ControlShape).padding(10.dp)) {
                    Row(Modifier.clickable { otherOpen = !otherOpen }, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        LucideIcon(if (otherOpen) "chevron-down" else "chevron-right", size = 14.dp)
                        Text("Other", fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
                        Dim("${d.placed.other.size}", size = 12)
                    }
                    if (otherOpen) {
                        Dim("What the report picks and no column does. Takes no cards.", size = 12)
                        val cells = if (d.placed.other.isEmpty()) emptyList() else formatTable(d.meta, d.placed.other, ctx).cells
                        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                            d.placed.other.forEachIndexed { j, row -> KanbanCard(store, kanban, d.meta, row, cells.getOrElse(j) { emptyList() }, -1, ::move) }
                        }
                    }
                }
                Spacer(Modifier.width(4.dp))
              }
            }
        }
    }
}

@Composable
private fun KanbanColumnView(name: String, count: Int, note: String?, width: Dp, cards: @Composable () -> Unit) {
    val colors = MaterialTheme.colorScheme
    // As tall as the page allows; the cards scroll inside it.
    Column(Modifier.width(width).fillMaxHeight().background(colors.surfaceVariant.copy(alpha = 0.4f), ControlShape).padding(10.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(name, fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
            Dim("$count", size = 12)
        }
        note?.let { Dim(it, size = 12) }
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) { cards() }
    }
}

/**
 * A card, drawn as the report's cards are (TaskCard): a finished task tinted, a priority in its own colour, tags as chips,
 * and the colour the table lays on the task's row (color.active, color.overdue, ...) as the card's edge.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun KanbanCard(store: Store, kanban: Kanban, meta: List<Column>, row: Row, cells: List<Cell>, from: Int, move: (String, Int, Int) -> Unit) {
    val colors = MaterialTheme.colorScheme
    var menu by remember { mutableStateOf(false) }
    val targets = kanban.columns.withIndex().filter { (i, c) -> i != from && c.move.isNotBlank() }
    val open = row.status == "pending"
    val overdue = open && row.due != null && row.due < store.now
    val panel = colors.surface
    val background = when (row.status) {
        "completed" -> extras.ok.copy(alpha = 0.09f).compositeOver(panel)
        "deleted" -> colors.error.copy(alpha = 0.09f).compositeOver(panel)
        else -> panel
    }
    val edge = row.style?.tintSource(extras.dark)
    Column(
        Modifier.fillMaxWidth().clip(ControlShape).background(background)
            .drawBehind { edge?.let { drawRect(it, size = Size(3.dp.toPx(), size.height)) } }
            .border(1.dp, edge ?: colors.outlineVariant, ControlShape)
            .clickable { store.openDetail(row.uuid) }.padding(start = 10.dp, end = 8.dp, top = 8.dp, bottom = 8.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        val at = meta.indexOfFirst { it.name == "description" }
        Text(
            cells.getOrNull(at)?.text ?: row.description,
            fontWeight = if (row.style?.bold == true) FontWeight.Bold else FontWeight.SemiBold,
            fontSize = 14.sp,
            color = if (open) colors.onSurface else colors.onSurfaceVariant,
        )
        val marks = meta.zip(cells).filter { (col, c) -> col.name != "description" && (c.text.isNotBlank() || !c.chips.isNullOrEmpty()) }
        if (marks.isNotEmpty()) {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(5.dp)) {
                for ((col, c) in marks) Mark(store, col, c, row, overdue, emptyList(), null, !open)
            }
        }
        if (targets.isNotEmpty()) {
            Box {
                TextButton(onClick = { menu = true }) { Text("Move to…", fontSize = 12.sp) }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    for ((i, c) in targets) {
                        DropdownMenuItem(text = { Text(c.name) }, onClick = {
                            menu = false
                            move(row.uuid, from, i)
                        })
                    }
                }
            }
        }
    }
}
