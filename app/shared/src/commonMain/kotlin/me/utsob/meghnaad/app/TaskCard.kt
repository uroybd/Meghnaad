package me.utsob.meghnaad.app

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

// One task as a card, on a phone: the cells of the report's own columns (the report definition says what is shown, as
// Taskwarrior's does), the description first on a line of its own and the task's number in the corner, then every other
// column as a mark, in exactly the order the report defines them. The user's
// colour rules (rule.precedence.color) are a faint wash behind the card; the priority is a ribbon down its left edge, in
// the colour the taskrc gives it (color.uda.priority.*), on every front end.
// Mirrors the phone layout of web/src/lib/ReportTable.svelte.

/** The small picture a mark has instead of a label, by the column it is. */
private fun markIcon(col: Column): String? = when {
    col.name == "project" -> "folder"
    col.name == "due" -> "clock"
    col.name == "priority" -> "flag"
    col.name == "urgency" -> "zap"
    col.name == "scheduled" -> "calendar"
    // A column that shows only an indicator letter: its name and its letter would read "R R", so a picture of what it means.
    col.name == "recur" && col.format == "indicator" -> "repeat"
    col.name == "depends" && col.format == "indicator" -> "link"
    col.name == "tags" && col.format == "indicator" -> "tag"
    col.name == "start" && col.format == "active" -> "play"
    else -> null
}

private val ICON_ONLY = setOf("indicator", "active")

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun TaskCard(
    store: Store,
    row: Row,
    report: ReportResult,
    ctx: Ctx,
    from: Entry?,
    selectable: Boolean,
    activeTags: List<String> = emptyList(),
    onTag: ((String) -> Unit)? = null,
    onEdit: ((Row) -> Unit)? = null,
    gapBefore: Boolean = false,
) = TaskCard(store, row, report.columns, cellsOfRow(report, ctx, row), ctx, from, selectable, activeTags, onTag, onEdit, gapBefore)

/**
 * The card of a task, from its columns and their cells: the report's list and the kanban's lanes. A kanban passes
 * [kanban]: its fields drawn as it asks (icons or labels, a UDA as a pill, a free-text one as a paragraph under its
 * name), and [below] for its own buttons.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun TaskCard(
    store: Store,
    row: Row,
    columns: List<Column>,
    rowCells: List<Cell>,
    ctx: Ctx,
    from: Entry?,
    selectable: Boolean,
    activeTags: List<String> = emptyList(),
    onTag: ((String) -> Unit)? = null,
    onEdit: ((Row) -> Unit)? = null,
    gapBefore: Boolean = false,
    kanban: Boolean = false,
    below: @Composable () -> Unit = {},
) {
    val colors = MaterialTheme.colorScheme
    val x = extras
    val dark = x.dark
    val scope = rememberCoroutineScope()
    val open = row.status == "pending"
    val started = row.start != null && row.end == null
    val overdue = open && row.due != null && row.due < store.now
    val picked = selectable && store.selection.has(row.uuid)

    // What the card sits on: the user's colour as a faint wash if a rule applies, else the state's own.
    val panel = colors.surfaceContainerLow
    val wash = row.style.tint(dark, panel)
    val background = wash ?: when {
        row.status == "completed" -> x.ok.copy(alpha = 0.09f).compositeOver(panel)
        row.status == "deleted" -> colors.error.copy(alpha = 0.09f).compositeOver(panel)
        started -> colors.primary.copy(alpha = 0.07f).compositeOver(panel)
        else -> panel
    }
    // The priority, as a ribbon down the left edge: in the colour the taskrc gives it, else the app's own.
    val stripe = row.priority?.let { p ->
        store.config?.colors?.get("uda.priority.$p")?.tintSource(dark) ?: when (p) {
            "H" -> x.priorityHigh
            "M" -> x.priorityMedium
            "L" -> x.priorityLow
            else -> null
        }
    }
    val done = !open
    val textColor = if (done) colors.onSurfaceVariant else colors.onSurface
    val bold = row.style?.bold == true

    val cells = columns.zip(rowCells)
    val marks = cells.filter { (col, c) -> col.name != "description" && col.name != "id" && !(c.text.isEmpty() && c.notes.isNullOrEmpty() && c.chips.isNullOrEmpty()) }
    // A field that is a few lines, not a mark: Markdown in a UDA, or on a kanban any free-text UDA.
    val udas = store.config?.config?.udas.orEmpty()
    fun paragraph(col: Column, c: Cell) = if (kanban) cardShape(col.name, udas, c.text, c.blocks) == CardShape.Text else c.blocks != null && inlineText(c.blocks) == null
    // Appearance's `cardIcons`: every field by its icon, a UDA as a pill. Off, a report's card shows a few by an icon and
    // the kanban's each by its label.
    val cardIcons = store.appearance.cardIcons
    val icons = if (cardIcons) true else if (kanban) false else null
    // A UDA as a pill while Icons in cards is on (off, its label then its value), but not one drawn as a progress bar:
    // that is the bar after its label.
    fun pill(col: Column, c: Cell) = cardIcons && cardShape(col.name, udas, c.text, c.blocks) == CardShape.Pill && store.appearance.progress[col.name] == null

    Column(Modifier.padding(top = if (gapBefore) 14.dp else 0.dp, bottom = if (kanban) 0.dp else 10.dp)) {
        Box(
            Modifier.fillMaxWidth()
                .clip(PanelShape)
                .background(background)
                .drawBehind { stripe?.let { drawRect(it, size = Size(4.dp.toPx(), size.height)) } }
                .border(BorderStroke(1.dp, if (picked) colors.primary else colors.outline), PanelShape)
                .clickable { store.openDetail(row.uuid, from) },
        ) {
            Column(Modifier.fillMaxWidth().padding(start = 15.dp, end = 8.dp, top = 10.dp, bottom = 10.dp)) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Top) {
                    if (selectable) {
                        Checkbox(
                            checked = picked,
                            onCheckedChange = { store.selection.toggle(row.uuid) },
                            modifier = Modifier.offset(x = (-9).dp, y = (-7).dp).size(33.dp),
                        )
                    }
                    Column(Modifier.weight(1f)) {
                        val description = cells.firstOrNull { it.first.name == "description" }?.second
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            DescriptionText(
                                description?.text ?: row.description,
                                description?.blocks,
                                Modifier.weight(1f, fill = false),
                                color = textColor,
                                fontSize = 15.5.sp,
                                lineHeight = 21.sp,
                                fontWeight = if (bold) FontWeight.Bold else FontWeight.Medium,
                            )
                            if (row.orphans.isNotEmpty()) Pill("+${row.orphans.size}", height = 18.dp)
                        }
                        description?.notes?.forEach { NoteLine(it, Modifier.padding(top = 2.dp)) }
                    }
                    // The task's number, a small badge in the corner.
                    cells.firstOrNull { it.first.name == "id" }?.second?.takeIf { it.text.isNotEmpty() }?.let { id ->
                        Pill((if (row.id != null) "#" else "") + id.text, Modifier.padding(start = 8.dp), mono = false, height = 20.dp, color = colors.onSurfaceVariant)
                    }
                    // Done and a ⋯ for the rest, on the title's line: the card has no row of buttons of its own.
                    Actions(store, row, from, onEdit, ctx, scope, compact = true, modifier = Modifier.offset(y = (-6).dp))
                }
                if (marks.isNotEmpty()) {
                    FlowRow(
                        Modifier.fillMaxWidth().padding(top = 8.dp),
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                        verticalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        for ((col, c) in marks) {
                            if (!paragraph(col, c)) Mark(store, col, c, row, overdue, activeTags, onTag, done, icons, pill = pill(col, c))
                        }
                    }
                    // A UDA written in Markdown is a few lines, not a mark: it gets the width of the card, in a faint frame of its own so
                    // it reads as a field beneath the task and not as more of the task's line, and its boxes work.
                    for ((col, c) in marks) if (paragraph(col, c)) {
                        Column(
                            Modifier.fillMaxWidth().padding(top = 8.dp)
                                .border(BorderStroke(1.dp, colors.outline.copy(alpha = 0.35f)), ControlShape)
                                .padding(start = 10.dp, end = 10.dp, top = 6.dp, bottom = 8.dp),
                        ) {
                            // On a kanban, headed by its name, as the web's card has it.
                            if (kanban) Text(col.label, fontSize = 14.sp, fontWeight = FontWeight.Bold, color = textColor) else Dim(col.label, size = 12)
                            if (c.blocks != null) {
                                MarkdownView(
                                    c.blocks, color = if (done) colors.onSurfaceVariant else colors.onSurface, size = 13.5.sp,
                                    onToggle = { line -> store.go { tickBox(from, row.uuid, col.name, c.text, line) } },
                                )
                            } else {
                                Text(c.text, fontSize = 13.5.sp, color = textColor)
                            }
                        }
                    }
                }
                below()
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun Mark(
    store: Store,
    col: Column,
    c: Cell,
    row: Row,
    overdue: Boolean,
    activeTags: List<String>,
    onTag: ((String) -> Unit)?,
    done: Boolean,
    /**
     * Appearance's `cardIcons`: true, every one of Taskwarrior's fields by its icon; false (a kanban's card with it off),
     * each by its report label. Null, as a report's card has it off: a few by an icon, the rest by label.
     */
    icons: Boolean? = null,
    /** A UDA as a pill of its label and value (a kanban's card, or any with `cardIcons`). */
    pill: Boolean = false,
) {
    val colors = MaterialTheme.colorScheme
    val dim = colors.onSurfaceVariant
    val icon = when (icons) {
        null -> markIcon(col)
        true -> fieldIcon(col.name)
        false -> null
    }
    val iconOnly = icons == null && col.format in ICON_ONLY
    // A label, or the icon, before a mark that draws only its value (chips, a project path) where the kanban asks.
    @Composable
    fun lead() {
        when {
            icons == null -> {}
            icon != null -> LucideIcon(icon, size = 13.dp, tint = dim.copy(alpha = 0.85f))
            else -> Text(col.label, fontSize = 12.sp, color = dim.copy(alpha = 0.8f))
        }
    }
    when {
        pill -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
            // A UDA: its label in bold, then its value, in line as a tag or a project is; a number without the zeros it
            // ends in, a choice as its own pill (spec 0034). One drawn as a progress bar is never this: it is the bar after
            // its label.
            Text(col.label, fontSize = 12.sp, fontWeight = FontWeight.SemiBold, color = dim)
            val numeric = store.config?.config?.udas?.get(baseColumn(col.name))?.type == "numeric"
            when {
                c.choice != null -> ChoicePill(store, col.name, c.text, c.choice)
                // One line of Markdown (a ticket key a transformation made a link) is drawn as it shows, its link kept.
                c.blocks != null -> MarkdownView(c.blocks, color = colors.onSurface, size = 12.5.sp)
                else -> Text(if (numeric) trimNumber(c.text) else c.text, fontSize = 12.5.sp, color = colors.onSurface, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
        }
        col.name == "priority" && row.priority != null && icons != false -> {
            // The priority as a small pill of its own colour: the user's `color.uda.priority.*` washes it, the text stays readable.
            val style = store.config?.colors?.get("uda.priority.${row.priority}")
            val tint = style?.tintSource(extras.dark) ?: dim
            Row(
                Modifier.clip(CircleShape).background(tint.copy(alpha = 0.16f)).padding(start = 6.dp, end = 8.dp, top = 1.dp, bottom = 1.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                LucideIcon("flag", size = 13.dp, tint = tint)
                Text(c.text, fontSize = 13.sp, fontWeight = FontWeight.Bold, color = colors.onSurface)
            }
        }
        c.choice != null && !iconOnly -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
            // Its label as the card's other fields have it, then the value as a pill (spec 0034).
            if (icons == null) Text(col.label.uppercase(), fontSize = 11.sp, letterSpacing = 0.6.sp, color = dim.copy(alpha = 0.75f)) else lead()
            ChoicePill(store, col.name, c.text, c.choice)
        }
        c.chips != null && !iconOnly -> FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp), itemVerticalAlignment = Alignment.CenterVertically) {
            lead()
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
        c.segments != null -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
            if (icons == null) LucideIcon("folder", size = 13.dp, tint = dim.copy(alpha = 0.85f)) else lead()
            ProjectPath(c.segments, onPick = { store.showProject(it) })
        }
        else -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
            // A numeric UDA the settings show as a progress bar (Settings → Appearance) is a bar here too, after its label.
            val bar = store.appearance.progress[col.name]?.let { range -> c.text.trim().toDoubleOrNull()?.let { range to it } }
            if (icon != null) LucideIcon(icon, size = 13.dp, tint = if (col.name == "due" && overdue) colors.error else dim.copy(alpha = 0.85f))
            else if (icons == false) Text(col.label, fontSize = 12.sp, color = dim.copy(alpha = 0.8f))
            else Text(col.label.uppercase(), fontSize = 11.sp, letterSpacing = 0.6.sp, color = dim.copy(alpha = 0.75f))
            if (bar != null) {
                ProgressBar(bar.second, trimNumber(c.text), bar.first, col.label, dim, Modifier.width(120.dp))
            } else if (!iconOnly && c.blocks != null) {
                MarkdownView(c.blocks, color = dim, size = 13.sp)
            } else if (!iconOnly) {
                Text(
                    if (store.config?.config?.udas?.get(baseColumn(col.name))?.type == "numeric") trimNumber(c.text) else c.text,
                    fontSize = 13.sp,
                    color = if (col.name == "due" && overdue) colors.error else dim,
                    fontWeight = if (col.name == "due" && overdue) FontWeight.SemiBold else FontWeight.Normal,
                    fontFamily = if (col.name == "urgency") FontFamily.Monospace else null,
                    maxLines = if (c.cls == "multiline") 4 else 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

/** A project path, `Home.Kitchen`, with the dots between its parts muted; each part names the path up to it. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun ProjectPath(segments: List<String>, modifier: Modifier = Modifier, onPick: ((String) -> Unit)? = null) {
    val dim = MaterialTheme.colorScheme.onSurfaceVariant
    Row(modifier, verticalAlignment = Alignment.CenterVertically) {
        segments.forEachIndexed { i, s ->
            if (i > 0) Text(".", color = dim.copy(alpha = 0.55f), fontSize = 13.sp)
            val part = Modifier.let { m -> if (onPick != null) m.clickable { onPick(segments.take(i + 1).joinToString(".")) } else m }
            Text(s, part, fontSize = 13.sp, color = dim)
        }
    }
}

/**
 * A task's buttons. [compact] is for a card: only Done (the everyday one) shows, and Start or Stop, Edit and Delete are in the
 * ⋯ menu with the task commands.
 */
@Composable
fun Actions(
    store: Store,
    row: Row,
    from: Entry?,
    onEdit: ((Row) -> Unit)?,
    ctx: Ctx,
    scope: kotlinx.coroutines.CoroutineScope,
    compact: Boolean = false,
    modifier: Modifier = Modifier,
) {
    val colors = MaterialTheme.colorScheme
    var sure by remember(row.uuid) { mutableStateOf(false) }
    val open = row.status == "pending"
    val box = if (compact) 36.dp else 44.dp
    // The second tap is the answer to Taskwarrior's "Delete task N?" (the `confirmation` setting); with it off, the first tap
    // deletes.
    val delete: () -> Unit = {
        if (!store.confirmation) {
            store.go { act(from, listOf(row.uuid, "delete"), Answers(approved = listOf(row.uuid))) }
        } else {
            sure = true
            scope.launch { delay(3000); sure = false }
        }
    }
    val confirm: () -> Unit = {
        sure = false
        store.go { act(from, listOf(row.uuid, "delete"), Answers(approved = listOf(row.uuid))) }
    }
    val start: () -> Unit = { store.go { act(from, listOf(row.uuid, if (row.start != null) "stop" else "start")) } }
    Row(modifier.then(if (compact) Modifier else Modifier.fillMaxWidth()), horizontalArrangement = Arrangement.End, verticalAlignment = Alignment.CenterVertically) {
        if (open) {
            IconAction("check", "Done", { store.go { act(from, listOf(row.uuid, "done")) } }, tint = extras.ok, box = box)
            if (!compact) {
                if (row.start != null) IconAction("square", "Stop", start, size = 15.dp) else IconAction("play", "Start", start, size = 15.dp)
            }
        }
        if (!compact) {
            if (onEdit != null) IconAction("pencil", "Edit", { onEdit(row) }, size = 15.dp)
            if (row.status != "deleted") {
                if (sure) {
                    Text(
                        "sure?",
                        color = colors.error,
                        fontSize = 13.sp,
                        modifier = Modifier.clip(CircleShape).clickable { confirm() }.padding(horizontal = 10.dp, vertical = 10.dp),
                    )
                } else {
                    IconAction("trash-2", "Delete", { delete() }, tint = colors.error, size = 15.dp)
                }
            }
        }
        val own = buildList {
            if (compact) {
                if (open) add(MenuAction(if (row.start != null) "Stop" else "Start", if (row.start != null) "square" else "play", run = start))
                if (onEdit != null) add(MenuAction("Edit", "pencil") { onEdit(row) })
                if (row.status != "deleted") {
                    add(MenuAction(if (sure) "Tap again to delete" else "Delete", "trash-2", danger = true, stay = store.confirmation && !sure) { if (sure) confirm() else delete() })
                }
            }
        }
        MoreMenu(store.menuCommands, extra = own, box = box) { c -> store.go { actSteps(from, commandSteps(c).map { it.copy(words = listOf(row.uuid) + it.words) }) } }
    }
}

/** One of the app's own buttons, folded into a [MoreMenu]. [stay] leaves the menu open after it runs (a delete that asks twice). */
class MenuAction(val label: String, val icon: String, val danger: Boolean = false, val stay: Boolean = false, val run: () -> Unit)

/** The task commands from `settings.json` under one "⋯" button, so the buttons for the everyday actions stay few; [extra] are buttons that had no room. */
@Composable
fun MoreMenu(
    commands: List<TaskCommand>,
    label: String = "More actions",
    enabled: Boolean = true,
    extra: List<MenuAction> = emptyList(),
    box: Dp = 44.dp,
    onRun: (TaskCommand) -> Unit,
) {
    if (commands.isEmpty() && extra.isEmpty()) return
    var open by remember { mutableStateOf(false) }
    Box {
        IconAction("ellipsis", label, { open = true }, enabled = enabled, box = box)
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            for (a in extra) {
                DropdownMenuItem(
                    leadingIcon = { LucideIcon(a.icon, size = 15.dp, tint = if (a.danger) MaterialTheme.colorScheme.error else LocalContentColor.current) },
                    text = { Text(a.label, fontSize = 14.sp, color = if (a.danger) MaterialTheme.colorScheme.error else Color.Unspecified) },
                    onClick = {
                        if (!a.stay) open = false
                        a.run()
                    },
                )
            }
            if (extra.isNotEmpty() && commands.isNotEmpty()) HorizontalDivider()
            for (c in commands) {
                DropdownMenuItem(
                    leadingIcon = { CommandIcon(c.icon, 15) },
                    text = {
                        Column {
                            Text(commandLabel(c), fontSize = 14.sp)
                            Text(c.command, fontSize = 11.sp, fontFamily = FontFamily.Monospace, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    },
                    onClick = {
                        open = false
                        onRun(c)
                    },
                )
            }
        }
    }
}

/** A command's icon: the Lucide icon it names, or a terminal when it names none (or one that does not exist). */
@Composable
fun CommandIcon(name: String?, size: Int = 16) {
    val shown = if (name != null && LucideLib.has(name)) name else "terminal"
    LucideIcon(shown, size = size.dp)
}
