package me.utsob.meghnaad.app

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.serializer

/**
 * The Tasks page: the report in focus and its filter on top, then the tasks. A header tap rewrites `rc.report.<name>.sort:…` in
 * the filter, exactly what you would type in the console, so the list, the chips and the "last command" line stay in step
 * (TasksView.svelte).
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
fun TasksView(store: Store) {
    var ranFilter by remember { mutableStateOf<String?>(null) }
    val args = reportArgs(store.filter, store.report)

    // Run the report whenever the report or the filter changes. Only typing is debounced: opening the page or picking another
    // report should not wait for a keystroke that is not coming. The console may already have loaded exactly this (it hands its
    // result over), so skip duplicates.
    LaunchedEffect(store.report, store.filter) {
        val key = engineJson.encodeToString(ListSerializer(String.serializer()), args)
        if (key == store.liveKey) {
            ranFilter = store.filter
            return@LaunchedEffect
        }
        val typing = ranFilter != null && store.filter != ranFilter
        if (typing) delay(250)
        ranFilter = store.filter
        store.runLive(CliInput(args = args))
    }

    // A task that is not in the list any more (another filter, or it was finished) cannot stay ticked.
    val result = (store.live?.result as? CliResult.Report)?.value
    LaunchedEffect(result) { result?.let { r -> store.selection.keep(r.rows.map { it.uuid }.toSet()) } }
    // Leaving the Tasks page ends the selection.
    DisposableEffect(Unit) { onDispose { store.selection.clear() } }

    val entry = store.live
    val ctx = result?.let { store.cellCtx(it.report) }
    val heads = if (result != null && ctx != null) groupHeads(result, ctx) else emptyList()
    val dimmed = entry?.loading == true && result != null

    LazyColumn(Modifier.fillMaxSize().padding(horizontal = 12.dp), contentPadding = androidx.compose.foundation.layout.PaddingValues(bottom = 96.dp)) {
        item { FilterBar(store) }
        item { BulkBar(store) }
        when {
            entry == null -> Unit
            entry.failure != null -> item { Text(entry.failure!!, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(vertical = 8.dp)) }
            result == null -> item { ResultBlock(store, entry) }
            result.rows.isEmpty() -> item { Dim("No tasks match.", Modifier.padding(vertical = 16.dp), size = 14) }
            else -> {
                item { SortBar(store, result) }
                items(result.rows.size, key = { result.rows[it].uuid }) { i ->
                    val row = result.rows[i]
                    heads.getOrNull(i)?.let { head ->
                        Text(head, Modifier.padding(top = 14.dp, bottom = 6.dp, start = 2.dp), fontWeight = FontWeight.SemiBold, fontSize = 13.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    Box(Modifier.let { if (dimmed) it else it }) {
                        TaskCard(
                            store, row, result, ctx!!, from = entry, selectable = true,
                            activeTags = activeTags(store.filter),
                            onTag = { store.filter = toggleTag(store.filter, it) },
                            onEdit = { store.editing = EditRef(it, entry) },
                            gapBefore = heads.isEmpty() && result.breaks.getOrNull(i) == true,
                        )
                    }
                }
                item {
                    Dim(
                        if (result.rows.size == result.matched) "${result.matched} ${if (result.matched == 1) "task" else "tasks"}"
                        else "showing ${result.rows.size} of ${result.matched}",
                        Modifier.padding(vertical = 8.dp),
                    )
                }
            }
        }
    }
}

/** Phones have no column headers to tap, so a sort picker stands in for them (the same command underneath). */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SortBar(store: Store, result: ReportResult) {
    val keys = parseSort(result.sort)
    val primary = keys.firstOrNull()
    val udas = store.config?.config?.udas ?: emptyMap()
    val columns = result.columns.filter { baseColumn(it.name) in SORTABLE || baseColumn(it.name) in udas }
    if (columns.isEmpty()) return
    val default = store.reports.firstOrNull { it.name == store.report }?.sort

    fun sortBy(column: String) {
        val res = clickSort(parseSort(result.sort), baseColumn(column), false)
        store.filter = withSortOverride(store.filter, store.report, if (res.reset) null else serializeSort(res.keys), default)
    }

    fun groupBy(column: String?) {
        val ks = if (column != null) toggleGroup(parseSort(result.sort), column, parseSort(default)) else clearGroups(parseSort(result.sort))
        store.filter = withSortOverride(store.filter, store.report, serializeSort(ks), default)
    }

    FlowRow(Modifier.fillMaxWidth().padding(vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            Dim("Sort")
            Choice(primary?.column?.let { c -> columns.firstOrNull { baseColumn(it.name) == c }?.label ?: c } ?: "report default", columns.map { it.label }) { i -> sortBy(columns[i].name) }
            IconAction(
                if (primary?.desc == true) "arrow-down" else "arrow-up",
                if (primary != null) "Sorted ${if (primary.desc) "descending" else "ascending"}; change direction" else "Sort direction",
                { primary?.let { sortBy(it.column) } },
                enabled = primary != null,
                size = 16.dp,
            )
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            Dim("Group")
            val grouped = groupColumn(keys)
            Choice(grouped?.let { g -> columns.firstOrNull { baseColumn(it.name) == g }?.label ?: g } ?: "none", listOf("none") + columns.map { it.label }) { i ->
                groupBy(if (i == 0) null else baseColumn(columns[i - 1].name))
            }
        }
    }
}

/** A small button that opens a menu of choices, answering with the index chosen. */
@Composable
fun Choice(current: String, options: List<String>, modifier: Modifier = Modifier, hints: List<String?> = emptyList(), onPick: (Int) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box(modifier) {
        OutlinedButton(onClick = { open = true }, contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp, vertical = 4.dp)) {
            Text(current, fontSize = 13.sp, maxLines = 1)
            LucideIcon("chevron-down", Modifier.padding(start = 4.dp), size = 14.dp)
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            options.forEachIndexed { i, o ->
                DropdownMenuItem(
                    text = {
                        Column {
                            Text(o, fontSize = 14.sp)
                            hints.getOrNull(i)?.takeIf { it.isNotEmpty() }?.let { Dim(it, size = 11) }
                        }
                    },
                    onClick = {
                        open = false
                        onPick(i)
                    },
                )
            }
        }
    }
}

/** The report and filters (FilterBar.svelte): a report to pick, a filter in Taskwarrior's syntax, and its words as chips. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun FilterBar(store: Store) {
    val meta = store.reports.firstOrNull { it.name == store.report }
    val words = splitWords(store.filter)
    val cfg = store.config?.config
    val context = cfg?.activeContext?.let { it to cfg.contexts[it]?.read }
    var completeSignal by remember { mutableStateOf(0) }

    fun pickReport(name: String) {
        // Sort overrides belong to one report; drop the ones that no longer apply.
        store.filter = joinWords(words.filter { !Regex("^rc\\.report\\.").containsMatchIn(it) || it.startsWith("rc.report.$name.") })
        store.report = name
    }

    Column(Modifier.fillMaxWidth().padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            val offered = store.offeredReports
            Choice(
                store.report,
                offered.map { it.name },
                modifier = Modifier.padding(top = 8.dp),
                hints = offered.map { it.description },
            ) { pickReport(offered[it].name) }
            CompletingField(
                value = store.filter,
                onValueChange = { store.filter = it },
                vocab = store.vocab,
                modifier = Modifier.weight(1f),
                placeholder = "Filters: +work due:eow",
                completeSignal = completeSignal,
            )
            IconAction("arrow-right-to-line", "Complete", { completeSignal++ }, modifier = Modifier.padding(top = 8.dp), size = 18.dp)
        }
        FilterChips(store.filter) { store.filter = it }
        FilterHelpers(store, words)
        if (meta?.filter != null || meta?.sort != null || context != null) {
            val text = buildString {
                if (meta?.filter != null) append("report filter ${meta.filter}")
                if (meta?.sort != null) append("${if (isNotEmpty()) " · " else ""}default sort ${meta.sort}")
                if (context != null) append("${if (isNotEmpty()) " · " else ""}context ${context.first}${context.second?.let { " ($it)" } ?: ""}")
            }
            Dim("$text — applied automatically; your filters are added to these.", size = 12)
        }
    }
}

private val SORT_TOKEN = Regex("^rc\\.report\\.[^.]+\\.sort[:=]")

/** The words of a filter as removable chips, and "clear all" (FilterChips.svelte). */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun FilterChips(filter: String, onChange: (String) -> Unit) {
    val words = splitWords(filter)
    if (words.isEmpty()) return
    FlowRow(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(4.dp), itemVerticalAlignment = Alignment.CenterVertically) {
        words.forEachIndexed { i, w ->
            val label = if (SORT_TOKEN.containsMatchIn(w)) "sort ${w.substring(w.indexOfFirst { it == ':' || it == '=' } + 1)}" else w
            Row(
                Modifier.clip(androidx.compose.foundation.shape.CircleShape).background(MaterialTheme.colorScheme.surfaceVariant),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(label, fontSize = 12.sp, fontFamily = FontFamily.Monospace, modifier = Modifier.padding(start = 9.dp))
                IconAction("x", "Remove $label", { onChange(joinWords(words.filterIndexed { j, _ -> j != i })) }, size = 11.dp, modifier = Modifier.padding(0.dp))
            }
        }
        TextButton(onClick = { onChange("") }) { Text("clear all", fontSize = 12.sp) }
    }
}

/** The filter box shared by the pages: Taskwarrior's filter syntax, with completion of projects, tags and attributes. */
@Composable
fun FilterField(store: Store, value: String, onChange: (String) -> Unit, modifier: Modifier = Modifier, placeholder: String = "Filters: +work due:eow") {
    var completeSignal by remember { mutableStateOf(0) }
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        CompletingField(value, onChange, store.vocab, Modifier.weight(1f), placeholder = placeholder, completeSignal = completeSignal)
        IconAction("arrow-right-to-line", "Complete", { completeSignal++ }, modifier = Modifier.padding(top = 8.dp), size = 18.dp)
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun FilterHelpers(store: Store, words: List<String>) {
    var open by remember { mutableStateOf(false) }
    TextButton(onClick = { open = !open }, contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 4.dp)) {
        LucideIcon("sliders-horizontal", size = 13.dp)
        Text("  Filter helpers", fontSize = 13.sp)
    }
    if (!open) return

    fun add(token: String) {
        if (token in words) return
        store.filter = joinWords(words + token)
    }
    FlowRow(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Helper("Project", store.projects) { add("project:$it") }
        Helper("Tag", store.tags.flatMap { listOf("+$it", "-$it") }) { add(it) }
        Helper("State", listOf("status:pending", "status:completed", "status:deleted", "+OVERDUE", "+DUETODAY", "+WEEK", "+ACTIVE", "+BLOCKED", "+READY"),
            labels = listOf("pending", "completed", "deleted", "overdue", "due today", "due this week", "active", "blocked", "ready")) { add(it) }
        Helper("Priority", listOf("priority:H", "priority:M", "priority:L", "priority.none:"), labels = listOf("H", "M", "L", "none")) { add(it) }
    }
}

@Composable
private fun Helper(label: String, values: List<String>, labels: List<String> = values, onPick: (String) -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        Dim(label)
        Choice("add…", labels.ifEmpty { listOf("(none yet)") }) { i -> values.getOrNull(i)?.let(onPick) }
    }
}
