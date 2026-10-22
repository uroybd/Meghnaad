package me.utsob.meghnaad.app

import androidx.compose.foundation.clickable
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Checkbox
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties

/**
 * What the form holds and what it makes of it. One form for both jobs: [row] set = edit that task, null = create a new one.
 * Either way it only sends what changed, as `modify` or `add` arguments (the state of TaskEditor.svelte).
 */
class EditorState(private val store: Store, val row: Row?, seed: FormSeed? = null) {
    val adding = row == null
    private val defs = store.config?.config?.udas ?: emptyMap()
    val defined: List<UdaDef> = defs.values.toList()
    val priorityValues: List<String> = defs["priority"]?.values?.filter { it.isNotEmpty() }?.takeIf { it.isNotEmpty() } ?: listOf("H", "M", "L")

    private fun wire(ts: Long?): String = if (ts == null) "" else fromParts(toParts(ts, store.offsetAt(ts)))

    private fun uda(u: UdaDef): String {
        if (row == null) return u.default ?: ""
        val raw = row.extra[u.name] ?: ""
        return if (u.type == "date" && raw.isNotEmpty()) wire(raw.toDoubleOrNull()?.toLong()) else raw
    }

    // What the fields are measured against. Edit diffs against the task; add diffs against "empty" (plus UDA defaults), so what
    // a seed (a line broken into fields by the quick add) filled in is sent too.
    private val startDescription = row?.description ?: ""
    private val startProject = row?.project ?: ""
    private val startPriority = row?.priority ?: ""
    private val startDue = wire(row?.due)
    private val startWait = wire(row?.wait)
    private val startScheduled = wire(row?.scheduled)
    private val startUntil = wire(row?.until)
    private val startTags = (row?.tags ?: emptyList()).joinToString(" ")
    private val startRecur = row?.recur ?: ""
    private val udaStart = defined.associate { it.name to uda(it) }

    // What the fields hold when the form opens: the task, or the seed.
    private val init = if (adding) seed else null
    private val initRecur = init?.recur ?: startRecur

    var description by mutableStateOf(init?.description ?: startDescription)
    var project by mutableStateOf(init?.project ?: startProject)
    var priority by mutableStateOf(init?.priority ?: startPriority)
    var due by mutableStateOf(init?.due ?: startDue)
    var wait by mutableStateOf(init?.wait ?: startWait)
    var scheduled by mutableStateOf(init?.scheduled ?: startScheduled)
    var until by mutableStateOf(init?.until ?: startUntil)
    var tags by mutableStateOf(init?.tags ?: startTags)

    // Repeat: a preset, or "custom" with a free-text period (`3d`, `P2W`, …). The engine validates it.
    var recurChoice by mutableStateOf(if (initRecur.isEmpty()) "" else presetFor(initRecur).ifEmpty { "custom" })
    var recurCustom by mutableStateOf(if (presetFor(initRecur).isNotEmpty()) "" else initRecur)
    val recur: String get() = if (recurChoice == "custom") recurCustom.trim() else recurChoice

    /** Instances follow their template: its period is shown but changed on the template. */
    val instance = row?.parent != null

    /** A template cannot stop repeating (Taskwarrior refuses); delete it instead. */
    val template = row?.status == "recurring"
    val needsDue: Boolean get() = !instance && recur.isNotEmpty() && due.isEmpty()
    val udaValues = mutableStateMapOf<String, String>().also { it.putAll(udaStart); it.putAll(init?.udas ?: emptyMap()) }
    var startNow by mutableStateOf(false)
    var removedDeps by mutableStateOf(emptyList<String>())
    var newDep by mutableStateOf(init?.depends ?: "")
    private val carried = init?.carried ?: emptyList()
    var note by mutableStateOf("")
    var busy by mutableStateOf(false)

    fun changes(): List<String> {
        val a = mutableListOf<String>()
        fun set(name: String, now: String, was: String) {
            if (now != was) a += "$name:$now"
        }
        if (description.trim() != startDescription) a += "description:${description.trim()}"
        set("project", project.trim(), startProject)
        set("priority", priority, startPriority)
        set("due", due, startDue)
        set("wait", wait, startWait)
        set("scheduled", scheduled, startScheduled)
        set("until", until, startUntil)
        if (!instance) set("recur", recur, startRecur)
        val want = tags.split(Regex("[\\s,]+")).filter { it.isNotEmpty() }.map { it.removePrefix("+") }.toSet()
        val had = startTags.split(' ').filter { it.isNotEmpty() }.toSet()
        for (t in want) if (t !in had) a += "+$t"
        for (t in had) if (t !in want) a += "-$t"
        for (u in defined) set(u.name, udaValues[u.name] ?: "", udaStart[u.name] ?: "")
        for (d in removedDeps) a += "depends:-$d"
        for (d in newDep.split(Regex("[\\s,]+")).filter { it.isNotEmpty() }) a += "depends:$d"
        if (adding) a += carried
        // Starting is a separate step after adding (see save): the `start` command also journals.
        return a
    }

    val dirty: Boolean get() = (if (adding) description.trim().isNotEmpty() else changes().isNotEmpty()) && !needsDue

    val command: String
        get() = if (adding) (listOf("add") + changes()).joinToString(" ") + (if (startNow) "  then: start" else "")
        else "${row!!.uuid.take(8)} modify ${changes().joinToString(" ")}"

    /** Save, then [done] with whether the form should close (false: "add another" keeps it open, fresh). */
    suspend fun save(from: Entry?): Boolean {
        busy = true
        val res = store.act(from, if (adding) listOf("add") + changes() else listOf(row!!.uuid, "modify") + changes())
        val made = (res?.result as? CliResult.Changed)?.value?.tasks?.firstOrNull()
        if (adding && made != null) {
            if (startNow) store.act(from, listOf(made.uuid, "start"))
            if (note.trim().isNotEmpty()) store.act(from, listOf(made.uuid, "annotate", note.trim()))
        }
        busy = false
        return res != null && res.result !is CliResult.Error
    }
}

/** The form, as a screen of its own over the page (TaskEditor.svelte). */
@Composable
fun TaskEditor(store: Store, row: Row?, from: Entry?, seed: FormSeed? = null, onClose: () -> Unit, onAdded: (again: Boolean) -> Unit = { onClose() }) {
    val st = remember(row?.uuid, seed) { EditorState(store, row, seed) }
    val colors = MaterialTheme.colorScheme
    Dialog(onDismissRequest = onClose, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Sheet(drawer = false, width = 760.dp, color = colors.surfaceContainerLow) {
            Column(Modifier.fillMaxSize().safeDrawingPadding()) {
                // What the form's own Enter does on the web: save, or add, when there is something to save.
                fun submit() {
                    if (st.busy || !st.dirty) return
                    store.go { if (st.save(from)) { if (st.adding) onAdded(false) else onClose() } }
                }
                // Ctrl+Enter (or Cmd+Enter) saves from a field where Enter adds a line (multiline.ts: newlineOnEnter).
                val saveOnCtrlEnter = Modifier.onPreviewKeyEvent { e ->
                    if (e.type == KeyEventType.KeyDown && e.key == Key.Enter && (e.isCtrlPressed || e.isMetaPressed)) { submit(); true } else false
                }
                Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp, top = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(if (st.adding) "New task" else "Edit task", Modifier.weight(1f), fontSize = 18.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold)
                    row?.let { Dim(it.uuid.take(8), size = 12) }
                    IconAction("x", "Cancel", onClose, size = 20.dp)
                }
                Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
                    FieldLabel("Description")
                    // A description is one line, as the web's input is: it may wrap on a phone, but Enter saves it instead of breaking it.
                    OutlinedTextField(
                        st.description,
                        { v -> if ('\n' in v) { st.description = v.replace("\n", " ").trimEnd(); submit() } else st.description = v },
                        Modifier.fillMaxWidth().onPreviewKeyEvent { e ->
                            if (e.type == KeyEventType.KeyDown && e.key == Key.Enter) { submit(); true } else false
                        },
                        minLines = 1, maxLines = 4,
                        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
                    )
                    // Fields that share a row, as many as fit the width (the web's `.pair` rows).
                    FieldGrid(150.dp, listOf(
                        FieldCell("Project", 2f) { SuggestField(st.project, { st.project = it }, store.projects, "project", placeholder = "none") },
                        FieldCell("Priority", 1f) { Choice(st.priority.ifEmpty { "none" }, listOf("none") + st.priorityValues) { i -> st.priority = if (i == 0) "" else st.priorityValues[i - 1] } },
                    ))
                    FieldGrid(150.dp, listOf(
                        FieldCell("Due") { DateTimeInput(st.due, { st.due = it }, "Due", store.now, store.utcOffset, Modifier.fillMaxWidth()) },
                        FieldCell("Wait") { DateTimeInput(st.wait, { st.wait = it }, "Wait", store.now, store.utcOffset, Modifier.fillMaxWidth()) },
                        FieldCell("Scheduled") { DateTimeInput(st.scheduled, { st.scheduled = it }, "Scheduled", store.now, store.utcOffset, Modifier.fillMaxWidth()) },
                        FieldCell("Until") { DateTimeInput(st.until, { st.until = it }, "Until", store.now, store.utcOffset, Modifier.fillMaxWidth()) },
                    ))
                    FieldGrid(240.dp, listOf(
                        FieldCell("Repeat", 1f) {
                            Column {
                    if (st.instance) {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Pill(describeRecur(row?.recur))
                            Dim("This task is one instance of a series.", size = 12)
                        }
                        row?.parent?.let { parent ->
                            TextButton(onClick = {
                                onClose()
                                store.openDetail(parent, from)
                            }) {
                                LucideIcon("corner-up-left", size = 13.dp)
                                Text("  open the recurring task", fontSize = 13.sp)
                            }
                        }
                    } else {
                        val labels = (if (!st.template) listOf("Doesn't repeat") else emptyList()) + PRESETS.map { it.label } + "Custom…"
                        val values = (if (!st.template) listOf("") else emptyList()) + PRESETS.map { it.value } + "custom"
                        Choice(labels.getOrElse(values.indexOf(st.recurChoice).coerceAtLeast(0)) { "Doesn't repeat" }, labels) { i -> st.recurChoice = values[i] }
                        if (st.recurChoice == "custom") {
                            OutlinedTextField(
                                st.recurCustom, { st.recurCustom = it }, Modifier.fillMaxWidth().padding(top = 6.dp), singleLine = true,
                                placeholder = { Text("3d, 2w, P1M, 6mo…", fontSize = 14.sp) },
                                keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
                            )
                        }
                        when {
                            st.needsDue -> Text("A repeating task needs a due date.", color = colors.error, fontSize = 12.sp, modifier = Modifier.padding(top = 4.dp))
                            st.recur.isNotEmpty() && !st.template -> Dim("Repeats ${describeRecur(st.recur)}, starting from the due date.", Modifier.padding(top = 4.dp), 12)
                            st.template -> Dim("Changes to a repeating task also apply to its open instances (not their dates).", Modifier.padding(top = 4.dp), 12)
                        }
                    }
                            }
                        },
                        FieldCell("Tags", 2f) { SuggestField(st.tags, { st.tags = it }, store.tags, "tag", list = true, placeholder = "space separated") },
                    ))
                    for (u in st.defined) {
                        FieldLabel(udaLabel(u))
                        val current = st.udaValues[u.name] ?: ""
                        when {
                            u.type == "date" -> DateTimeInput(current, { st.udaValues[u.name] = it }, udaLabel(u), store.now, store.utcOffset)
                            u.type == "string" && u.values.isNotEmpty() -> {
                                val options = listOf("none") + u.values.filter { it.isNotEmpty() }
                                Choice(current.ifEmpty { "none" }, options) { i -> st.udaValues[u.name] = if (i == 0) "" else options[i] }
                            }
                            u.type == "numeric" -> OutlinedTextField(
                                current, { st.udaValues[u.name] = it }, Modifier.fillMaxWidth(), singleLine = true,
                                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                            )
                            u.type == "string" -> OutlinedTextField(current, { st.udaValues[u.name] = it }, Modifier.fillMaxWidth().then(saveOnCtrlEnter), minLines = 1, maxLines = 5)
                            else -> OutlinedTextField(current, { st.udaValues[u.name] = it }, Modifier.fillMaxWidth(), singleLine = true)
                        }
                    }
                    FieldLabel("Depends on")
                    row?.depends?.forEach { d ->
                        val gone = d in st.removedDeps
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(d.take(8), fontFamily = FontFamily.Monospace, fontSize = 13.sp, color = if (gone) colors.onSurfaceVariant else colors.onSurface, textDecoration = if (gone) androidx.compose.ui.text.style.TextDecoration.LineThrough else null)
                            IconAction(if (gone) "rotate-ccw" else "x", if (gone) "Keep dependency" else "Remove dependency", { st.removedDeps = if (gone) st.removedDeps - d else st.removedDeps + d }, size = 14.dp)
                        }
                    }
                    DependsField(store, st.newDep) { st.newDep = it }
                    if (st.adding) {
                        FieldLabel("Start")
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Checkbox(st.startNow, { st.startNow = it })
                            Text("start working on it now", fontSize = 14.sp)
                        }
                    }
                    if (row != null && row.orphans.isNotEmpty()) {
                        Row(Modifier.padding(top = 14.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            LucideIcon("lock", size = 13.dp)
                            Dim("Not in taskrc", size = 13)
                        }
                        for (k in row.orphans) Text("$k = ${row.extra[k]}", fontFamily = FontFamily.Monospace, fontSize = 12.sp)
                        Dim("read-only: define uda.${row.orphans[0]}.type in your taskrc to edit", size = 12)
                    }
                    fun addNote() {
                        val text = st.note.trim()
                        store.go {
                            val res = act(from, listOf(row!!.uuid, "annotate", text))
                            if (res != null && res.result !is CliResult.Error) {
                                st.note = ""
                                onClose()
                            }
                        }
                    }
                    FieldLabel(if (st.adding) "Note" else "Annotations")
                    for (a in visibleNotes(row?.annotations ?: emptyList(), store.config?.journal)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Dim(formatMoment(a.entry, store.offsetAt(a.entry), formatFor(FormatKind.InfoNote, store.config?.config?.settings)), size = 12)
                            Text("  " + a.text, Modifier.weight(1f), fontSize = 14.sp)
                            IconAction("x", "Remove annotation", {
                                store.go {
                                    val res = act(from, listOf(row!!.uuid, "denotate", a.text))
                                    if (res != null && res.result !is CliResult.Error) onClose()
                                }
                            }, size = 14.dp)
                        }
                    }
                    Row(Modifier.fillMaxWidth().padding(bottom = 16.dp), verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        OutlinedTextField(
                            st.note, { st.note = it },
                            Modifier.weight(1f).onPreviewKeyEvent { e ->
                                // Enter adds the note to a task being edited, or adds the task being made (Shift+Enter is a line break).
                                if (e.type == KeyEventType.KeyDown && e.key == Key.Enter && !e.isShiftPressed) {
                                    if (st.adding) submit() else if (st.note.isNotBlank()) addNote()
                                    true
                                } else false
                            },
                            minLines = 1, maxLines = 4,
                            placeholder = { Text(if (st.adding) "Optional first note" else "Add an annotation", fontSize = 14.sp) },
                        )
                        if (!st.adding) {
                            Button(enabled = st.note.isNotBlank(), onClick = ::addNote) { Text("Add") }
                        }
                    }
                }
                HorizontalDivider(color = colors.outlineVariant)
                Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
                    if (st.dirty) Text("task ${st.command}", fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = colors.onSurfaceVariant, maxLines = 2, modifier = Modifier.padding(bottom = 6.dp))
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End), verticalAlignment = Alignment.CenterVertically) {
                        TextButton(onClick = onClose) { Text("Cancel") }
                        if (st.adding) {
                            OutlinedButton(enabled = !st.busy && st.dirty, onClick = { store.go { if (st.save(from)) onAdded(true) } }) { Text("Add & another") }
                        }
                        Button(enabled = !st.busy && st.dirty, onClick = { store.go { if (st.save(from)) { if (st.adding) onAdded(false) else onClose() } } }) {
                            Text(if (st.adding) "Add task" else "Save")
                        }
                    }
                }
            }
        }
    }
}

/** The task a dependency can name: a field for ids or uuid prefixes, offering the tasks that have a number (the web's datalist). */
@Composable
private fun DependsField(store: Store, value: String, onChange: (String) -> Unit) {
    var focused by remember { mutableStateOf(false) }
    val word = wordAt(value, value.length, list = true)
    val typed = word.text.trim()
    val options = if (!focused || typed.isEmpty()) emptyList() else store.tasks.filter { t ->
        t.id != null && (t.id.toString().startsWith(typed) || t.description.contains(typed, ignoreCase = true))
    }.take(8)
    Column(Modifier.fillMaxWidth()) {
        OutlinedTextField(
            value, onChange, Modifier.fillMaxWidth().onFocusChanged { focused = it.isFocused }, singleLine = true,
            placeholder = { Text("task ids or uuid prefixes", fontSize = 14.sp) },
        )
        if (options.isNotEmpty()) {
            Surface(Modifier.fillMaxWidth().padding(top = 2.dp), shape = MaterialTheme.shapes.medium, tonalElevation = 3.dp, shadowElevation = 4.dp, color = MaterialTheme.colorScheme.surfaceContainerHigh) {
                Column {
                    options.forEachIndexed { i, t ->
                        Row(Modifier.fillMaxWidth().clickable { onChange(choose(value, word, t.id.toString(), list = true).value) }.padding(horizontal = 12.dp, vertical = 10.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                            Text(t.id.toString(), fontFamily = FontFamily.Monospace, fontSize = 13.sp, fontWeight = FontWeight.Bold)
                            Text(t.description, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                        if (i < options.lastIndex) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))
                    }
                }
            }
        }
    }
}
