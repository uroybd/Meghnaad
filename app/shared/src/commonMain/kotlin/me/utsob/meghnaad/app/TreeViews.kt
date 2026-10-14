package me.utsob.meghnaad.app

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.serialization.builtins.ListSerializer

/**
 * The filter box of the Projects and Tags pages, and the tasks it selects (`task <filter> _rows`, any status). The filter is the
 * user's: the pages start with `status:pending`, and taking that chip away brings finished tasks in, so a project or tag stays
 * listed after its last task is done (ExportFilter.svelte).
 */
@Composable
fun ExportFilter(store: Store, filter: String, onFilter: (String) -> Unit, onResult: (rows: List<Row>?, error: String?) -> Unit) {
    // Ask again when the filter changes and when a task changes anywhere (done, edit, add, console…). Only typing waits a
    // moment; opening the page does not.
    var lastFilter by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(filter, store.rev) {
        val typing = lastFilter != null && filter != lastFilter
        if (typing) delay(250)
        lastFilter = filter
        try {
            val r = store.query(splitWords(filter) + "_rows")
            val rows = (r as? CliResult.Json)?.value?.let { engineJson.decodeFromJsonElement(ListSerializer(Row.serializer()), it) }
            onResult(rows, null)
        } catch (e: Exception) {
            onResult(null, e.message ?: e.toString())
        }
    }
    Column(Modifier.fillMaxWidth().padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        FilterField(store, filter, onFilter, placeholder = "Filters: project:Home +work due.before:eow")
        FilterChips(filter, onFilter)
    }
}

/** One pending task as a line in a tree or list: tick it done, open it, see its tags and due date (TaskLine.svelte). */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun TaskLine(store: Store, row: Row, depth: Int) {
    val colors = MaterialTheme.colorScheme
    val pending = row.status == "pending"
    val late = pending && row.due != null && row.due < store.now
    val wash = row.style.tint(extras.dark, colors.surface)
    val fmt = formatFor(FormatKind.Report, store.config?.config?.settings)
    Row(
        Modifier.fillMaxWidth().padding(start = (depth * 14).dp, top = 2.dp, bottom = 2.dp).clip(RoundedCornerShape(10.dp)).background(wash ?: Color.Transparent),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (pending) IconAction("check", "Mark done: ${row.description}", { store.go { act(null, listOf(row.uuid, "done")) } }, tint = extras.ok, size = 16.dp)
        else androidx.compose.foundation.layout.Box(Modifier.padding(start = 44.dp))
        FlowRow(
            Modifier.weight(1f).clickable { store.openDetail(row.uuid) }.padding(vertical = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
            itemVerticalAlignment = Alignment.CenterVertically,
        ) {
            row.priority?.let { Pill(it, bold = true, height = 18.dp) }
            Text(
                row.description,
                fontSize = 15.sp,
                color = if (!pending) colors.onSurfaceVariant else colors.onSurface,
                fontWeight = if (row.style?.bold == true) FontWeight.Bold else FontWeight.Normal,
            )
            row.tags.forEach { t -> Pill(t, height = 20.dp, onClick = { store.showTag(t) }) }
            row.due?.let { Text(formatMoment(it, store.offsetAt(it), fmt), fontSize = 12.5.sp, color = if (late) colors.error else colors.onSurfaceVariant, fontWeight = if (late) FontWeight.SemiBold else FontWeight.Normal) }
        }
    }
}

@Composable
private fun Header(title: String, expandAll: () -> Unit, collapseAll: () -> Unit, canExpand: Boolean, canCollapse: Boolean) {
    Row(Modifier.fillMaxWidth().padding(top = 10.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(title, Modifier.weight(1f), fontSize = 18.sp, fontWeight = FontWeight.SemiBold)
        TextButton(onClick = expandAll, enabled = canExpand) { Text("Expand all", fontSize = 13.sp) }
        TextButton(onClick = collapseAll, enabled = canCollapse) { Text("Collapse all", fontSize = 13.sp) }
    }
}

@Composable
private fun TreeHead(
    icon: String, label: String, isOpen: Boolean, flash: Boolean, depth: Int, count: Int, extra: String?, overdue: Int,
    onToggle: () -> Unit, onOpenInTasks: () -> Unit, labelPrefix: String = "",
) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().padding(start = (depth * 14).dp).clip(RoundedCornerShape(10.dp))
            .background(if (flash) colors.primary.copy(alpha = 0.16f) else Color.Transparent),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(Modifier.weight(1f, fill = false).clickable(onClick = onToggle).padding(vertical = 10.dp, horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            LucideIcon(if (isOpen) "chevron-down" else "chevron-right", size = 16.dp)
            LucideIcon(icon, size = 15.dp)
            Text(labelPrefix + label, fontWeight = FontWeight.SemiBold, fontSize = 15.sp)
        }
        Dim("$count", Modifier.padding(start = 6.dp))
        extra?.let { Dim("· $it", Modifier.padding(start = 4.dp)) }
        if (overdue > 0) Text("$overdue overdue", Modifier.padding(start = 8.dp), color = colors.error, fontSize = 12.5.sp, fontWeight = FontWeight.SemiBold)
        androidx.compose.foundation.layout.Spacer(Modifier.weight(1f))
        TextButton(onClick = onOpenInTasks) { Text("Open in Tasks", fontSize = 12.sp) }
    }
}

private sealed interface TreeItem {
    class Branch(val node: ProjectNode, val depth: Int) : TreeItem
    class Leaf(val row: Row, val depth: Int) : TreeItem
    class Empty(val depth: Int) : TreeItem
}

private fun flatten(nodes: List<ProjectNode>, depth: Int, open: Set<String>, out: MutableList<TreeItem>) {
    for (n in nodes) {
        out += TreeItem.Branch(n, depth)
        if (n.name in open) {
            flatten(n.children, depth + 1, open, out)
            n.own.forEach { out += TreeItem.Leaf(it, depth + 1) }
            if (n.children.isEmpty() && n.own.isEmpty()) out += TreeItem.Empty(depth + 1)
        }
    }
}

/** The projects as a tree: `Home.Kitchen` under `Home`, with each one's tasks (ProjectsView.svelte). */
@Composable
fun ProjectsView(store: Store) {
    var res by remember { mutableStateOf<Pair<List<Row>, String?>?>(null) }
    var open by remember { mutableStateOf(emptySet<String>()) }
    var seeded by remember { mutableStateOf(false) }
    var flash by remember { mutableStateOf<String?>(null) }
    val rows = res?.first ?: emptyList()
    val loading = res == null
    val tree = buildTree(rows, store.now, store.projectFocus?.first)
    val state = rememberLazyListState()

    // First load: open the main projects so their sub-projects show grouped underneath.
    LaunchedEffect(loading) {
        if (!loading && !seeded) {
            seeded = true
            open = tree.filter { it.children.isNotEmpty() }.map { it.name }.toSet()
        }
    }
    // A project tapped somewhere else: open its entry (and the projects above it), bring it into view and flash it.
    val focus = store.projectFocus
    LaunchedEffect(focus, loading) {
        if (focus == null || loading) return@LaunchedEffect
        val parts = focus.first.split('.')
        open = open + parts.indices.map { i -> parts.take(i + 1).joinToString(".") }
        flash = focus.first
        store.focusHandled()
        val items = mutableListOf<TreeItem>().also { flatten(buildTree(rows, store.now, focus.first), 0, open, it) }
        val at = items.indexOfFirst { it is TreeItem.Branch && it.node.name == focus.first }
        if (at >= 0) state.animateScrollToItem(at + 2)
        delay(1800)
        flash = null
    }

    val items = mutableListOf<TreeItem>().also { flatten(tree, 0, open, it) }
    LazyColumn(Modifier.fillMaxSize().padding(horizontal = 12.dp), state = state, contentPadding = androidx.compose.foundation.layout.PaddingValues(bottom = 96.dp)) {
        item { ExportFilter(store, store.projectsFilter, { store.projectsFilter = it }) { r, e -> res = (r ?: res?.first ?: emptyList()) to e } }
        item { Header("Projects", { open = expandable(tree).toSet() }, { open = emptySet() }, tree.isNotEmpty(), open.isNotEmpty()) }
        when {
            res?.second != null -> item { Text(res!!.second!!, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(vertical = 8.dp)) }
            loading -> item { Dim("Loading…", Modifier.padding(vertical = 8.dp)) }
            tree.isEmpty() -> item { Dim("No tasks match this filter.", Modifier.padding(vertical = 8.dp)) }
            else -> itemsIndexed(items) { _, it ->
                when (it) {
                    is TreeItem.Branch -> {
                        val n = it.node
                        TreeHead(
                            "folder", n.label, n.name in open, flash == n.name, it.depth, n.total,
                            if (n.children.isNotEmpty()) "${n.children.size} sub-project${if (n.children.size == 1) "" else "s"}" else null, n.overdue,
                            onToggle = { open = if (n.name in open) open - n.name else open + n.name },
                            onOpenInTasks = {
                                store.filter = if (n.name.isNotEmpty()) "project:${n.name}" else "project:"
                                store.view = View.Tasks
                            },
                        )
                    }
                    is TreeItem.Leaf -> TaskLine(store, it.row, it.depth)
                    is TreeItem.Empty -> Dim("Nothing here.", Modifier.padding(start = (it.depth * 14 + 8).dp, top = 4.dp, bottom = 4.dp))
                }
            }
        }
    }
}

/** The tags, each with the tasks that have it; a task with two tags is under both (TagsView.svelte). */
@Composable
fun TagsView(store: Store) {
    var res by remember { mutableStateOf<Pair<List<Row>, String?>?>(null) }
    var open by remember { mutableStateOf(emptySet<String>()) }
    var flash by remember { mutableStateOf<String?>(null) }
    val rows = res?.first ?: emptyList()
    val loading = res == null
    val tags = buildTags(rows, store.now, store.tagFocus?.first)
    val state = rememberLazyListState()

    // A tag chip tapped somewhere else: open that tag's entry, bring it into view and flash it.
    val focus = store.tagFocus
    LaunchedEffect(focus, loading) {
        if (focus == null || loading) return@LaunchedEffect
        open = open + focus.first
        flash = focus.first
        store.focusHandled()
        val at = buildTags(rows, store.now, focus.first).indexOfFirst { it.name == focus.first }
        if (at >= 0) state.animateScrollToItem(at + 2)
        delay(1800)
        flash = null
    }

    LazyColumn(Modifier.fillMaxSize().padding(horizontal = 12.dp), state = state, contentPadding = androidx.compose.foundation.layout.PaddingValues(bottom = 96.dp)) {
        item { ExportFilter(store, store.tagsFilter, { store.tagsFilter = it }) { r, e -> res = (r ?: res?.first ?: emptyList()) to e } }
        item { Header("Tags", { open = tags.map { it.name }.toSet() }, { open = emptySet() }, tags.isNotEmpty(), open.isNotEmpty()) }
        when {
            res?.second != null -> item { Text(res!!.second!!, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(vertical = 8.dp)) }
            loading -> item { Dim("Loading…", Modifier.padding(vertical = 8.dp)) }
            tags.isEmpty() -> item { Dim("No task with a tag matches this filter.", Modifier.padding(vertical = 8.dp)) }
            else -> for (t in tags) {
                val isOpen = t.name in open
                item(key = "tag:${t.name}") {
                    TreeHead(
                        "tag", t.name, isOpen, flash == t.name, 0, t.tasks.size, null, t.overdue,
                        onToggle = { open = if (isOpen) open - t.name else open + t.name },
                        onOpenInTasks = {
                            store.filter = "+${shellQuote(t.name)}"
                            store.view = View.Tasks
                        },
                        labelPrefix = "+",
                    )
                }
                if (isOpen) {
                    for (r in t.tasks) item(key = "tag:${t.name}:${r.uuid}") { TaskLine(store, r, 1) }
                    if (t.tasks.isEmpty()) item(key = "tag:${t.name}:empty") { Dim("No task matches the filter for this tag.", Modifier.padding(start = 22.dp, top = 4.dp, bottom = 4.dp)) }
                }
            }
        }
    }
}
