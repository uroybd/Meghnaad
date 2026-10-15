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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

// One task as a card, on a phone: the cells of the report's own columns (the report definition says what is shown, as
// Taskwarrior's does), the description first on a line of its own and the task's number in the corner, then every other
// column as a mark, in exactly the order the report defines them. The user's
// colour rules are only a faint wash behind the card; the priority is a stripe down its left edge.
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
    // The priority, as a stripe down the left edge (a started task's is the accent).
    val stripe = when {
        started -> colors.primary
        row.priority == "H" -> x.priorityHigh
        row.priority == "M" -> x.priorityMedium
        row.priority == "L" -> x.priorityLow
        else -> null
    }
    val done = !open
    val textColor = if (done) colors.onSurfaceVariant else colors.onSurface
    val bold = row.style?.bold == true

    val cells = report.columns.map { it to cell(it, row, ctx) }
    val marks = cells.filter { (col, c) -> col.name != "description" && col.name != "id" && !(c.text.isEmpty() && c.lines.isNullOrEmpty()) }

    Column(Modifier.padding(top = if (gapBefore) 14.dp else 0.dp, bottom = 10.dp)) {
        Box(
            Modifier.fillMaxWidth()
                .clip(RoundedCornerShape(14.dp))
                .background(background)
                .drawBehind { stripe?.let { drawRect(it, size = Size(4.dp.toPx(), size.height)) } }
                .border(BorderStroke(1.dp, if (picked) colors.primary else colors.outline), RoundedCornerShape(14.dp))
                .clickable { store.openDetail(row.uuid, from) },
        ) {
            Column(Modifier.fillMaxWidth().padding(start = 15.dp, end = 12.dp, top = 12.dp, bottom = 6.dp)) {
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
                            Text(
                                description?.text ?: row.description,
                                color = textColor,
                                fontSize = 15.5.sp,
                                lineHeight = 21.sp,
                                fontWeight = if (bold) FontWeight.Bold else FontWeight.Medium,
                                modifier = Modifier.weight(1f, fill = false),
                            )
                            if (row.orphans.isNotEmpty()) Pill("+${row.orphans.size}", height = 18.dp)
                        }
                        description?.lines?.forEach { line -> Dim(line, Modifier.padding(top = 2.dp)) }
                    }
                    // The task's number, a small badge in the corner.
                    cells.firstOrNull { it.first.name == "id" }?.second?.takeIf { it.text.isNotEmpty() }?.let { id ->
                        Pill((if (row.id != null) "#" else "") + id.text, Modifier.padding(start = 8.dp), mono = false, height = 20.dp, color = colors.onSurfaceVariant)
                    }
                }
                if (marks.isNotEmpty()) {
                    FlowRow(
                        Modifier.fillMaxWidth().padding(top = 8.dp),
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                        verticalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        for ((col, c) in marks) Mark(store, col, c, row, overdue, activeTags, onTag, done)
                    }
                }
                Actions(store, row, from, onEdit, ctx, scope)
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Mark(
    store: Store,
    col: Column,
    c: Cell,
    row: Row,
    overdue: Boolean,
    activeTags: List<String>,
    onTag: ((String) -> Unit)?,
    done: Boolean,
) {
    val colors = MaterialTheme.colorScheme
    val dim = colors.onSurfaceVariant
    val icon = markIcon(col)
    val iconOnly = col.format in ICON_ONLY
    when {
        col.name == "priority" && row.priority != null -> {
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
        c.chips != null && !iconOnly -> FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
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
            LucideIcon("folder", size = 13.dp, tint = dim.copy(alpha = 0.85f))
            ProjectPath(c.segments, onPick = { store.showProject(it) })
        }
        else -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
            if (icon != null) LucideIcon(icon, size = 13.dp, tint = if (col.name == "due" && overdue) colors.error else dim.copy(alpha = 0.85f))
            else Text(col.label.uppercase(), fontSize = 11.sp, letterSpacing = 0.6.sp, color = dim.copy(alpha = 0.75f))
            if (!iconOnly) {
                Text(
                    c.text,
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

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Actions(store: Store, row: Row, from: Entry?, onEdit: ((Row) -> Unit)?, ctx: Ctx, scope: kotlinx.coroutines.CoroutineScope) {
    val colors = MaterialTheme.colorScheme
    var sure by remember(row.uuid) { mutableStateOf(false) }
    val open = row.status == "pending"
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End, verticalAlignment = Alignment.CenterVertically) {
        if (open) {
            IconAction("check", "Done", { store.go { act(from, listOf(row.uuid, "done")) } }, tint = extras.ok)
            if (row.start != null) {
                IconAction("square", "Stop", { store.go { act(from, listOf(row.uuid, "stop")) } }, size = 15.dp)
            } else {
                IconAction("play", "Start", { store.go { act(from, listOf(row.uuid, "start")) } }, size = 15.dp)
            }
        }
        if (onEdit != null) IconAction("pencil", "Edit", { onEdit(row) }, size = 15.dp)
        if (row.status != "deleted") {
            // The second tap is the answer to Taskwarrior's "Delete task N?" (the `confirmation` setting); with it off, the
            // first tap deletes.
            if (sure) {
                Text(
                    "sure?",
                    color = colors.error,
                    fontSize = 13.sp,
                    modifier = Modifier.clip(CircleShape).clickable {
                        sure = false
                        store.go { act(from, listOf(row.uuid, "delete"), Answers(approved = listOf(row.uuid))) }
                    }.padding(horizontal = 10.dp, vertical = 10.dp),
                )
            } else {
                IconAction("trash-2", "Delete", {
                    if (!store.confirmation) {
                        store.go { act(from, listOf(row.uuid, "delete"), Answers(approved = listOf(row.uuid))) }
                    } else {
                        sure = true
                        scope.launch { delay(3000); sure = false }
                    }
                }, tint = colors.error, size = 15.dp)
            }
        }
        MoreMenu(store.taskCommands) { c -> store.go { act(from, listOf(row.uuid) + commandWords(c)) } }
    }
}

/** The task commands from `settings.json` under one "⋯" button, so the buttons for the everyday actions stay few. */
@Composable
fun MoreMenu(commands: List<TaskCommand>, label: String = "More actions", enabled: Boolean = true, onRun: (TaskCommand) -> Unit) {
    if (commands.isEmpty()) return
    var open by remember { mutableStateOf(false) }
    Box {
        IconAction("ellipsis", label, { open = true }, enabled = enabled)
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            for (c in commands) {
                DropdownMenuItem(
                    leadingIcon = { CommandIcon(c.icon, 15) },
                    text = {
                        Column {
                            Text(c.name, fontSize = 14.sp)
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
