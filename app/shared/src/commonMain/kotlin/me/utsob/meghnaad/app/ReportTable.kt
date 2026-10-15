package me.utsob.meghnaad.app

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
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
import androidx.compose.runtime.remember
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
import androidx.compose.ui.unit.Dp
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

/** What a table keeps for the tick box and for the row's buttons, which the columns share the rest of the row besides. */
val TABLE_SELECT = SELECT_WIDTH
val TABLE_ACTIONS = ACTIONS_WIDTH

/** What each column asks for, in dp: its widest cell or its header, whichever is wider, so a narrow column keeps its text on one line. */
private fun preferredWidths(result: ReportResult, ctx: Ctx): List<Float> = shownColumns(result, ctx).map { ci ->
    val col = result.columns[ci]
    var longest = 0
    for (cells in cellsOf(result, ctx)) {
        val c = cells[ci]
        val n = when {
            c.chips != null -> c.chips.sumOf { it.length + 3 }
            c.segments != null -> c.segments.sumOf { it.length } + c.segments.size
            c.links != null -> c.links.size * 9
            else -> c.text.lines().maxOfOrNull { it.length } ?: 0
        }
        if (n > longest) longest = n
    }
    val body = longest * 7.4f + 18f + if (col.kind == "number") 6f else 0f
    val header = col.label.length * 7.2f + 32f // its label, the sort arrow and its padding
    when (col.kind) {
        "description" -> maxOf(240f, minOf(body, 480f), header)
        "notes" -> maxOf(200f, minOf(body, 400f), header)
        "priority" -> maxOf(68f, body, header)
        else -> maxOf(48f, body, header)
    }
}

/**
 * How wide each column is, in dp, in [available] (what is left of the row after the tick box and the buttons). The columns take what
 * their cells hold and the description takes the rest, as a table does. When the cells hold more than the row has, the description
 * keeps a width of its own and the table scrolls sideways, with the tick box and the buttons kept in place (the web's does). A table
 * cannot measure its cells when each row is its own item of a lazy list, so the widths are worked out once.
 */
fun tableWidths(result: ReportResult, ctx: Ctx, available: Float): List<Dp> {
    val pref = preferredWidths(result, ctx)
    val flex = shownColumns(result, ctx).indexOfFirst { result.columns[it].kind == "description" }
    val others = pref.filterIndexed { i, _ -> i != flex }.sum()
    return when {
        flex >= 0 -> pref.mapIndexed { i, w -> (if (i == flex) maxOf(available - others, 240f) else w).dp }
        pref.sum() < available -> pref.map { (it * available / pref.sum()).dp }
        else -> pref.map { it.dp }
    }
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
    widths: List<Dp>,
    scroll: ScrollState,
    heads: List<String?>,
    entry: Entry?,
    activeTags: List<String>,
    onTag: (String) -> Unit,
    onEdit: (Row) -> Unit,
    onSort: (column: String, tieBreak: Boolean) -> Unit,
) {
    val keys = parseSort(result.sort)
    item(key = "table-head") { TableHeader(store, result, ctx, keys, widths, scroll, selectable = true, onSort) }
    items(result.rows.size, key = { result.rows[it].uuid }) { i ->
        val row = result.rows[i]
        heads.getOrNull(i)?.let { GroupHead(it) }
        TableRow(store, result, ctx, widths, scroll, row, entry, selectable = true, activeTags, onTag, onEdit, gapBefore = heads.isEmpty() && result.breaks.getOrNull(i) == true)
    }
}

/**
 * The same table where nothing is sorted, grouped or ticked: a report the console prints, or the tasks due in the months of a
 * calendar (the web's ReportTable with only the report). A tag chip opens the tag's entry on the Tags page.
 */
@Composable
fun ReportTableBlock(store: Store, result: ReportResult, ctx: Ctx, heads: List<String?>, entry: Entry?, onEdit: (Row) -> Unit) {
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val widths = remember(result, maxWidth) { tableWidths(result, ctx, (maxWidth - ACTIONS_WIDTH).value) }
        val scroll = rememberScrollState()
        Column(Modifier.fillMaxWidth()) {
            TableHeader(store, result, ctx, parseSort(result.sort), widths, scroll, selectable = false, onSort = null)
            result.rows.forEachIndexed { i, row ->
                heads.getOrNull(i)?.let { GroupHead(it) }
                TableRow(store, result, ctx, widths, scroll, row, entry, selectable = false, emptyList(), null, onEdit, gapBefore = heads.isEmpty() && result.breaks.getOrNull(i) == true)
            }
        }
    }
}

/** The heading above a group of rows: what its tasks have in common, in the accent colour with a line under it. */
@Composable
private fun GroupHead(text: String) {
    Column(Modifier.fillMaxWidth().padding(top = 14.dp)) {
        Text(text, Modifier.padding(bottom = 4.dp), color = MaterialTheme.colorScheme.primary, fontWeight = FontWeight.Bold, fontSize = 13.sp)
        HorizontalDivider(color = MaterialTheme.colorScheme.primary)
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun TableHeader(store: Store, result: ReportResult, ctx: Ctx, keys: List<SortKey>, widths: List<Dp>, scroll: ScrollState, selectable: Boolean, onSort: ((String, Boolean) -> Unit)?) {
    val colors = MaterialTheme.colorScheme
    val udas = store.config?.config?.udas ?: emptyMap()
    val all = result.rows.map { it.uuid }
    val ticked = all.count { store.selection.has(it) }
    Column(Modifier.fillMaxWidth()) {
        Row(Modifier.fillMaxWidth().padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            if (selectable) Box(Modifier.width(SELECT_WIDTH), contentAlignment = Alignment.Center) {
                TriStateCheckbox(
                    state = when {
                        all.isNotEmpty() && ticked == all.size -> ToggleableState.On
                        ticked == 0 -> ToggleableState.Off
                        else -> ToggleableState.Indeterminate
                    },
                    onClick = { store.selection.toggleAll(all) },
                )
            }
            Row(Modifier.weight(1f).horizontalScroll(scroll), verticalAlignment = Alignment.CenterVertically) {
            shownColumns(result, ctx).forEachIndexed { pos, ci ->
                val col = result.columns[ci]
                val st = sortState(keys, col.name)
                val sortable = onSort != null && (baseColumn(col.name) in SORTABLE || baseColumn(col.name) in udas)
                Row(Modifier.width(widths[pos]).padding(end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    val tint = if (st != null) colors.primary else colors.onSurfaceVariant
                    Row(
                        Modifier.clip(ControlShape).let { m ->
                            if (sortable) m.combinedClickable(onClick = { onSort?.invoke(col.name, false) }, onLongClick = { onSort?.invoke(col.name, true) }) else m
                        }.padding(horizontal = 4.dp, vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(col.label, color = tint, fontSize = 13.sp, fontWeight = if (st != null) FontWeight.Bold else FontWeight.Medium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        if (st != null) {
                            LucideIcon(if (st.desc) "arrow-down" else "arrow-up", Modifier.padding(start = 2.dp), size = 12.dp, tint = tint)
                            st.rank?.let { Text(it.toString(), color = tint, fontSize = 10.sp) }
                        }
                    }
                    // The column the table is grouped by is marked (the web's mark); the Group picker above chooses it.
                    if (st?.group == true) LucideIcon("rows-3", Modifier.padding(start = 4.dp), size = 12.dp, tint = colors.primary)
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
    widths: List<Dp>,
    scroll: ScrollState,
    row: Row,
    entry: Entry?,
    selectable: Boolean,
    activeTags: List<String>,
    onTag: ((String) -> Unit)?,
    onEdit: (Row) -> Unit,
    gapBefore: Boolean,
) {
    val colors = MaterialTheme.colorScheme
    val scope = rememberCoroutineScope()
    val open = row.status == "pending"
    val overdue = open && row.due != null && row.due < store.now
    val picked = selectable && store.selection.has(row.uuid)
    val done = row.status == "completed" || row.status == "deleted" || row.waiting
    val bold = row.style?.bold == true
    val cells = cellsOfRow(result, ctx, row)
    Column(Modifier.fillMaxWidth().padding(top = if (gapBefore) 14.dp else 0.dp)) {
        Row(
            Modifier.fillMaxWidth().background(rowBackground(row, picked)).clickable { store.openDetail(row.uuid, entry) },
            verticalAlignment = Alignment.Top,
        ) {
            if (selectable) Box(Modifier.width(SELECT_WIDTH).padding(top = 2.dp), contentAlignment = Alignment.Center) {
                Checkbox(checked = picked, onCheckedChange = { store.selection.toggle(row.uuid) })
            }
            Row(Modifier.weight(1f).horizontalScroll(scroll), verticalAlignment = Alignment.Top) {
            shownColumns(result, ctx).forEachIndexed { pos, ci ->
                val col = result.columns[ci]
                val c = cells[ci]
                Column(Modifier.width(widths[pos]).padding(end = 8.dp, top = 11.dp, bottom = 9.dp)) {
                    TableCell(store, col, c, row, overdue, done, bold, activeTags, onTag)
                    if (col.name == "description") c.notes?.forEach { NoteLine(it, Modifier.padding(top = 2.dp)) }
                }
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
    onTag: ((String) -> Unit)?,
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
                    onClick = { if (onTag != null) onTag(t) else store.showTag(t) },
                )
            }
        }
        c.blocks != null -> MarkdownView(
            c.blocks, color = tone, size = 13.5.sp,
            onToggle = { line -> store.go { tickBox(null, row.uuid, col.name, c.text, line) } },
        )
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
