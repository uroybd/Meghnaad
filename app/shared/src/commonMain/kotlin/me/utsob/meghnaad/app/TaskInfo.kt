package me.utsob.meghnaad.app

import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.JsonElement

@Composable
private fun Field(label: String, content: @Composable () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(vertical = 5.dp), verticalAlignment = Alignment.Top) {
        Dim(label, Modifier.width(104.dp).padding(top = 1.dp))
        Column(Modifier.weight(1f)) { content() }
    }
}

@Composable
private fun Heading(text: String, icon: String? = null) {
    Row(Modifier.padding(top = 18.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        if (icon != null) LucideIcon(icon, size = 14.dp)
        Text(text, fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
    }
}

@Composable
private fun UrgencyText(u: Double) {
    val x = extras
    val c = when (urgencyLevel(u)) {
        UrgencyLevel.Crit -> MaterialTheme.colorScheme.error
        UrgencyLevel.High -> x.warn
        UrgencyLevel.Mid -> MaterialTheme.colorScheme.onSurface
        UrgencyLevel.Low -> x.dim
    }
    Text(fixed(u, 2), color = c, fontFamily = FontFamily.Monospace, fontSize = 14.sp, fontWeight = if (urgencyLevel(u) >= UrgencyLevel.High) FontWeight.SemiBold else FontWeight.Normal)
}

/** Everything about one task: its state, dates, tracked time, links to other tasks, UDAs, notes and history (TaskInfo.svelte). */
@Composable
fun TaskInfo(store: Store, task: Row, onOpen: (String) -> Unit) {
    val settings = store.config?.config?.settings
    val infoFmt = formatFor(FormatKind.Info, settings)
    val noteFmt = formatFor(FormatKind.InfoNote, settings)
    val defs = store.config?.config?.udas ?: emptyMap()
    val clipboard = LocalClipboardManager.current
    fun at(epoch: Long) = formatMoment(epoch, store.offsetAt(epoch), infoFmt)
    // History moments carry seconds (a row is the changes within one second); a configured `dateformat.info` wins.
    fun stamp(epoch: Long) = if (infoFmt != null) formatMoment(epoch, store.offsetAt(epoch), infoFmt) else formatStamp(epoch, store.offsetAt(epoch))
    val waiting = task.status == "pending" && task.waiting
    val udaKeys = task.extra.keys.filter { it in defs }.sorted()
    val dates = listOf("Entered" to task.entry, "Due" to task.due, "Scheduled" to task.scheduled, "Wait" to task.wait, "Until" to task.until, "Started" to task.start, "Ended" to task.end, "Modified" to task.modified)
    // The journal's "Started task"/"Stopped task" notes are bookkeeping; the sessions table shows them.
    val notes = visibleNotes(task.annotations, store.config?.journal)
    val totalTracked = task.activeSeconds ?: task.sessions.sumOf { it.seconds }

    Column(Modifier.fillMaxWidth()) {
        Field("ID") {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(task.id?.toString() ?: "–", fontWeight = if (task.id != null) FontWeight.SemiBold else FontWeight.Normal, fontSize = 14.sp)
                Text(
                    shortUuid(task.uuid),
                    fontFamily = FontFamily.Monospace,
                    fontSize = 12.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.androidxClickable { clipboard.setText(AnnotatedString(task.uuid)); store.notify("uuid copied") },
                )
            }
        }
        Field("Status") {
            Chips {
                if (waiting) StatusPill("waiting") else StatusPill(task.status)
                if (task.blocked) StatusPill("blocked")
                if (task.blocking) StatusPill("blocking")
            }
        }
        task.recur?.let { recur ->
            Field("Repeats") {
                Chips {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        LucideIcon("repeat", size = 12.dp)
                        Text(describeRecur(recur), fontSize = 14.sp)
                    }
                    if (task.status == "recurring") Pill("template")
                    task.parent?.let { DepLink(store, it, onOpen) }
                }
            }
        }
        task.project?.let { p -> Field("Project") { ProjectPath(projectSegments(p), onPick = { store.showProject(it) }) } }
        task.priority?.let { Field("Priority") { Text(it, fontSize = 14.sp, fontWeight = FontWeight.SemiBold) } }
        if (task.tags.isNotEmpty()) Field("Tags") { Chips { task.tags.forEach { t -> Pill(t, onClick = { store.showTag(t) }) } } }
        for ((label, ts) in dates) {
            if (ts != null) Field(label) {
                val overdue = label == "Due" && "OVERDUE" in task.virtualTags
                Text(at(ts), fontSize = 14.sp, color = if (overdue) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface, fontWeight = if (overdue) FontWeight.SemiBold else FontWeight.Normal)
            }
        }
        task.activeSeconds?.let { secs ->
            Field("Time tracked") {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(formatSeconds(secs), fontSize = 14.sp)
                    if (task.start != null) Pill("running", background = extras.ok.copy(alpha = 0.15f), color = extras.ok)
                }
            }
        }
        if (task.depends.isNotEmpty()) Field("Depends on") { Column { task.depends.forEach { d -> DepItem(store, d, onOpen = onOpen) } } }
        if (task.blocking) {
            var blockers by remember(task.uuid, store.rev) { mutableStateOf<List<Row>?>(null) }
            var asking by remember(task.uuid, store.rev) { mutableStateOf(false) }
            var failure by remember(task.uuid, store.rev) { mutableStateOf<String?>(null) }
            // The tasks waiting on this one are looked for when asked, not with every task shown.
            LaunchedEffect(asking) {
                if (!asking) return@LaunchedEffect
                try {
                    val r = store.query(listOf("status:pending", "depends.has:${task.uuid}", "all"))
                    if (r is CliResult.Report) blockers = r.value.rows else failure = "The tasks could not be read."
                } catch (e: Exception) {
                    failure = e.message ?: e.toString()
                }
                asking = false
            }
            Field("Blocking") {
                val list = blockers
                if (list == null) {
                    TextButton(onClick = { asking = true }, enabled = !asking, contentPadding = androidx.compose.foundation.layout.PaddingValues(0.dp)) {
                        Text(if (asking) "Loading…" else "Show the tasks waiting on this", fontSize = 13.sp)
                    }
                    failure?.let { Text(it, color = MaterialTheme.colorScheme.error, fontSize = 13.sp) }
                } else if (list.isEmpty()) {
                    Dim("Nothing is waiting on it now.")
                } else {
                    list.forEach { b -> DepItem(store, b.uuid, row = b, onOpen = onOpen) }
                }
            }
        }
        for (k in udaKeys) Field(udaLabel(defs.getValue(k))) { Text(task.extra.getValue(k), fontSize = 14.sp) }
        Field("Urgency") { UrgencyText(task.urgency) }
        if (task.virtualTags.isNotEmpty()) Field("Virtual tags") { Dim(task.virtualTags.joinToString(" "), size = 13) }

        // A recurring template lists its instances (newest first).
        if (task.status == "recurring") {
            var instances by remember(task.uuid) { mutableStateOf<List<Row>>(emptyList()) }
            LaunchedEffect(task.uuid, task.mask) {
                instances = try {
                    (store.query(listOf("parent:${task.uuid}", "all")) as? CliResult.Report)?.value?.rows ?: emptyList()
                } catch (e: Exception) {
                    emptyList()
                }
            }
            val shown = instances.sortedByDescending { it.due ?: 0 }.take(10)
            if (shown.isNotEmpty()) {
                Heading("Instances", "repeat")
                for (i in shown) {
                    Row(Modifier.padding(vertical = 2.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(
                            i.due?.let { at(it) } ?: "no due date",
                            fontSize = 14.sp,
                            color = MaterialTheme.colorScheme.primary,
                            modifier = Modifier.androidxClickable { onOpen(i.uuid) }.padding(vertical = 6.dp),
                        )
                        Pill(i.status)
                    }
                }
                if (instances.size > shown.size) Dim("+${instances.size - shown.size} older")
            }
        }

        if (task.sessions.isNotEmpty()) {
            Heading("Time sessions", "timer")
            Column(Modifier.horizontalScroll(rememberScrollState())) {
                Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    Dim("#", Modifier.width(24.dp), 12); Dim("Started", Modifier.width(150.dp), 12); Dim("Stopped", Modifier.width(150.dp), 12); Dim("Duration", Modifier.width(70.dp), 12)
                }
                task.sessions.forEachIndexed { i, s ->
                    Row(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
                        Dim("${i + 1}", Modifier.width(24.dp), 12)
                        Text(formatStamp(s.start, store.offsetAt(s.start)), Modifier.width(150.dp), fontFamily = FontFamily.Monospace, fontSize = 12.sp)
                        if (s.end != null) Text(formatStamp(s.end, store.offsetAt(s.end)), Modifier.width(150.dp), fontFamily = FontFamily.Monospace, fontSize = 12.sp)
                        else Box150 { Pill("running", background = extras.ok.copy(alpha = 0.15f), color = extras.ok) }
                        Text(formatSeconds(s.seconds), Modifier.width(70.dp), fontFamily = FontFamily.Monospace, fontSize = 12.sp)
                    }
                }
                HorizontalDivider(Modifier.padding(vertical = 4.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    Text("Total", Modifier.width(24.dp + 150.dp + 150.dp + 32.dp), fontSize = 12.sp)
                    Text(formatSeconds(totalTracked), Modifier.width(70.dp), fontFamily = FontFamily.Monospace, fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
                }
            }
        }

        if (notes.isNotEmpty()) {
            Heading("Annotations")
            for (a in notes) {
                Row(Modifier.padding(vertical = 3.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Dim(formatMoment(a.entry, store.offsetAt(a.entry), noteFmt), size = 12)
                    Text(a.text, fontSize = 14.sp)
                }
            }
        }

        // Closed until asked for, and fetched then (`_history`): the log grows with every edit, and is rarely what you opened the task for.
        var histOpen by remember(task.uuid) { mutableStateOf(false) }
        var history by remember(task.uuid) { mutableStateOf<List<HistoryEntry>?>(null) }
        var histError by remember(task.uuid) { mutableStateOf<String?>(null) }
        LaunchedEffect(task.uuid, task.modified, histOpen) {
            if (!histOpen) return@LaunchedEffect
            histError = null
            try {
                when (val r = store.query(listOf("_history", task.uuid))) {
                    is CliResult.Json -> history = engineJson.decodeFromJsonElement(ListSerializer(HistoryEntry.serializer()), r.value as JsonElement)
                    else -> histError = "The history could not be read."
                }
            } catch (e: Exception) {
                histError = e.message ?: e.toString()
            }
        }
        Row(
            Modifier.fillMaxWidth().padding(top = 14.dp).androidxClickable { histOpen = !histOpen }.padding(vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            LucideIcon(if (histOpen) "chevron-down" else "chevron-right", size = 14.dp)
            Text("History", fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
            history?.let { Dim("· ${it.size} ${if (it.size == 1) "moment" else "moments"}") }
        }
        if (histOpen) {
            val h = history
            when {
                histError != null -> Text(histError!!, color = MaterialTheme.colorScheme.error, fontSize = 13.sp)
                h == null -> Dim("Loading…")
                h.isEmpty() -> Dim("Nothing is recorded: journal.info is off, or the changes came before this app last started.")
                else -> for (e in h) {
                    Column(Modifier.padding(vertical = 4.dp)) {
                        Text(stamp(e.at), fontFamily = FontFamily.Monospace, fontSize = 12.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        for (c in e.changes) Text(describeChange(c, ::stamp), fontSize = 13.sp)
                    }
                }
            }
        }

        if (task.orphans.isNotEmpty()) {
            Row(Modifier.padding(top = 18.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                LucideIcon("lock", size = 14.dp)
                Text("Not defined in your taskrc", fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
                Pill("read-only")
            }
            for (k in task.orphans) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(k, fontFamily = FontFamily.Monospace, fontSize = 12.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Text(task.extra[k] ?: "", fontFamily = FontFamily.Monospace, fontSize = 12.sp)
                }
            }
        }
    }
}

@Composable
private fun Box150(content: @Composable () -> Unit) {
    androidx.compose.foundation.layout.Box(Modifier.width(150.dp)) { content() }
}

private fun Modifier.androidxClickable(onClick: () -> Unit): Modifier = this.clickable(onClick = onClick)
