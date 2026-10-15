package me.utsob.meghnaad.app

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Checkbox
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TriStateCheckbox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

// The report as a table, when the window has room for one (the web's, from 761 px): the report's own columns, a header that sorts
// and groups, a tick box on each row, and the row's buttons at the end. A narrower window shows cards instead (TaskCard.kt).
// Mirrors the table of web/src/lib/ReportTable.svelte. Every row is its own item of the page's list, so a long report stays lazy,
// which is why the columns share the width by weight rather than by what the cells hold.

/** The width, from which a table is drawn rather than cards: the web's breakpoint. */
val TABLE_FROM = 760.dp

private val SELECT_WIDTH = 40.dp
private val ACTIONS_WIDTH = 190.dp

/** How much of the row a column asks for, in proportion to the others. */
private fun share(col: Column): Float = when {
    col.kind == "description" -> 3.4f
    col.kind == "notes" -> 2.4f
    col.kind == "id" -> 0.5f
    col.kind == "priority" -> 0.7f
    col.kind == "project" -> 1.6f
    col.kind == "tags" -> 1.7f
    col.kind == "uuids" -> 1.1f
    col.kind == "status" -> 0.9f
    col.kind == "number" -> 0.8f
    col.format in setOf("indicator", "active") -> 0.45f
    else -> 1.1f
}

/** A row's own colour: the user's rule as a faint wash if one applies, else what its state calls for, else the page. */
@Composable
private fun rowBackground(row: Row, picked: Boolean): Color {
    val colors = MaterialTheme.colorScheme
    val page = colors.background
    val x = extras
    val started = row.start != null && row.end == null
    return when {
        picked -> colors.primary.copy(alpha = 0.10f).compositeOver(page)
        else -> row.style.tint(x.dark, page) ?: when {
            row.status == "completed" -> x.ok.copy(alpha = 0.09f).compositeOver(page)
            row.status == "deleted" -> colors.error.copy(alpha = 0.09f).compositeOver(page)
            started -> colors.primary.copy(alpha = 0.07f).compositeOver(page)
            else -> Color.Transparent
        }
    }
}

/**
 * The header, a row for each task (with a heading above each group), all as items of the page's list. [onSort] is a header tap
 * (a long press adds a tie-breaker, the web's Shift-click); [onGroup] is the grouping button of a column, null for none.
 */
@OptIn(ExperimentalLayoutApi::class)
fun LazyListScope.reportTable(
    store: Store,
    result: ReportResult,
    ctx: Ctx,
    heads: List<String?>,
    entry: Entry?,
    activeTags: List<String>,
    onTag: (String) -> Unit,
    onEdit: (Row) -> Unit,
    onSort: (column: String, tieBreak: Boolean) -> Unit,
    onGroup: (column: String) -> Unit,
) {
    val keys = parseSort(result.sort)
    item(key = "table-head") { TableHeader(store, result, keys, onSort, onGroup) }
    items(result.rows.size, key = { result.rows[it].uuid }) { i ->
        val row = result.rows[i]
        heads.getOrNull(i)?.let { head ->
            Column(Modifier.fillMaxWidth().padding(top = 14.dp)) {
                Text(head, Modifier.padding(bottom = 4.dp), color = MaterialTheme.colorScheme.primary, fontWeight = FontWeight.Bold, fontSize = 13.sp)
                HorizontalDivider(color = MaterialTheme.colorScheme.primary)
            }
        }
        TableRow(store, result, ctx, row, entry, activeTags, onTag, onEdit, gapBefore = heads.isEmpty() && result.breaks.getOrNull(i) == true)
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun TableHeader(store: Store, result: ReportResult, keys: List<SortKey>, onSort: (String, Boolean) -> Unit, onGroup: (String) -> Unit) {
    val colors = MaterialTheme.colorScheme
    val udas = store.config?.config?.udas ?: emptyMap()
    val all = result.rows.map { it.uuid }
    val ticked = all.count { store.selection.has(it) }
    Column(Modifier.fillMaxWidth()) {
        Row(Modifier.fillMaxWidth().padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.width(SELECT_WIDTH), contentAlignment = Alignment.Center) {
                TriStateCheckbox(
                    state = when {
                        all.isNotEmpty() && ticked == all.size -> ToggleableState.On
                        ticked == 0 -> ToggleableState.Off
                        else -> ToggleableState.Indeterminate
                    },
                    onClick = { store.selection.toggleAll(all) },
                )
            }
            for (col in result.columns) {
                val st = sortState(keys, col.name)
                val sortable = baseColumn(col.name) in SORTABLE || baseColumn(col.name) in udas
                Row(Modifier.weight(share(col)).padding(end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    val tint = if (st != null) colors.primary else colors.onSurfaceVariant
                    Row(
                        Modifier.clip(ControlShape).let { m ->
                            if (sortable) m.combinedClickable(onClick = { onSort(col.name, false) }, onLongClick = { onSort(col.name, true) }) else m
                        }.padding(horizontal = 4.dp, vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(col.label, color = tint, fontSize = 13.sp, fontWeight = if (st != null) FontWeight.Bold else FontWeight.Medium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        if (st != null) {
                            LucideIcon(if (st.desc) "arrow-down" else "arrow-up", Modifier.padding(start = 2.dp), size = 12.dp, tint = tint)
                            st.rank?.let { Text(it.toString(), color = tint, fontSize = 10.sp) }
                        }
                    }
                    if (sortable) {
                        IconAction("rows-3", if (st?.group == true) "Stop grouping by ${col.label}" else "Group by ${col.label}", { onGroup(baseColumn(col.name)) }, size = 12.dp, box = 28.dp, tint = if (st?.group == true) colors.primary else colors.onSurfaceVariant.copy(alpha = 0.6f))
                    }
                }
            }
            Box(Modifier.width(ACTIONS_WIDTH))
        }
        HorizontalDivider(color = colors.outline)
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun TableRow(
    store: Store,
    result: ReportResult,
    ctx: Ctx,
    row: Row,
    entry: Entry?,
    activeTags: List<String>,
    onTag: (String) -> Unit,
    onEdit: (Row) -> Unit,
    gapBefore: Boolean,
) {
    val colors = MaterialTheme.colorScheme
    val scope = rememberCoroutineScope()
    val open = row.status == "pending"
    val overdue = open && row.due != null && row.due < store.now
    val picked = store.selection.has(row.uuid)
    val done = row.status == "completed" || row.status == "deleted" || row.waiting
    val bold = row.style?.bold == true
    Column(Modifier.fillMaxWidth().padding(top = if (gapBefore) 14.dp else 0.dp)) {
        Row(
            Modifier.fillMaxWidth().background(rowBackground(row, picked)).clickable { store.openDetail(row.uuid, entry) },
            verticalAlignment = Alignment.Top,
        ) {
            Box(Modifier.width(SELECT_WIDTH).padding(top = 2.dp), contentAlignment = Alignment.Center) {
                Checkbox(checked = picked, onCheckedChange = { store.selection.toggle(row.uuid) })
            }
            for (col in result.columns) {
                val c = cell(col, row, ctx)
                Column(Modifier.weight(share(col)).padding(end = 8.dp, top = 11.dp, bottom = 9.dp)) {
                    TableCell(store, col, c, row, overdue, done, bold, activeTags, onTag)
                    if (col.name == "description") c.lines?.forEach { Dim(it, Modifier.padding(top = 2.dp)) }
                }
            }
            Box(Modifier.width(ACTIONS_WIDTH)) { Actions(store, row, entry, onEdit, ctx, scope) }
        }
        HorizontalDivider(color = colors.outlineVariant.copy(alpha = 0.6f))
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun TableCell(
    store: Store,
    col: Column,
    c: Cell,
    row: Row,
    overdue: Boolean,
    done: Boolean,
    bold: Boolean,
    activeTags: List<String>,
    onTag: (String) -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val x = extras
    val clipboard = LocalClipboardManager.current
    val dim = colors.onSurfaceVariant
    val base = if (done) dim else colors.onSurface
    // The cell's own class says how it reads: a priority, an urgency band, or a quiet value.
    val (tone, weight) = when (c.cls) {
        "dim" -> dim to FontWeight.Normal
        "pri-h" -> x.priorityHigh to FontWeight.SemiBold
        "pri-m" -> x.priorityMedium to FontWeight.SemiBold
        "pri-l" -> x.priorityLow to FontWeight.SemiBold
        "urg-low" -> dim to FontWeight.Normal
        "urg-mid" -> base to FontWeight.Medium
        "urg-high" -> x.warn to FontWeight.SemiBold
        "urg-crit" -> colors.error to FontWeight.Bold
        else -> base to if (col.kind == "id") FontWeight.Bold else if (bold) FontWeight.Bold else FontWeight.Normal
    }
    when {
        c.uuid != null -> Text(
            c.text, Modifier.clickable { clipboard.setText(AnnotatedString(c.uuid)); store.notify("uuid copied") },
            color = tone, fontSize = 13.sp, fontFamily = FontFamily.Monospace,
        )
        c.links != null -> FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            for (u in c.links) DepLink(store, u) { id -> store.openDetail(id, null) }
        }
        c.segments != null -> ProjectPath(c.segments, onPick = { store.showProject(it) })
        c.chips != null -> FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            for (t in c.chips) {
                val on = t in activeTags
                Pill(
                    t,
                    height = 21.dp,
                    background = if (on) colors.primary.copy(alpha = 0.18f) else colors.surfaceVariant,
                    color = if (on) colors.primary else colors.onSurface,
                    onClick = { onTag(t) },
                )
            }
        }
        col.name == "priority" && row.priority != null -> {
            // The priority as a small pill of its own colour: the user's `color.uda.priority.*` washes it, the text stays readable.
            val style = store.config?.colors?.get("uda.priority.${row.priority}")
            val pill = style?.tintSource(x.dark) ?: tone
            Row(
                Modifier.clip(CircleShape).background(pill.copy(alpha = 0.16f)).padding(start = 6.dp, end = 8.dp, top = 1.dp, bottom = 1.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                LucideIcon("flag", size = 13.dp, tint = pill)
                Text(c.text, fontSize = 13.sp, fontWeight = FontWeight.Bold, color = colors.onSurface)
            }
        }
        else -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(
                c.text,
                Modifier.weight(1f, fill = false),
                color = if (col.name == "due" && overdue) colors.error else tone,
                fontWeight = if (col.name == "due" && overdue) FontWeight.SemiBold else weight,
                fontSize = if (col.kind == "description") 14.5.sp else 13.5.sp,
                maxLines = if (c.cls == "multiline") 6 else if (col.kind == "description") 4 else 2,
                overflow = TextOverflow.Ellipsis,
            )
            if (col.name == "description" && row.recur != null) LucideIcon("repeat", size = 12.dp, tint = dim)
            if (col.name == "description" && row.orphans.isNotEmpty()) Pill("+${row.orphans.size}", height = 18.dp)
        }
    }
}
