package me.utsob.meghnaad.app

import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** The Kanban tab's edits: kept while another tab is shown (KanbanPanel.svelte). */
class KanbanState(private val store: Store) {
    val saved get() = store.kanbanSettings
    var draft by mutableStateOf(saved)
    var busy by mutableStateOf(false)
    var error by mutableStateOf<String?>(null)
    var done by mutableStateOf(false)

    /** What the fields were last made from: when the saved settings change and they were not touched, they are made again. */
    private var base = saved

    /** The saved settings changed (applied here, settings.json read late, another device): follow them, unless there are edits. */
    fun follow() {
        val now = saved
        if (now == base) return
        val untouched = draft == base
        base = now
        if (untouched) draft = now
    }

    val edited get() = draft != saved
    val udaProblem get() = udaNameProblem(draft.uda)
    val status get() = udaStatus(store.config, draft.uda)
    val lines get() = if (udaProblem != null) emptyList() else kanbanSetupLines(store.config, draft.uda)
    val review get() = hasReview(store.config)

    /** The move commands Set up adds to Commands (the ones not there yet). */
    val newMoves get() = withMoveCommands(store.taskCommands, review, draft.uda).drop(store.taskCommands.size)
    /** The automatic setup adds a `kanban` report to the taskrc, so the kanbans need no filter of their own. */
    var withReport by mutableStateOf(true)
    val hasReport get() = store.config?.config?.reports?.containsKey(KANBAN_REPORT) == true
    val addLines get() = if (withReport) kanbanReportLines(store.config) else emptyList()

    fun kanbanProblemAt(i: Int): String? =
        kanbanProblem(resolveKanban(draft, draft.kanbans[i]), draft.kanbans.filterIndexed { j, _ -> j != i }.map { it.id })
    val valid get() = udaProblem == null && draft.columns.all { columnProblem(it) == null } &&
        draft.kanbans.indices.all { i -> kanbanProblemAt(i) == null && draft.kanbans[i].columns.orEmpty().all { columnProblem(it) == null } }

    fun set(f: (KanbanSettings) -> KanbanSettings) {
        draft = f(draft)
        done = false
    }

    fun change(i: Int, f: (KanbanEntry) -> KanbanEntry) = set { s -> s.copy(kanbans = s.kanbans.toMutableList().also { it[i] = f(it[i]) }) }

    fun addKanban() = set { s -> s.copy(kanbans = s.kanbans + newKanban("", s.kanbans.map { it.id })) }

    private suspend fun save(next: KanbanSettings, commands: List<TaskCommand>? = null) {
        busy = true
        error = null
        done = false
        try {
            store.saveKanban(next, commands)
            draft = next
            base = next
            done = true
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        } finally {
            busy = false
        }
    }

    /** Write the UDA's `config` lines (and the report's, when the setup adds it), one at a time (each reads the saved settings and writes them back). */
    private suspend fun writeLines(more: List<Pair<String, String>> = emptyList()) {
        try {
            for (line in lines + more) store.query(configArgs(line))
        } finally {
            store.settingsChanged()
        }
    }

    suspend fun setUp(automatic: Boolean) {
        if (automatic) {
            if (udaProblem != null) return
            busy = true
            error = null
            try {
                writeLines(addLines)
            } catch (e: Exception) {
                error = e.message ?: e.toString()
                busy = false
                return
            }
            busy = false
        }
        if (!automatic) {
            save(draft.copy(enabled = true, setup = "manual", report = store.reports.firstOrNull()?.name ?: "next"))
            return
        }
        // Each column moves with a task command (pending, inprogress, review), added unless one of that name stands for something else.
        val moves = usableMoves(store.taskCommands, review, draft.uda)
        save(automaticSettings(draft, review, draft.uda, withReport, moves), withMoveCommands(store.taskCommands, review, draft.uda))
    }

    suspend fun applyLines() {
        busy = true
        error = null
        try {
            writeLines()
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        } finally {
            busy = false
        }
    }

    suspend fun apply() = save(draft)

    suspend fun turnOff() = save(saved.copy(enabled = false))

    fun revert() {
        draft = saved
        done = false
    }
}

private val mono = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 13.sp)
private val plain = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None)

private fun partText(p: Part) = when (p) {
    Part.Ok -> "defined"
    Part.Missing -> "not defined"
    Part.Different -> "defined differently"
}

@Composable
private fun UdaField(st: KanbanState) {
    OutlinedTextField(
        st.draft.uda, { v -> st.set { it.copy(uda = v) } }, Modifier.fillMaxWidth(), singleLine = true, textStyle = mono, keyboardOptions = plain,
        label = { Text("UDA") }, placeholder = { Text(PROGRESS_UDA, fontSize = 13.sp) },
    )
    st.udaProblem?.let { Text(it, color = MaterialTheme.colorScheme.error, fontSize = 12.sp) }
}

@Composable
fun KanbanPanel(store: Store, st: KanbanState) {
    val colors = MaterialTheme.colorScheme
    LaunchedEffect(st.saved) { st.follow() }
    val saved = st.saved
    val d = st.draft
    val status = st.status
    val lines = st.lines

    Column(Modifier.fillMaxWidth().padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Text("Kanban", fontSize = 18.sp, fontWeight = FontWeight.SemiBold)
        Dim(
            "A kanban is columns of tasks. Each column has a filter that picks its tasks (a task is in the first column that matches it) and an On move " +
                "command line, run on a task moved into it: modify ${d.uda}:review, done, or one of your commands. A kanban is drawn from a report, " +
                "whose filter, order and columns it takes: what a card shows is that report's columns. The settings below are every kanban's; a kanban " +
                "can have its own report, filter, Completed since or columns instead.",
        )
        if (!saved.enabled) {
            Column(Modifier.fillMaxWidth().border(1.dp, colors.outlineVariant, ControlShape).padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Automatic", fontWeight = FontWeight.SemiBold)
                Dim(
                    "Defines a UDA and gives every kanban the columns To do, In progress, Review and Done. Moving a card out of Done " +
                        "reopens it; nothing starts a task." +
                        if (st.review) " tasksh's review is set up, so moving a card to Review puts it back in the review queue (it clears reviewed)." else "",
                )
                UdaField(st)
                if (st.udaProblem == null) {
                    Text("The ${d.uda} UDA is ${partText(status.part)}.", fontSize = 13.sp)
                    Row(Modifier.clickable { st.withReport = !st.withReport }, verticalAlignment = Alignment.CenterVertically) {
                        Checkbox(st.withReport, { st.withReport = it })
                        Text("Draw the kanbans on a $KANBAN_REPORT report" + if (st.hasReport) " (your taskrc has one: it is used as it is)" else ", added to your taskrc", fontSize = 13.sp)
                    }
                    Dim(
                        if (st.withReport) "Its filter leaves out deleted and waiting tasks and recurring templates, so a kanban needs no filter of its own."
                        else "The kanbans are drawn on all, with the filter ( +PENDING or +COMPLETED ).",
                        size = 12,
                    )
                    val all = lines + st.addLines
                    if (all.isNotEmpty()) {
                        Dim("Set up writes these lines to your taskrc, and nothing else:", size = 12)
                        all.forEach { (n, v) -> Text("config $n $v", fontFamily = FontFamily.Monospace, fontSize = 11.sp) }
                    }
                    val moves = st.newMoves
                    if (moves.isNotEmpty()) {
                        Dim("The columns move a card with task commands, which you can also run from a task's menu or the console (3 review). Set up adds them to Commands:", size = 12)
                        moves.forEach { Text("${it.name}  ${it.command}", fontFamily = FontFamily.Monospace, fontSize = 11.sp) }
                    }
                }
                Button(onClick = { store.go { st.setUp(true) } }, enabled = !st.busy && st.udaProblem == null) { Text("Set up automatically") }
            }
            Column(Modifier.fillMaxWidth().border(1.dp, colors.outlineVariant, ControlShape).padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Manual", fontWeight = FontWeight.SemiBold)
                Dim("Nothing in your taskrc changes: you write the columns yourself, with tags, a UDA of your own, +ACTIVE, anything a filter can say.")
                Button(onClick = { store.go { st.setUp(false) } }, enabled = !st.busy) { Text("Turn on") }
            }
        } else {
            if (status.part != Part.Ok && st.udaProblem == null && saved.setup == "automatic" && d.uda == saved.uda) {
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text("The ${d.uda} UDA is ${partText(status.part)}. These lines would define it:", color = colors.error, fontSize = 13.sp)
                    lines.forEach { (n, v) -> Text("config $n $v", fontFamily = FontFamily.Monospace, fontSize = 11.sp) }
                    TextButton(onClick = { store.go { st.applyLines() } }, enabled = !st.busy) { Text("Write them") }
                }
            }
            val unread = store.appSettings?.kanbanErrors ?: emptyList()
            if (unread.isNotEmpty()) {
                Column {
                    Text("${if (unread.size == 1) "One entry" else "${unread.size} entries"} in settings.json can't be used, and kept in it as they are:", color = colors.error, fontSize = 13.sp)
                    unread.forEach { Text("• $it", color = colors.error, fontSize = 12.sp) }
                }
            }
            Column(Modifier.fillMaxWidth().border(1.dp, colors.outlineVariant, ControlShape).padding(12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Text("Every kanban", fontWeight = FontWeight.SemiBold)
                UdaField(st)
                if (d.uda != saved.uda) Dim("The columns name the UDA in their own filters and moves: change them to match.", size = 12)
                ReportField(store, d.report) { v -> st.set { it.copy(report = v) } }
                TextField("Filter", d.filter) { v -> st.set { it.copy(filter = v) } }
                TextField("Completed since", d.completedSince, "today-14d (empty: all)") { v -> st.set { it.copy(completedSince = v) } }
                Text("Columns", fontWeight = FontWeight.Medium, fontSize = 13.sp)
                ColumnsEditor(store, d.columns) { cols -> st.set { it.copy(columns = cols) } }
            }
            d.kanbans.forEachIndexed { i, k -> KanbanEditor(store, st, i, k) }
            TextButton(onClick = st::addKanban) { Text("Add a kanban") }
            if (st.done && !st.edited) Text("Applied.", color = extras.ok, fontSize = 13.sp)
            Row(Modifier.fillMaxWidth().padding(bottom = 24.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                TextButton(onClick = { store.go { st.turnOff() } }, enabled = !st.busy) { Text("Turn Kanban off") }
                Spacer(Modifier.weight(1f))
                if (st.edited) TextButton(onClick = st::revert) { Text("Revert edits") }
                Button(onClick = { store.go { st.apply() } }, enabled = !st.busy && st.edited && st.valid) { Text("Apply") }
            }
        }
        st.error?.let { Text(it, color = colors.error, fontSize = 13.sp) }
    }
}

@Composable
private fun ReportField(store: Store, report: String, onChange: (String) -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Text("Report", fontSize = 13.sp)
        val names = store.reports.map { it.name }.let { if (report in it) it else listOf(report) + it }
        Choice(report, names) { n -> onChange(names[n]) }
    }
}

@Composable
private fun TextField(label: String, value: String, placeholder: String? = null, onChange: (String) -> Unit) {
    OutlinedTextField(
        value, onChange, Modifier.fillMaxWidth(), singleLine = true, textStyle = mono, keyboardOptions = plain,
        label = { Text(label) }, placeholder = placeholder?.let { { Text(it, fontSize = 13.sp) } },
    )
}

@Composable
private fun ColumnsEditor(store: Store, columns: List<KanbanColumn>, onChange: (List<KanbanColumn>) -> Unit) {
    val colors = MaterialTheme.colorScheme
    // What each move runs on an iterative task, where that is another line (`status:` left out, a `done` clearing).
    val udas = store.config?.config?.udas?.keys?.toList() ?: emptyList()
    val clears = boardClears(columns, store.taskCommands, udas)
    fun change(j: Int, f: (KanbanColumn) -> KanbanColumn) = onChange(columns.toMutableList().also { it[j] = f(it[j]) })
    fun move(j: Int, by: Int) {
        val to = j + by
        if (to in columns.indices) onChange(columns.toMutableList().also { val c = it[j]; it[j] = it[to]; it[to] = c })
    }
    if (columns.isEmpty()) Dim("No columns yet.")
    columns.forEachIndexed { j, c ->
        Column(Modifier.fillMaxWidth().padding(start = 8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("${j + 1}. ${c.name.ifEmpty { "New column" }}", Modifier.weight(1f), fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
                IconAction("arrow-up", "Earlier", onClick = { move(j, -1) }, enabled = j > 0)
                IconAction("arrow-down", "Later", onClick = { move(j, 1) }, enabled = j < columns.size - 1)
                IconAction("x", "Remove", onClick = { onChange(columns.filterIndexed { n, _ -> n != j }) })
            }
            TextField("Name", c.name) { v -> change(j) { it.copy(name = v) } }
            TextField("Filter", c.filter) { v -> change(j) { it.copy(filter = v) } }
            TextField("On move", c.move, "empty: takes no cards") { v -> change(j) { it.copy(move = v) } }
            if (c.move.isNotBlank()) {
                iterativeMove(c.move, clears, store.taskCommands, udas)?.let { line ->
                    Text("On an iterative task: ${line.ifEmpty { "nothing" }}", color = colors.onSurfaceVariant, fontSize = 12.sp, fontFamily = FontFamily.Monospace)
                }
            }
            columnProblem(c)?.let { Text(it, color = colors.error, fontSize = 12.sp) }
        }
    }
    TextButton(onClick = { onChange(columns + KanbanColumn("", "", "")) }) { Text("Add a column") }
}

@Composable
private fun Own(label: String, on: Boolean, onToggle: () -> Unit) {
    Row(Modifier.fillMaxWidth().heightIn(min = 40.dp).clickable(onClick = onToggle), verticalAlignment = Alignment.CenterVertically) {
        Checkbox(on, onCheckedChange = null, modifier = Modifier.padding(horizontal = 8.dp))
        Text(label, fontSize = 14.sp)
    }
}

@Composable
private fun KanbanEditor(store: Store, st: KanbanState, i: Int, k: KanbanEntry) {
    val colors = MaterialTheme.colorScheme
    val d = st.draft
    Column(Modifier.fillMaxWidth().border(1.dp, colors.outlineVariant, ControlShape).padding(12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(k.name.ifEmpty { "New kanban" }, fontWeight = FontWeight.SemiBold)
        val saved = store.kanbanSettings.kanbans.any { it.id == k.id }
        val others = d.kanbans.filterIndexed { j, _ -> j != i }.map { it.id }
        OutlinedTextField(k.name, { v -> st.change(i) { renamedKanban(it, v, saved, others) } }, Modifier.fillMaxWidth(), singleLine = true, label = { Text("Name") })
        OutlinedTextField(
            k.id, { v -> st.change(i) { it.copy(id = v) } }, Modifier.fillMaxWidth(), singleLine = true, textStyle = mono, keyboardOptions = plain,
            label = { Text("Address") }, prefix = { Text("/kanban/", fontFamily = FontFamily.Monospace, fontSize = 13.sp) },
        )
        Own("Its own report", k.report != null) { st.change(i) { it.copy(report = if (it.report == null) d.report else null) } }
        k.report?.let { r -> ReportField(store, r) { v -> st.change(i) { it.copy(report = v) } } }
        Own("Its own filter", k.filter != null) { st.change(i) { it.copy(filter = if (it.filter == null) d.filter else null) } }
        k.filter?.let { f -> TextField("Filter", f) { v -> st.change(i) { it.copy(filter = v) } } }
        Own("Its own Completed since", k.completedSince != null) {
            st.change(i) { it.copy(completedSince = if (it.completedSince == null) d.completedSince else null) }
        }
        k.completedSince?.let { c -> TextField("Completed since", c, "today-14d (empty: all)") { v -> st.change(i) { it.copy(completedSince = v) } } }
        Own("Its own columns", k.columns != null) { st.change(i) { it.copy(columns = if (it.columns == null) d.columns else null) } }
        st.kanbanProblemAt(i)?.let { Text(it, color = colors.error, fontSize = 12.sp) }
        k.columns?.let { cols ->
            Text("Its columns", fontWeight = FontWeight.Medium, fontSize = 13.sp)
            ColumnsEditor(store, cols) { next -> st.change(i) { it.copy(columns = next) } }
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Spacer(Modifier.weight(1f))
            TextButton(onClick = { st.set { s -> s.copy(kanbans = s.kanbans.filterIndexed { n, _ -> n != i }) } }) { Text("Delete this kanban", color = colors.error) }
        }
    }
}
