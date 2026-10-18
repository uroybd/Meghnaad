package me.utsob.meghnaad.app

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
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
    val choice = store.contextChoice(Scope.Tasks)
    LaunchedEffect(store.report, store.filter, choice) {
        val key = engineJson.encodeToString(ListSerializer(String.serializer()), args)
        if (key == store.liveKey && store.liveContext == choice.orEmpty()) {
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
    val heads = remember(result, ctx) { if (result != null && ctx != null) groupHeads(result, ctx) else emptyList() }
    val dimmed = entry?.loading == true && result != null

    BoxWithConstraints(Modifier.fillMaxSize()) {
    // A table where there is room for one (the web's width), cards where there is not.
    val wide = maxWidth > TABLE_FROM
    val tableScroll = rememberScrollState()
    // The columns' widths, in what the row has after the page's margins, the tick box and the buttons.
    val widths = remember(result, maxWidth) { if (result != null && ctx != null && wide) tableWidths(result, ctx, (maxWidth - 24.dp - TABLE_SELECT - TABLE_ACTIONS).value) else emptyList() }
    LazyColumn(Modifier.fillMaxSize().padding(horizontal = 12.dp), contentPadding = androidx.compose.foundation.layout.PaddingValues(bottom = 96.dp)) {
        item { FilterBar(store) }
        item { BulkBar(store) }
        when {
            entry == null -> Unit
            entry.failure != null -> item { Text(entry.failure!!, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(vertical = 8.dp)) }
            result == null -> item { ResultBlock(store, entry) }
            result.rows.isEmpty() -> item { Dim("No tasks match.", Modifier.padding(vertical = 16.dp), size = 14) }
            wide -> {
                // The Group picker is how a table is grouped on a touch screen (the web's group buttons show on hover).
                item { SortBar(store, result) }
                reportTable(
                    store, result, ctx!!, widths, tableScroll, heads, entry,
                    activeTags = activeTags(store.filter),
                    onTag = { store.filter = toggleTag(store.filter, it) },
                    onEdit = { store.editing = EditRef(it, entry) },
                    onSort = { column, tieBreak -> sortReport(store, result, column, tieBreak) },
                )
            }
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
            }
        }
        // The count, under the cards or the table.
        if (result != null && result.rows.isNotEmpty() && entry?.failure == null) item {
            Dim(
                if (result.rows.size == result.matched) "${result.matched} ${if (result.matched == 1) "task" else "tasks"}"
                else "showing ${result.rows.size} of ${result.matched}",
                Modifier.padding(vertical = 8.dp),
            )
        }
    }
    }
}

/** What a tap on a column's header does: its sort key is added, reversed or dropped, as a rewrite of `rc.report.<name>.sort:…` in the filter. */
private fun sortReport(store: Store, result: ReportResult, column: String, tieBreak: Boolean) {
    val default = store.reports.firstOrNull { it.name == store.report }?.sort
    val res = clickSort(parseSort(result.sort), baseColumn(column), tieBreak)
    store.filter = withSortOverride(store.filter, store.report, if (res.reset) null else serializeSort(res.keys), default)
}

/** Group by [column] (null: stop grouping): the same rewrite, with the trailing `/` on its key. */
private fun groupReport(store: Store, result: ReportResult, column: String?) {
    val default = store.reports.firstOrNull { it.name == store.report }?.sort
    val ks = if (column != null) toggleGroup(parseSort(result.sort), column, parseSort(default)) else clearGroups(parseSort(result.sort))
    store.filter = withSortOverride(store.filter, store.report, serializeSort(ks), default)
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
    fun sortBy(column: String) = sortReport(store, result, column, false)

    fun groupBy(column: String?) = groupReport(store, result, column)

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

/**
 * A small button that opens a menu of choices, answering with the index chosen. [fillHeight]: as tall as what it stands beside
 * (a field), not just as tall as its text.
 */
@Composable
fun Choice(current: String, options: List<String>, modifier: Modifier = Modifier, hints: List<String?> = emptyList(), fillHeight: Boolean = false, onPick: (Int) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box(modifier) {
        OutlinedButton(
            onClick = { open = true },
            modifier = if (fillHeight) Modifier.fillMaxHeight() else Modifier,
            contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp, vertical = 4.dp),
        ) {
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
    val context = store.contextIn(Scope.Tasks)?.let { it to cfg?.contexts?.get(it)?.read }

    fun pickReport(name: String) {
        // Sort overrides belong to one report; drop the ones that no longer apply.
        store.filter = joinWords(words.filter { !Regex("^rc\\.report\\.").containsMatchIn(it) || it.startsWith("rc.report.$name.") })
        store.report = name
    }

    Column(Modifier.fillMaxWidth().padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        var fieldHeight by remember { mutableStateOf(0) }
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            val offered = store.offeredReports
            BesideField(fieldHeight) {
                Choice(
                    store.report,
                    offered.map { it.name },
                    hints = offered.map { it.description },
                    fillHeight = true,
                ) { pickReport(offered[it].name) }
            }
            CompletingField(
                value = store.filter,
                onValueChange = { store.filter = it },
                vocab = store.vocab,
                modifier = Modifier.weight(1f),
                placeholder = "Filters: +work due:eow",
                trailing = { complete -> IconAction("arrow-right-to-line", "Complete", complete, size = 18.dp, box = 32.dp) },
                onSize = { fieldHeight = it.height },
            )
        }
        ContextPicker(store, Scope.Tasks)
        FilterChips(store.filter) { store.filter = it }
        // What the report and the context filter by and sort by on their own: kept out of the way until asked for.
        val about = if (meta?.filter != null || meta?.sort != null || context != null) {
            buildString {
                if (meta?.filter != null) append("report filter ${meta.filter}")
                if (meta?.sort != null) append("${if (isNotEmpty()) " · " else ""}default sort ${meta.sort}")
                if (context != null) append("${if (isNotEmpty()) " · " else ""}context ${context.first}${context.second?.let { " ($it)" } ?: ""}")
            } + " — applied automatically; your filters are added to these."
        } else null
        FilterHelpers(store, words, about)
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
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        CompletingField(
            value, onChange, store.vocab, Modifier.weight(1f), placeholder = placeholder,
            trailing = { complete -> IconAction("arrow-right-to-line", "Complete", complete, size = 18.dp, box = 32.dp) },
        )
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun FilterHelpers(store: Store, words: List<String>, about: String?) {
    var open by remember { mutableStateOf(false) }
    var showAbout by remember { mutableStateOf(false) }
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.SpaceBetween) {
        TextButton(onClick = { open = !open }, contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 4.dp)) {
            LucideIcon("sliders-horizontal", size = 13.dp)
            Text("  Filter helpers", fontSize = 13.sp)
        }
        // What the report filters and sorts by itself, shown or hidden (hidden to begin with).
        if (about != null) {
            IconAction(
                "info", if (showAbout) "Hide what the report filters and sorts by" else "Show what the report filters and sorts by", { showAbout = !showAbout },
                tint = if (showAbout) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant, size = 16.dp, box = 36.dp,
            )
        }
    }
    if (about != null && showAbout) Dim(about, size = 12)
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

/**
 * The context one page shows its tasks in (ContextPicker.svelte). The active context (`context work` in the console) is the
 * console's; a page follows it unless it is told otherwise here, and then keeps its own choice, so a page never changes under another's.
 */
@Composable
fun ContextPicker(store: Store, scope: String, modifier: Modifier = Modifier, fillHeight: Boolean = false) {
    val contexts = store.config?.config?.contexts ?: emptyMap()
    val choice = store.contextChoice(scope)
    if (contexts.isEmpty() && choice == null) return
    val names = contexts.keys.sorted()
    val options = listOf("Context: ${store.activeContext ?: "none"} (same as console)", "Context: none") + names.map { "Context: $it" }
    val current = when (choice) {
        null -> options[0]
        NO_CONTEXT -> options[1]
        else -> "Context: $choice"
    }
    Choice(current, options, modifier, hints = listOf(null, null) + names.map { contexts[it]?.read }, fillHeight = fillHeight) { i ->
        store.setContextChoice(scope, when (i) { 0 -> null; 1 -> NO_CONTEXT; else -> names[i - 2] })
    }
}
