package me.utsob.meghnaad.app

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** A task's state as a coloured pill: each state has its own colour (StatusPill.svelte). */
@Composable
fun StatusPill(kind: String, label: String = kind) {
    val x = extras
    val colors = MaterialTheme.colorScheme
    val s = if (x.dark) 0.60f else 0.55f
    val l = if (x.dark) 0.70f else 0.36f
    val c = when (kind) {
        "pending" -> colors.primary
        "completed" -> x.ok
        "deleted" -> colors.error
        "recurring" -> Color.hsl(265f, s, l)
        "waiting" -> x.warn
        "blocked" -> Color.hsl(25f, s, l)
        "blocking" -> Color.hsl(180f, s, l)
        else -> colors.onSurface
    }
    Text(
        label,
        color = c,
        fontSize = 12.5.sp,
        modifier = Modifier.clip(CircleShape).background(c.copy(alpha = 0.13f)).border(1.dp, c.copy(alpha = 0.35f), CircleShape).padding(horizontal = 9.dp, vertical = 1.dp),
    )
}

/** What to know of another task at a glance: its number and state, what it is, and where it sits (TaskCard.svelte). */
@Composable
fun MiniCard(store: Store, row: Row) {
    val kind = if (row.status == "pending" && ("WAITING" in row.virtualTags || row.waiting)) "waiting" else row.status
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            row.id?.let { Dim("#$it", size = 13) }
            StatusPill(kind)
        }
        Text(row.description, fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
        val meta = listOfNotNull(
            row.project,
            row.priority?.let { "priority $it" },
            row.due?.let { "due ${formatMoment(it, store.offsetAt(it))}" },
            row.tags.takeIf { it.isNotEmpty() }?.joinToString(" ") { "+$it" },
        ).joinToString(" · ")
        if (meta.isNotEmpty()) Dim(meta, size = 12)
    }
}

/**
 * Another task as a line you can tap: its name as far as the app knows it, and on a tap a card of it, fetched only then, with a
 * button that opens it (DepItem.svelte).
 */
@Composable
fun DepItem(store: Store, uuid: String, row: Row? = null, onOpen: (String) -> Unit) {
    var shown by androidx.compose.runtime.remember { androidx.compose.runtime.mutableStateOf(false) }
    var fetched by androidx.compose.runtime.remember(uuid) { androidx.compose.runtime.mutableStateOf<Row?>(null) }
    var missing by androidx.compose.runtime.remember(uuid) { androidx.compose.runtime.mutableStateOf(false) }
    val card = row ?: fetched
    val name = card?.description ?: store.tasks.firstOrNull { it.uuid == uuid }?.description
    androidx.compose.runtime.LaunchedEffect(shown) {
        if (shown && card == null) {
            fetched = store.fetchTask(uuid)
            missing = fetched == null
        }
    }
    Column(Modifier.padding(vertical = 2.dp)) {
        Row(
            Modifier.clip(androidx.compose.foundation.shape.RoundedCornerShape(8.dp)).clickable { shown = !shown }.padding(vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            LucideIcon(if (shown) "chevron-down" else "chevron-right", size = 14.dp)
            Text(name ?: "Task", fontSize = 14.sp)
            Text(shortUuid(uuid), fontFamily = FontFamily.Monospace, fontSize = 12.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        if (shown) {
            Card(Modifier.padding(start = 20.dp)) {
                Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    when {
                        card != null -> MiniCard(store, card)
                        missing -> Dim("This task is not here any more.")
                        else -> Dim("Loading…")
                    }
                    androidx.compose.material3.TextButton(onClick = { onOpen(uuid) }) { Text("Open task") }
                }
            }
        }
    }
}

/**
 * Another task named by its uuid (a recurring task's parent): its short id, which on a tap shows a small card of the task with a
 * button that opens it (DepLink.svelte).
 */
@Composable
fun DepLink(store: Store, uuid: String, onOpen: (String) -> Unit) {
    var shown by androidx.compose.runtime.remember { androidx.compose.runtime.mutableStateOf(false) }
    Pill(shortUuid(uuid), mono = true, onClick = { shown = true })
    if (shown) {
        var card by androidx.compose.runtime.remember(uuid) { androidx.compose.runtime.mutableStateOf<Row?>(null) }
        var missing by androidx.compose.runtime.remember(uuid) { androidx.compose.runtime.mutableStateOf(false) }
        androidx.compose.runtime.LaunchedEffect(uuid) {
            card = store.fetchTask(uuid)
            missing = card == null
        }
        androidx.compose.material3.AlertDialog(
            onDismissRequest = { shown = false },
            text = {
                val shownCard = card
                when {
                    shownCard != null -> MiniCard(store, shownCard)
                    missing -> Dim("This task is not here any more.", size = 14)
                    else -> Dim("Loading…", size = 14)
                }
            },
            confirmButton = { androidx.compose.material3.TextButton(onClick = { shown = false; onOpen(uuid) }) { Text("Open task") } },
            dismissButton = { androidx.compose.material3.TextButton(onClick = { shown = false }) { Text("Close") } },
        )
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun Chips(content: @Composable () -> Unit) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(4.dp), itemVerticalAlignment = Alignment.CenterVertically) { content() }
}
